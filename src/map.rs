use super::*;

pub struct MapWriter<K: PcoSerde, V: PcoSerde> {
    pub keys: K::Writer,
    pub values: V::Writer,
    pub entry_counts: Vec<u32>,
}

impl<K: PcoSerde, V: PcoSerde> Default for MapWriter<K, V> {
    fn default() -> Self {
        Self { keys: K::Writer::default(), values: V::Writer::default(), entry_counts: Vec::new() }
    }
}

pub struct MapReader<K: PcoSerde, V: PcoSerde>
where
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    pub keys: K::Reader,
    pub values: V::Reader,
    pub entry_counts: Arc<[u32]>,
    pub count_index: usize,
    pub cached_offset: usize,
}

impl<K: PcoSerde, V: PcoSerde> Default for MapReader<K, V>
where
    K::Reader: Default,
    V::Reader: Default,
{
    fn default() -> Self {
        Self {
            keys: K::Reader::default(),
            values: V::Reader::default(),
            entry_counts: Arc::new([]),
            count_index: 0,
            cached_offset: 0,
        }
    }
}

impl<K: PcoSerde, V: PcoSerde> Clone for MapReader<K, V>
where
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            values: self.values.clone(),
            entry_counts: self.entry_counts.clone(),
            count_index: self.count_index,
            cached_offset: self.cached_offset,
        }
    }
}

/// Filter a flat key stream, tracking entry boundaries via `entry_counts`.
fn filter_map_keys<K, V>(reader: &MapReader<K, V>, filter: &Filter, matches: &mut FilterMask) -> Result<()>
where
    K: PcoFilter,
    V: PcoSerde,
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    let counts = &*reader.entry_counts;
    let num_entries = counts.len();
    let mut virtual_keys = reader.keys.clone();
    let mut offset: usize = 0;
    let mut matches_vec: Vec<bool> = Vec::with_capacity(num_entries);
    for &count in counts.iter() {
        let keys_in_entry = count as usize;
        let mut matched = false;
        for i in 0..keys_in_entry {
            if let Ok(Some(key)) = K::get(&mut virtual_keys, offset + i)
                && K::filter_match(&key, filter)
            {
                matched = true;
                break;
            }
        }
        matches_vec.push(matched);
        offset += keys_in_entry;
    }
    matches.and_with(&FilterMask::from_bool_slice(&matches_vec));
    Ok(())
}

impl<K, V> PcoSerde for BTreeMap<K, V>
where
    K: PcoSerde + Hash + Eq + Ord + Default,
    V: PcoSerde + Default,
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    type Writer = MapWriter<K, V>;
    type Reader = MapReader<K, V>;

    // Cloning a map rebuilds the tree node by node, not one memcpy like a Vec.
    const CHEAP_TO_CLONE: bool = false;

    fn write(data: Vec<Self>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let mut entry_counts: Vec<u32> = Vec::with_capacity(data.len());
        let mut keys: Vec<K> = Vec::with_capacity(data.iter().map(|m| m.len()).sum());
        let mut values: Vec<V> = Vec::with_capacity(data.iter().map(|m| m.len()).sum());
        for map in data {
            entry_counts.push(map.len() as u32);
            for (key, value) in map {
                keys.push(key);
                values.push(value);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&u32::write(entry_counts, 0, time_round)?);
        out.extend_from_slice(&K::write(keys, float_round, time_round)?);
        out.extend_from_slice(&V::write(values, float_round, time_round)?);
        Ok(out)
    }

    fn write_borrowed(values: &[&Self], float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let mut entry_counts: Vec<u32> = Vec::with_capacity(values.len());
        let total_entries: usize = values.iter().map(|m| m.len()).sum();
        let mut keys: Vec<&K> = Vec::with_capacity(total_entries);
        let mut column_values: Vec<&V> = Vec::with_capacity(total_entries);
        for map in values {
            entry_counts.push(map.len() as u32);
            for (key, value) in *map {
                keys.push(key);
                column_values.push(value);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&u32::write(entry_counts, 0, time_round)?);
        out.extend_from_slice(&K::write_borrowed(&keys, float_round, time_round)?);
        out.extend_from_slice(&V::write_borrowed(&column_values, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        let counts_reader = u32::read(src, float_round, time_round)?;
        let entry_counts: Arc<[u32]> = counts_reader.values;
        let keys = K::read(src, float_round, time_round)?;
        let values = V::read(src, float_round, time_round)?;
        Ok(MapReader { keys, values, entry_counts, count_index: 0, cached_offset: 0 })
    }

    fn reader_len(reader: &mut Self::Reader) -> Result<usize> {
        Ok(reader.entry_counts.len())
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let pairs_count = reader.entry_counts.get(index).copied().context("entry_count missing")? as usize;
        let mut offset = reader.cached_offset;
        for i in reader.count_index..index {
            offset += reader.entry_counts[i] as usize;
        }
        reader.cached_offset = offset + pairs_count;
        reader.count_index = index + 1;
        let mut map = BTreeMap::new();
        for i in 0..pairs_count {
            let k = K::get(&mut reader.keys, offset + i)?.context("key missing")?;
            let v = V::get(&mut reader.values, offset + i)?.context("value missing")?;
            map.insert(k, v);
        }
        Ok(Some(map))
    }

    /// Chunk-major survivor rows: project each surviving row's entry span onto
    /// the flat key/value streams, gather both once through their own typed
    /// loops, and move the owned pairs into rows. The deep clone of every
    /// survivor that the row-at-a-time fallback paid (a `serde_json::Value`
    /// tree clone is many allocations) is gone. The scan clamps to the
    /// caller's mask word span, which covers every row below the mask's
    /// length: rows past the last survivor but below that length select
    /// nothing, and the cursor advances past them anyway. Rows the forward
    /// cursor has passed read empty as the documented gather leniency for
    /// out-of-contract cursor reuse, not parity with `get`.
    fn gather(reader: &mut Self::Reader, mask: &FilterMask, out: &mut Vec<Self>) -> Result<()>
    where
        Self: Default,
    {
        let mask_words = mask.gather_words();
        let rows = reader.entry_counts.len().min(mask_words.len() * 64);
        let mut inner_mask: Vec<u64> = Vec::new();
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut offset = reader.cached_offset;
        for i in reader.count_index..rows {
            let count = reader.entry_counts[i] as usize;
            if (mask_words[i >> 6] >> (i & 63)) & 1 == 1 {
                crate::set_bits(&mut inner_mask, offset, count);
                spans.push((i, count));
            }
            offset += count;
        }
        reader.cached_offset = offset;
        reader.count_index = rows;
        let mut keys: Vec<K> = Vec::new();
        let mut values: Vec<V> = Vec::new();
        if !inner_mask.is_empty() {
            let inner_mask = FilterMask::from_raw_words(inner_mask);
            K::gather(&mut reader.keys, &inner_mask, &mut keys)?;
            V::gather(&mut reader.values, &inner_mask, &mut values)?;
        }
        let mut kit = keys.into_iter();
        let mut vit = values.into_iter();
        let mut next = spans.into_iter();
        let mut pending = next.next();
        for_each_set_bit(mask, |row| match pending {
            Some((span_row, count)) if span_row == row => {
                let mut map = BTreeMap::new();
                for _ in 0..count {
                    if let (Some(k), Some(v)) = (kit.next(), vit.next()) {
                        map.insert(k, v);
                    }
                }
                out.push(map);
                pending = next.next();
            }
            Some(_) | None => out.push(BTreeMap::new()),
        });
        Ok(())
    }
}

impl<K, V> PcoFilter for BTreeMap<K, V>
where
    K: PcoFilter + Hash + Eq + Ord + Default,
    V: PcoSerde + Default,
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        filter_map_keys(reader, filter, matches)?;
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        value.keys().any(|k| K::filter_match(k, filter))
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for BTreeMap")
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };
        if let Some(rem) = remainder {
            return Err(::anyhow::anyhow!(
                "BTreeMap filtering only supports key matching; nested path '{}' is not supported",
                rem
            ));
        }
        let filter = K::resolve_filter(root, json)?;
        if filter.path.len() != 1 {
            return Err(::anyhow::anyhow!("Map key filter must resolve to a single segment"));
        }
        Ok(ResolvedFilter { path: vec![0], filter: filter.filter })
    }
}

// HashMap uses the same storage format (and key order) of BTreeMap for ideal compression,
// at the cost of slightly higher serialization times.
impl<K, V> PcoSerde for HashMap<K, V>
where
    K: PcoSerde + Hash + Eq + Ord + Default,
    V: PcoSerde + Default,
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    type Writer = MapWriter<K, V>;
    type Reader = MapReader<K, V>;

    const CHEAP_TO_CLONE: bool = false;

    fn write(data: Vec<Self>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let data: Vec<BTreeMap<K, V>> = data.into_iter().map(|map| map.into_iter().collect()).collect();
        BTreeMap::<K, V>::write(data, float_round, time_round)
    }

    fn write_borrowed(values: &[&Self], float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let mut entry_counts: Vec<u32> = Vec::with_capacity(values.len());
        let total_entries: usize = values.iter().map(|m| m.len()).sum();
        let mut keys: Vec<&K> = Vec::with_capacity(total_entries);
        let mut column_values: Vec<&V> = Vec::with_capacity(total_entries);
        for map in values {
            // Owned write sorts keys via a BTreeMap; match that order for identical bytes.
            let sorted: BTreeMap<&K, &V> = map.iter().collect();
            entry_counts.push(map.len() as u32);
            for (key, value) in sorted {
                keys.push(key);
                column_values.push(value);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&u32::write(entry_counts, 0, time_round)?);
        out.extend_from_slice(&K::write_borrowed(&keys, float_round, time_round)?);
        out.extend_from_slice(&V::write_borrowed(&column_values, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        BTreeMap::<K, V>::read(src, float_round, time_round)
    }

    fn reader_len(reader: &mut Self::Reader) -> Result<usize> {
        BTreeMap::<K, V>::reader_len(reader)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let pairs_count = reader.entry_counts.get(index).copied().context("pairs_count missing")? as usize;
        let mut offset = reader.cached_offset;
        for i in reader.count_index..index {
            offset += reader.entry_counts[i] as usize;
        }
        reader.cached_offset = offset + pairs_count;
        reader.count_index = index + 1;
        let mut map = HashMap::new();
        for i in 0..pairs_count {
            let k = K::get(&mut reader.keys, offset + i)?.context("key missing")?;
            let v = V::get(&mut reader.values, offset + i)?.context("value missing")?;
            map.insert(k, v);
        }
        Ok(Some(map))
    }

    /// The storage is BTreeMap's, so the surviving rows project onto the same
    /// flat entry spans `BTreeMap::gather` uses; pairs move into each HashMap.
    /// The scan clamps to the caller's mask word span.
    fn gather(reader: &mut Self::Reader, mask: &FilterMask, out: &mut Vec<Self>) -> Result<()>
    where
        Self: Default,
    {
        let mask_words = mask.gather_words();
        let rows = reader.entry_counts.len().min(mask_words.len() * 64);
        let mut inner_mask: Vec<u64> = Vec::new();
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut offset = reader.cached_offset;
        for i in reader.count_index..rows {
            let count = reader.entry_counts[i] as usize;
            if (mask_words[i >> 6] >> (i & 63)) & 1 == 1 {
                crate::set_bits(&mut inner_mask, offset, count);
                spans.push((i, count));
            }
            offset += count;
        }
        reader.cached_offset = offset;
        reader.count_index = rows;
        let mut keys: Vec<K> = Vec::new();
        let mut values: Vec<V> = Vec::new();
        if !inner_mask.is_empty() {
            let inner_mask = FilterMask::from_raw_words(inner_mask);
            K::gather(&mut reader.keys, &inner_mask, &mut keys)?;
            V::gather(&mut reader.values, &inner_mask, &mut values)?;
        }
        let mut kit = keys.into_iter();
        let mut vit = values.into_iter();
        let mut next = spans.into_iter();
        let mut pending = next.next();
        for_each_set_bit(mask, |row| match pending {
            Some((span_row, count)) if span_row == row => {
                let mut map = HashMap::with_capacity(count);
                for _ in 0..count {
                    if let (Some(k), Some(v)) = (kit.next(), vit.next()) {
                        map.insert(k, v);
                    }
                }
                out.push(map);
                pending = next.next();
            }
            Some(_) | None => out.push(HashMap::new()),
        });
        Ok(())
    }
}

impl<K, V> PcoFilter for HashMap<K, V>
where
    K: PcoFilter + Hash + Eq + Ord + Default,
    V: PcoSerde + Default,
    K::Reader: Clone + Default,
    V::Reader: Clone + Default,
{
    fn filter_bulk(reader: &mut Self::Reader, _field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        filter_map_keys(reader, filter, matches)?;
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        value.keys().any(|k| K::filter_match(k, filter))
    }

    fn filter_nested(
        _reader: &mut Self::Reader, _path: &[usize], _filter: &Filter, _matches: &mut FilterMask,
    ) -> Result<()> {
        unreachable!("filter_nested not supported for HashMap")
    }

    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };
        if let Some(rem) = remainder {
            return Err(::anyhow::anyhow!(
                "HashMap filtering only supports key matching; nested path '{}' is not supported",
                rem
            ));
        }
        let filter = K::resolve_filter(root, json)?;
        if filter.path.len() != 1 {
            return Err(::anyhow::anyhow!("Map key filter must resolve to a single segment"));
        }
        Ok(ResolvedFilter { path: vec![0], filter: filter.filter })
    }
}
