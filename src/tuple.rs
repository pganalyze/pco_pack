use super::*;

fn filter_tuple_element<T: PcoFilter>(reader: &mut T::Reader, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
    let len = T::validate_bounds(reader)?.unwrap_or(0);
    if len == 0 {
        return Ok(());
    }
    let mut local_matches = FilterMask::ones(len);
    T::filter_bulk(reader, 0, filter, &mut local_matches)?;
    matches.and_with(&local_matches);
    Ok(())
}

fn filter_tuple_element_nested<T: PcoFilter>(
    reader: &mut T::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
) -> Result<()> {
    if path.len() > 1 {
        T::filter_nested(reader, &path[1..], filter, matches)?;
    } else {
        filter_tuple_element::<T>(reader, filter, matches)?;
    }
    Ok(())
}

impl<T1, T2> PcoSerde for (T1, T2)
where
    T1: PcoSerde,
    T2: PcoSerde + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
{
    type Writer = (T1::Writer, T2::Writer);
    type Reader = (T1::Reader, T2::Reader);

    fn write(data: Vec<(T1, T2)>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let (mut c0, mut c1) = (Vec::with_capacity(data.len()), Vec::with_capacity(data.len()));
        for (v0, v1) in data {
            c0.push(v0);
            c1.push(v1);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&T1::write(c0, float_round, time_round)?);
        out.extend_from_slice(&T2::write(c1, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        Ok((T1::read(src, float_round, time_round)?, T2::read(src, float_round, time_round)?))
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        let mut max_rows: Option<usize> = None;
        match T1::validate_bounds(&mut reader.0)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T2::validate_bounds(&mut reader.1)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        if max_rows.is_none() {
            let has_any = matches!(T1::validate_bounds(&mut reader.0)?, Some(_))
                || matches!(T2::validate_bounds(&mut reader.1)?, Some(_));
            if has_any {
                return Ok(Some(0));
            }
            return Ok(None);
        }
        Ok(max_rows)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let (c0, c1) = &mut *reader;
        let v0 = T1::get(c0, index)?.ok_or_else(|| anyhow::anyhow!("tuple element 0 missing at index {}", index))?;
        let v1 = T2::get(c1, index)?.unwrap_or_default();
        Ok(Some((v0, v1)))
    }
}

impl<T1, T2> PcoFilter for (T1, T2)
where
    T1: PcoFilter,
    T2: PcoFilter + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
{
    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };

        match root {
            "0" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 0 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T1::resolve_filter("", json)?;
                filter.path[0] = 0;
                Ok(filter)
            }
            "1" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 1 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T2::resolve_filter("", json)?;
                filter.path[0] = 1;
                Ok(filter)
            }
            _ => Err(::anyhow::anyhow!("Tuple index '{}' not found; expected a numeric string index", root)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match field {
            0 => filter_tuple_element::<T1>(&mut reader.0, filter, matches)?,
            1 => filter_tuple_element::<T2>(&mut reader.1, filter, matches)?,
            _ => return Err(anyhow::anyhow!("filter_bulk called with invalid field index {} for tuple", field)),
        }
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        let (t1, t2) = value;
        T1::filter_match(t1, filter) || T2::filter_match(t2, filter)
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        match path[0] {
            0 => filter_tuple_element_nested::<T1>(&mut reader.0, &path[1..], filter, matches)?,
            1 => filter_tuple_element_nested::<T2>(&mut reader.1, &path[1..], filter, matches)?,
            _ => return Err(anyhow::anyhow!("filter_nested called with invalid path {:?} for tuple", path)),
        }
        Ok(())
    }
}

impl<T1, T2, T3> PcoSerde for (T1, T2, T3)
where
    T1: PcoSerde,
    T2: PcoSerde + Default,
    T3: PcoSerde + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
{
    type Writer = (T1::Writer, T2::Writer, T3::Writer);
    type Reader = (T1::Reader, T2::Reader, T3::Reader);

    fn write(data: Vec<(T1, T2, T3)>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let (mut c0, mut c1, mut c2) =
            (Vec::with_capacity(data.len()), Vec::with_capacity(data.len()), Vec::with_capacity(data.len()));
        for (v0, v1, v2) in data {
            c0.push(v0);
            c1.push(v1);
            c2.push(v2);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&T1::write(c0, float_round, time_round)?);
        out.extend_from_slice(&T2::write(c1, float_round, time_round)?);
        out.extend_from_slice(&T3::write(c2, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        Ok((
            T1::read(src, float_round, time_round)?,
            T2::read(src, float_round, time_round)?,
            T3::read(src, float_round, time_round)?,
        ))
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        // Find the max row count across all elements, treating 0-length as "missing" (defaults).
        let mut max_rows: Option<usize> = None;
        match T1::validate_bounds(&mut reader.0)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T2::validate_bounds(&mut reader.1)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T3::validate_bounds(&mut reader.2)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        if max_rows.is_none() {
            let has_any = matches!(T1::validate_bounds(&mut reader.0)?, Some(_))
                || matches!(T2::validate_bounds(&mut reader.1)?, Some(_))
                || matches!(T3::validate_bounds(&mut reader.2)?, Some(_));
            if has_any {
                return Ok(Some(0));
            }
            return Ok(None);
        }
        Ok(max_rows)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let (c0, c1, c2) = &mut *reader;
        let v0 = T1::get(c0, index)?.ok_or_else(|| anyhow::anyhow!("tuple element 0 missing at index {}", index))?;
        let v1 = T2::get(c1, index)?.unwrap_or_default();
        let v2 = T3::get(c2, index)?.unwrap_or_default();
        Ok(Some((v0, v1, v2)))
    }
}

impl<T1, T2, T3> PcoFilter for (T1, T2, T3)
where
    T1: PcoFilter,
    T2: PcoFilter + Default,
    T3: PcoFilter + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
{
    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };

        match root {
            "0" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 0 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T1::resolve_filter("", json)?;
                filter.path[0] = 0;
                Ok(filter)
            }
            "1" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 1 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T2::resolve_filter("", json)?;
                filter.path[0] = 1;
                Ok(filter)
            }
            "2" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 2 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T3::resolve_filter("", json)?;
                filter.path[0] = 2;
                Ok(filter)
            }
            _ => Err(::anyhow::anyhow!("Tuple index '{}' not found; expected a numeric string index", root)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match field {
            0 => filter_tuple_element::<T1>(&mut reader.0, filter, matches)?,
            1 => filter_tuple_element::<T2>(&mut reader.1, filter, matches)?,
            2 => filter_tuple_element::<T3>(&mut reader.2, filter, matches)?,
            _ => return Err(anyhow::anyhow!("filter_bulk called with invalid field index {} for tuple", field)),
        }
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        let (t1, t2, t3) = value;
        T1::filter_match(t1, filter) || T2::filter_match(t2, filter) || T3::filter_match(t3, filter)
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        match path[0] {
            0 => filter_tuple_element_nested::<T1>(&mut reader.0, &path[1..], filter, matches)?,
            1 => filter_tuple_element_nested::<T2>(&mut reader.1, &path[1..], filter, matches)?,
            2 => filter_tuple_element_nested::<T3>(&mut reader.2, &path[1..], filter, matches)?,
            _ => return Err(anyhow::anyhow!("filter_nested called with invalid path {:?} for tuple", path)),
        }
        Ok(())
    }
}

impl<T1, T2, T3, T4> PcoSerde for (T1, T2, T3, T4)
where
    T1: PcoSerde,
    T2: PcoSerde + Default,
    T3: PcoSerde + Default,
    T4: PcoSerde + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
    T4::Reader: Clone,
{
    type Writer = (T1::Writer, T2::Writer, T3::Writer, T4::Writer);
    type Reader = (T1::Reader, T2::Reader, T3::Reader, T4::Reader);

    fn write(data: Vec<(T1, T2, T3, T4)>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Vec<u8>> {
        let (mut c0, mut c1, mut c2, mut c3) = (
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
        );
        for (v0, v1, v2, v3) in data {
            c0.push(v0);
            c1.push(v1);
            c2.push(v2);
            c3.push(v3);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&T1::write(c0, float_round, time_round)?);
        out.extend_from_slice(&T2::write(c1, float_round, time_round)?);
        out.extend_from_slice(&T3::write(c2, float_round, time_round)?);
        out.extend_from_slice(&T4::write(c3, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        Ok((
            T1::read(src, float_round, time_round)?,
            T2::read(src, float_round, time_round)?,
            T3::read(src, float_round, time_round)?,
            T4::read(src, float_round, time_round)?,
        ))
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        let mut max_rows: Option<usize> = None;
        match T1::validate_bounds(&mut reader.0)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T2::validate_bounds(&mut reader.1)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T3::validate_bounds(&mut reader.2)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T4::validate_bounds(&mut reader.3)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        if max_rows.is_none() {
            let has_any = matches!(T1::validate_bounds(&mut reader.0)?, Some(_))
                || matches!(T2::validate_bounds(&mut reader.1)?, Some(_))
                || matches!(T3::validate_bounds(&mut reader.2)?, Some(_))
                || matches!(T4::validate_bounds(&mut reader.3)?, Some(_));
            if has_any {
                return Ok(Some(0));
            }
            return Ok(None);
        }
        Ok(max_rows)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let (c0, c1, c2, c3) = &mut *reader;
        let v0 = T1::get(c0, index)?.ok_or_else(|| anyhow::anyhow!("tuple element 0 missing at index {}", index))?;
        let v1 = T2::get(c1, index)?.unwrap_or_default();
        let v2 = T3::get(c2, index)?.unwrap_or_default();
        let v3 = T4::get(c3, index)?.unwrap_or_default();
        Ok(Some((v0, v1, v2, v3)))
    }
}

impl<T1, T2, T3, T4> PcoFilter for (T1, T2, T3, T4)
where
    T1: PcoFilter,
    T2: PcoFilter + Default,
    T3: PcoFilter + Default,
    T4: PcoFilter + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
    T4::Reader: Clone,
{
    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };

        match root {
            "0" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 0 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T1::resolve_filter("", json)?;
                filter.path[0] = 0;
                Ok(filter)
            }
            "1" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 1 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T2::resolve_filter("", json)?;
                filter.path[0] = 1;
                Ok(filter)
            }
            "2" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 2 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T3::resolve_filter("", json)?;
                filter.path[0] = 2;
                Ok(filter)
            }
            "3" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 3 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T4::resolve_filter("", json)?;
                filter.path[0] = 3;
                Ok(filter)
            }
            _ => Err(::anyhow::anyhow!("Tuple index '{}' not found; expected a numeric string index", root)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match field {
            0 => filter_tuple_element::<T1>(&mut reader.0, filter, matches)?,
            1 => filter_tuple_element::<T2>(&mut reader.1, filter, matches)?,
            2 => filter_tuple_element::<T3>(&mut reader.2, filter, matches)?,
            3 => filter_tuple_element::<T4>(&mut reader.3, filter, matches)?,
            _ => return Err(anyhow::anyhow!("filter_bulk called with invalid field index {} for tuple", field)),
        }
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        let (t1, t2, t3, t4) = value;
        T1::filter_match(t1, filter)
            || T2::filter_match(t2, filter)
            || T3::filter_match(t3, filter)
            || T4::filter_match(t4, filter)
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        match path[0] {
            0 => {
                filter_tuple_element_nested::<T1>(&mut reader.0, &path[1..], filter, matches)?;
            }
            1 => {
                filter_tuple_element_nested::<T2>(&mut reader.1, &path[1..], filter, matches)?;
            }
            2 => {
                filter_tuple_element_nested::<T3>(&mut reader.2, &path[1..], filter, matches)?;
            }
            3 => {
                filter_tuple_element_nested::<T4>(&mut reader.3, &path[1..], filter, matches)?;
            }
            _ => return Err(anyhow::anyhow!("filter_nested called with invalid path {:?} for tuple", path)),
        }
        Ok(())
    }
}

impl<T1, T2, T3, T4, T5> PcoSerde for (T1, T2, T3, T4, T5)
where
    T1: PcoSerde,
    T2: PcoSerde + Default,
    T3: PcoSerde + Default,
    T4: PcoSerde + Default,
    T5: PcoSerde + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
    T4::Reader: Clone,
    T5::Reader: Clone,
{
    type Writer = (T1::Writer, T2::Writer, T3::Writer, T4::Writer, T5::Writer);
    type Reader = (T1::Reader, T2::Reader, T3::Reader, T4::Reader, T5::Reader);

    fn write(
        data: Vec<(T1, T2, T3, T4, T5)>, float_round: u32, time_round: chrono::Duration,
    ) -> anyhow::Result<Vec<u8>> {
        let (mut c0, mut c1, mut c2, mut c3, mut c4) = (
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
        );
        for (v0, v1, v2, v3, v4) in data {
            c0.push(v0);
            c1.push(v1);
            c2.push(v2);
            c3.push(v3);
            c4.push(v4);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&T1::write(c0, float_round, time_round)?);
        out.extend_from_slice(&T2::write(c1, float_round, time_round)?);
        out.extend_from_slice(&T3::write(c2, float_round, time_round)?);
        out.extend_from_slice(&T4::write(c3, float_round, time_round)?);
        out.extend_from_slice(&T5::write(c4, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        Ok((
            T1::read(src, float_round, time_round)?,
            T2::read(src, float_round, time_round)?,
            T3::read(src, float_round, time_round)?,
            T4::read(src, float_round, time_round)?,
            T5::read(src, float_round, time_round)?,
        ))
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        // Find the max row count across all elements, treating 0-length as "missing" (defaults).
        let mut max_rows: Option<usize> = None;
        match T1::validate_bounds(&mut reader.0)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T2::validate_bounds(&mut reader.1)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T3::validate_bounds(&mut reader.2)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T4::validate_bounds(&mut reader.3)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T5::validate_bounds(&mut reader.4)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        if max_rows.is_none() {
            let has_any = matches!(T1::validate_bounds(&mut reader.0)?, Some(_))
                || matches!(T2::validate_bounds(&mut reader.1)?, Some(_))
                || matches!(T3::validate_bounds(&mut reader.2)?, Some(_))
                || matches!(T4::validate_bounds(&mut reader.3)?, Some(_))
                || matches!(T5::validate_bounds(&mut reader.4)?, Some(_));
            if has_any {
                return Ok(Some(0));
            }
            return Ok(None);
        }
        Ok(max_rows)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let (c0, c1, c2, c3, c4) = &mut *reader;
        let v0 = T1::get(c0, index)?.ok_or_else(|| anyhow::anyhow!("tuple element 0 missing at index {}", index))?;
        let v1 = T2::get(c1, index)?.unwrap_or_default();
        let v2 = T3::get(c2, index)?.unwrap_or_default();
        let v3 = T4::get(c3, index)?.unwrap_or_default();
        let v4 = T5::get(c4, index)?.unwrap_or_default();
        Ok(Some((v0, v1, v2, v3, v4)))
    }
}

impl<T1, T2, T3, T4, T5> PcoFilter for (T1, T2, T3, T4, T5)
where
    T1: PcoFilter,
    T2: PcoFilter + Default,
    T3: PcoFilter + Default,
    T4: PcoFilter + Default,
    T5: PcoFilter + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
    T4::Reader: Clone,
    T5::Reader: Clone,
{
    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };

        match root {
            "0" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 0 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T1::resolve_filter("", json)?;
                filter.path[0] = 0;
                Ok(filter)
            }
            "1" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 1 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T2::resolve_filter("", json)?;
                filter.path[0] = 1;
                Ok(filter)
            }
            "2" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 2 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T3::resolve_filter("", json)?;
                filter.path[0] = 2;
                Ok(filter)
            }
            "3" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 3 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T4::resolve_filter("", json)?;
                filter.path[0] = 3;
                Ok(filter)
            }
            "4" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 4 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T5::resolve_filter("", json)?;
                filter.path[0] = 4;
                Ok(filter)
            }
            _ => Err(::anyhow::anyhow!("Tuple index '{}' not found; expected a numeric string index", root)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match field {
            0 => filter_tuple_element::<T1>(&mut reader.0, filter, matches)?,
            1 => filter_tuple_element::<T2>(&mut reader.1, filter, matches)?,
            2 => filter_tuple_element::<T3>(&mut reader.2, filter, matches)?,
            3 => filter_tuple_element::<T4>(&mut reader.3, filter, matches)?,
            4 => filter_tuple_element::<T5>(&mut reader.4, filter, matches)?,
            _ => return Err(anyhow::anyhow!("filter_bulk called with invalid field index {} for tuple", field)),
        }
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        let (t1, t2, t3, t4, t5) = value;
        T1::filter_match(t1, filter)
            || T2::filter_match(t2, filter)
            || T3::filter_match(t3, filter)
            || T4::filter_match(t4, filter)
            || T5::filter_match(t5, filter)
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        match path[0] {
            0 => {
                filter_tuple_element_nested::<T1>(&mut reader.0, &path[1..], filter, matches)?;
            }
            1 => {
                filter_tuple_element_nested::<T2>(&mut reader.1, &path[1..], filter, matches)?;
            }
            2 => {
                filter_tuple_element_nested::<T3>(&mut reader.2, &path[1..], filter, matches)?;
            }
            3 => {
                filter_tuple_element_nested::<T4>(&mut reader.3, &path[1..], filter, matches)?;
            }
            4 => {
                filter_tuple_element_nested::<T5>(&mut reader.4, &path[1..], filter, matches)?;
            }
            _ => return Err(anyhow::anyhow!("filter_nested called with invalid path {:?} for tuple", path)),
        }
        Ok(())
    }
}

impl<T1, T2, T3, T4, T5, T6> PcoSerde for (T1, T2, T3, T4, T5, T6)
where
    T1: PcoSerde,
    T2: PcoSerde + Default,
    T3: PcoSerde + Default,
    T4: PcoSerde + Default,
    T5: PcoSerde + Default,
    T6: PcoSerde + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
    T4::Reader: Clone,
    T5::Reader: Clone,
    T6::Reader: Clone,
{
    type Writer = (T1::Writer, T2::Writer, T3::Writer, T4::Writer, T5::Writer, T6::Writer);
    type Reader = (T1::Reader, T2::Reader, T3::Reader, T4::Reader, T5::Reader, T6::Reader);

    fn write(
        data: Vec<(T1, T2, T3, T4, T5, T6)>, float_round: u32, time_round: chrono::Duration,
    ) -> anyhow::Result<Vec<u8>> {
        let (mut c0, mut c1, mut c2, mut c3, mut c4, mut c5) = (
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
            Vec::with_capacity(data.len()),
        );
        for (v0, v1, v2, v3, v4, v5) in data {
            c0.push(v0);
            c1.push(v1);
            c2.push(v2);
            c3.push(v3);
            c4.push(v4);
            c5.push(v5);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&T1::write(c0, float_round, time_round)?);
        out.extend_from_slice(&T2::write(c1, float_round, time_round)?);
        out.extend_from_slice(&T3::write(c2, float_round, time_round)?);
        out.extend_from_slice(&T4::write(c3, float_round, time_round)?);
        out.extend_from_slice(&T5::write(c4, float_round, time_round)?);
        out.extend_from_slice(&T6::write(c5, float_round, time_round)?);
        Ok(out)
    }

    fn read(src: &mut Cursor<&[u8]>, float_round: u32, time_round: chrono::Duration) -> anyhow::Result<Self::Reader> {
        Ok((
            T1::read(src, float_round, time_round)?,
            T2::read(src, float_round, time_round)?,
            T3::read(src, float_round, time_round)?,
            T4::read(src, float_round, time_round)?,
            T5::read(src, float_round, time_round)?,
            T6::read(src, float_round, time_round)?,
        ))
    }

    fn validate_bounds(reader: &mut Self::Reader) -> Result<Option<usize>> {
        // Find the max row count across all elements, treating 0-length as "missing" (defaults).
        let mut max_rows: Option<usize> = None;
        match T1::validate_bounds(&mut reader.0)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T2::validate_bounds(&mut reader.1)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T3::validate_bounds(&mut reader.2)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T4::validate_bounds(&mut reader.3)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T5::validate_bounds(&mut reader.4)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        match T6::validate_bounds(&mut reader.5)? {
            Some(r) if r > 0 => max_rows = Some(max_rows.map_or(r, |prev| prev.max(r))),
            _ => {}
        }
        if max_rows.is_none() {
            let has_any = matches!(T1::validate_bounds(&mut reader.0)?, Some(_))
                || matches!(T2::validate_bounds(&mut reader.1)?, Some(_))
                || matches!(T3::validate_bounds(&mut reader.2)?, Some(_))
                || matches!(T4::validate_bounds(&mut reader.3)?, Some(_))
                || matches!(T5::validate_bounds(&mut reader.4)?, Some(_))
                || matches!(T6::validate_bounds(&mut reader.5)?, Some(_));
            if has_any {
                return Ok(Some(0));
            }
            return Ok(None);
        }
        Ok(max_rows)
    }

    fn get(reader: &mut Self::Reader, index: usize) -> Result<Option<Self>> {
        let (c0, c1, c2, c3, c4, c5) = &mut *reader;
        let v0 = T1::get(c0, index)?.ok_or_else(|| anyhow::anyhow!("tuple element 0 missing at index {}", index))?;
        let v1 = T2::get(c1, index)?.unwrap_or_default();
        let v2 = T3::get(c2, index)?.unwrap_or_default();
        let v3 = T4::get(c3, index)?.unwrap_or_default();
        let v4 = T5::get(c4, index)?.unwrap_or_default();
        let v5 = T6::get(c5, index)?.unwrap_or_default();
        Ok(Some((v0, v1, v2, v3, v4, v5)))
    }
}

impl<T1, T2, T3, T4, T5, T6> PcoFilter for (T1, T2, T3, T4, T5, T6)
where
    T1: PcoFilter,
    T2: PcoFilter + Default,
    T3: PcoFilter + Default,
    T4: PcoFilter + Default,
    T5: PcoFilter + Default,
    T6: PcoFilter + Default,
    T1::Reader: Clone,
    T2::Reader: Clone,
    T3::Reader: Clone,
    T4::Reader: Clone,
    T5::Reader: Clone,
    T6::Reader: Clone,
{
    fn resolve_filter(path: &str, json: &serde_json::Value) -> ::anyhow::Result<ResolvedFilter> {
        let (root, remainder) = match path.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (path, None),
        };

        match root {
            "0" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 0 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T1::resolve_filter("", json)?;
                filter.path[0] = 0;
                Ok(filter)
            }
            "1" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 1 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T2::resolve_filter("", json)?;
                filter.path[0] = 1;
                Ok(filter)
            }
            "2" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 2 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T3::resolve_filter("", json)?;
                filter.path[0] = 2;
                Ok(filter)
            }
            "3" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 3 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T4::resolve_filter("", json)?;
                filter.path[0] = 3;
                Ok(filter)
            }
            "4" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 4 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T5::resolve_filter("", json)?;
                filter.path[0] = 4;
                Ok(filter)
            }
            "5" => {
                if let Some(rem) = remainder {
                    return Err(::anyhow::anyhow!(
                        "Tuple element 5 is a leaf index and cannot contain nested path '{}'",
                        rem
                    ));
                }
                let mut filter = T6::resolve_filter("", json)?;
                filter.path[0] = 5;
                Ok(filter)
            }
            _ => Err(::anyhow::anyhow!("Tuple index '{}' not found; expected a numeric string index", root)),
        }
    }

    fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &Filter, matches: &mut FilterMask) -> Result<()> {
        match field {
            0 => {
                filter_tuple_element::<T1>(&mut reader.0, filter, matches)?;
            }
            1 => {
                filter_tuple_element::<T2>(&mut reader.1, filter, matches)?;
            }
            2 => {
                filter_tuple_element::<T3>(&mut reader.2, filter, matches)?;
            }
            3 => {
                filter_tuple_element::<T4>(&mut reader.3, filter, matches)?;
            }
            4 => {
                filter_tuple_element::<T5>(&mut reader.4, filter, matches)?;
            }
            5 => {
                filter_tuple_element::<T6>(&mut reader.5, filter, matches)?;
            }
            _ => return Err(anyhow::anyhow!("filter_bulk called with invalid field index {} for tuple", field)),
        }
        Ok(())
    }

    fn filter_match(value: &Self, filter: &Filter) -> bool {
        let (t1, t2, t3, t4, t5, t6) = value;
        T1::filter_match(t1, filter)
            || T2::filter_match(t2, filter)
            || T3::filter_match(t3, filter)
            || T4::filter_match(t4, filter)
            || T5::filter_match(t5, filter)
            || T6::filter_match(t6, filter)
    }

    fn filter_nested(
        reader: &mut Self::Reader, path: &[usize], filter: &Filter, matches: &mut FilterMask,
    ) -> Result<()> {
        match path[0] {
            0 => {
                filter_tuple_element_nested::<T1>(&mut reader.0, &path[1..], filter, matches)?;
            }
            1 => {
                filter_tuple_element_nested::<T2>(&mut reader.1, &path[1..], filter, matches)?;
            }
            2 => {
                filter_tuple_element_nested::<T3>(&mut reader.2, &path[1..], filter, matches)?;
            }
            3 => {
                filter_tuple_element_nested::<T4>(&mut reader.3, &path[1..], filter, matches)?;
            }
            4 => {
                filter_tuple_element_nested::<T5>(&mut reader.4, &path[1..], filter, matches)?;
            }
            5 => {
                filter_tuple_element_nested::<T6>(&mut reader.5, &path[1..], filter, matches)?;
            }
            _ => return Err(anyhow::anyhow!("filter_nested called with invalid path {:?} for tuple", path)),
        }
        Ok(())
    }
}
