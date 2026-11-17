use crate::*;
use ::uuid::Uuid;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::Cursor;
use std::sync::Arc;

/// Deterministic xorshift so any failure reproduces exactly.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn chance(&mut self, pct: u64) -> bool {
        self.next_u64() % 100 < pct
    }
}

/// Random mask over `rows` rows. `FilterMask::gather_words` clears any bits
/// past `rows` in the last word, exactly like the masks the generated loops
/// hand to `gather`.
fn random_mask(rows: usize, pct: u64, rng: &mut Rng) -> FilterMask {
    let bits: Vec<bool> = (0..rows).map(|_| rng.chance(pct)).collect();
    FilterMask::from_bool_slice(&bits)
}

/// Row-at-a-time reference: `pop_inner` semantics, `Default` for any index
/// `get` cannot answer. A failed `get` on data this suite wrote is a bug,
/// not an expected outcome, so it panics rather than defaulting.
fn expected_via_get<T: PcoSerde + Default>(reader: &mut T::Reader, mask: &FilterMask) -> Vec<T> {
    let mut out = Vec::new();
    for_each_set_bit(mask, |i| match T::get(reader, i) {
        Ok(Some(v)) => {
            out.push(v);
            Ok(())
        }
        Ok(None) => {
            out.push(T::default());
            Ok(())
        }
        Err(e) => panic!("get failed on well-formed data: {e}"),
    })
    .unwrap();
    out
}

/// Gather and the row loop must agree for every reader shape: buffer-backed
/// overrides, forward-cursor containers, the `Rounded` float encoding, short
/// buffers, and row counts that are not multiples of 64. Fresh readers per
/// round because forward cursors never rewind.
fn gather_matches_get<T>(label: &str, values: &[T], pct: u64, float_round: u32)
where
    T: PcoSerde + Default + PartialEq + Debug + Clone,
    T::Reader: Clone,
{
    let rows = values.len();
    let bytes = T::write(values.to_vec(), float_round, Default::default()).unwrap();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (rows as u64) ^ (pct << 32) ^ (float_round as u64));
    for round in 0..8 {
        let mask = random_mask(rows, pct, &mut rng);
        let mut gathered_reader = T::read(&mut Cursor::new(&bytes[..]), float_round, Default::default()).unwrap();
        let mut gathered = Vec::new();
        T::gather(&mut gathered_reader, &mask, &mut gathered).unwrap();
        let mut row_reader = T::read(&mut Cursor::new(&bytes[..]), float_round, Default::default()).unwrap();
        let expected = expected_via_get::<T>(&mut row_reader, &mask);
        assert_eq!(gathered.len(), mask.count_ones(), "{label}");
        assert_eq!(gathered, expected, "{label} round {round} pct {pct} rows {rows}");
    }
}

#[test]
fn gather_ints_match_get_across_selectivities() {
    let rows: Vec<i64> = (0..1000).collect();
    for pct in [0, 1, 25, 50, 75, 100] {
        gather_matches_get("i64", &rows, pct, 0);
    }
    let small: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    for pct in [1, 50, 100] {
        gather_matches_get("u8", &small, pct, 0);
    }
    let neg: Vec<i32> = (0..130_i32).map(|i| i - 64).collect();
    for pct in [25, 50, 100] {
        gather_matches_get("i32", &neg, pct, 0);
    }
    let wide: Vec<u64> = (0..1000).map(|i| u64::MAX - i).collect();
    gather_matches_get("u64", &wide, 37, 0);
    gather_matches_get("i8", &(0..65_i8).cycle().take(257).collect::<Vec<i8>>(), 60, 0);
    gather_matches_get("u32", &(0..131_u32).map(|i| i.wrapping_mul(7)).collect::<Vec<u32>>(), 45, 0);
    gather_matches_get("u16", &(0..70_u16).collect::<Vec<u16>>(), 33, 0);
    gather_matches_get("i16", &(0..70_i16).collect::<Vec<i16>>(), 33, 0);
}

#[test]
fn gather_floats_match_get_including_rounded() {
    let f64s: Vec<f64> = (0..1000).map(|i| i as f64 * 1.5).collect();
    for pct in [1, 25, 50, 100] {
        gather_matches_get("f64", &f64s, pct, 0);
    }
    let f32s: Vec<f32> = (0..130).map(|i| i as f32 / 3.0).collect();
    gather_matches_get("f32", &f32s, 40, 0);
    let f16s: Vec<half::f16> = (0..70).map(|i| half::f16::from_f64(i as f64)).collect();
    gather_matches_get("f16", &f16s, 55, 0);

    // `Rounded` columns decode through a different branch; one mask has to
    // produce the identical stream of unrounded values.
    gather_matches_get("f64_rounded", &f64s, 25, 2);
    gather_matches_get("f32_rounded", &f32s, 25, 3);
    gather_matches_get("f16_rounded", &f16s, 25, 2);
}

#[test]
fn gather_bools_match_get() {
    let bools: Vec<bool> = (0..300).map(|i| i % 7 < 3).collect();
    for pct in [1, 50, 100] {
        gather_matches_get("bool", &bools, pct, 0);
    }
}

#[test]
fn gather_dates_match_get() {
    let dates: Vec<chrono::DateTime<chrono::Utc>> = (0..200)
        .map(|i| chrono::DateTime::from_timestamp_micros(1_700_000_000_000_000 + i * 1_000_000).unwrap())
        .collect();
    for pct in [1, 25, 100] {
        gather_matches_get("datetime", &dates, pct, 0);
    }
}

#[test]
fn gather_uuids_match_get() {
    let uuids: Vec<Uuid> = (0..130).map(|i| Uuid::from_u128(i as u128 * 0x9E37_79B9_7F4A_7C15)).collect();
    for pct in [1, 40, 100] {
        gather_matches_get("uuid", &uuids, pct, 0);
    }
}

#[test]
fn gather_strings_match_get_across_chunks() {
    // 300 rows spans three lazy-msgpack chunks; the trailing partial chunk
    // exercises the same tail the word mask masks against.
    let strings: Vec<String> = (0..300).map(|i| format!("value_{i}_{}", i % 7 == 0)).collect();
    for pct in [1, 25, 50, 100] {
        gather_matches_get("string", &strings, pct, 0);
    }
    let smol: Vec<smol_str::SmolStr> = (0..300).map(|i| smol_str::SmolStr::new(format!("s_{i}"))).collect();
    gather_matches_get("smolstr", &smol, 35, 0);
}

#[test]
fn gather_bytes_match_get() {
    let blobs: Vec<serde_bytes::ByteBuf> =
        (0..300).map(|i| serde_bytes::ByteBuf::from(vec![(i % 256) as u8; 8 + i % 3])).collect();
    for pct in [1, 50, 100] {
        gather_matches_get("bytebuf", &blobs, pct, 0);
    }
}

#[test]
fn gather_json_values_match_get_across_chunks() {
    let values: Vec<serde_json::Value> = (0..300)
        .map(|i| serde_json::json!({ "id": i, "tag": if i % 3 == 0 { "a" } else { "b" }, "nested": [i, i + 1] }))
        .collect();
    for pct in [1, 25, 50, 100] {
        gather_matches_get("json", &values, pct, 0);
    }
}

/// Survivors pinned exactly on chunk edges (128-row chunks, 64-bit words).
/// The first chunk holds no survivor so gather must `skip_chunk` it; the
/// rest sit on chunk starts, chunk ends, and the final partial chunk.
#[test]
fn gather_text_survivors_on_chunk_edges() {
    let strings: Vec<String> = (0..300).map(|i| format!("v{i}")).collect();
    let bytes = String::write(strings, 0, Default::default()).unwrap();
    let rows: Vec<usize> = vec![0, 63, 64, 127, 128, 129, 191, 192, 255, 256, 299];
    let bits: Vec<bool> = (0..300).map(|i| rows.contains(&i)).collect();
    let mask = FilterMask::from_bool_slice(&bits);
    let mut gathered = Vec::new();
    let mut reader = <String as PcoSerde>::read(&mut Cursor::new(&bytes[..]), 0, Default::default()).unwrap();
    <String as PcoSerde>::gather(&mut reader, &mask, &mut gathered).unwrap();
    let expected: Vec<String> = rows.iter().map(|i| format!("v{i}")).collect();
    assert_eq!(gathered, expected);
}

/// `pop_inner` calls before `gather` prime the chunk cache mid-column; the
/// gather must drain that cache first and resume the forward scan after it.
/// The forward-only cursor cannot rewind, so rows before the primed cache
/// answer `Default` (matching what `get` returns from the same primed
/// state), and rows at or past `total_rows` stay `Default`.
#[test]
fn gather_text_match_get_after_cursor_prime() {
    let strings: Vec<String> = (0..300).map(|i| format!("row_{i}")).collect();
    let bytes = String::write(strings.clone(), 0, Default::default()).unwrap();
    let mut rng = Rng(0x5EED_1234_BEEF_0BAD);
    let mask = random_mask(300, 20, &mut rng);
    let mut primed = <String as PcoSerde>::read(&mut Cursor::new(&bytes[..]), 0, Default::default()).unwrap();
    // Prime the cache onto chunk 1 (rows 128..256).
    <String as PcoSerde>::get(&mut primed, 130).unwrap();
    let mut gathered = Vec::new();
    <String as PcoSerde>::gather(&mut primed, &mask, &mut gathered).unwrap();

    let mut expected: Vec<String> = Vec::new();
    for_each_set_bit(&mask, |row| {
        // Rows below the primed chunk are unreachable for a forward cursor;
        // `get` answers None there and the gather pushes `Default`.
        let value = if row < 128 { String::new() } else { strings.get(row).cloned().unwrap_or_default() };
        expected.push(value);
        Ok(())
    })
    .unwrap();
    assert_eq!(gathered, expected);
    // Sanity: the mask really selects rows below, inside, and past chunk 1.
    let below = mask.as_raw_slice()[..2].iter().map(|w| w.count_ones()).sum::<u32>();
    assert!(below > 0 && mask.count_ones() as u32 > below);
}

/// Survivor bits at or past `total_rows` push `Default` exactly like `get`
/// returning `None`, even though the chunk stream ends long before them.
#[test]
fn gather_text_past_row_count_defaults_like_get() {
    let strings: Vec<String> = (0..300).map(|i| format!("v{i}")).collect();
    let bytes = String::write(strings, 0, Default::default()).unwrap();
    let mut bits = vec![false; 8 * 64];
    bits[1] = true;
    bits[3] = true;
    bits[4 * 64 + 62] = true; // row 318, well past the 300-row column
    let mask = FilterMask::from_bool_slice(&bits);
    let mut gathered = Vec::new();
    let mut reader = <String as PcoSerde>::read(&mut Cursor::new(&bytes[..]), 0, Default::default()).unwrap();
    <String as PcoSerde>::gather(&mut reader, &mask, &mut gathered).unwrap();
    let expected = expected_via_get::<String>(&mut reader, &mask);
    assert_eq!(gathered, expected);
    assert_eq!(gathered.len(), 3);
    assert_eq!(&gathered[..2], &["v1".to_string(), "v3".to_string()]);
    assert_eq!(gathered[2..], [String::new()]);
}

/// Cutting the decompressed msgpack stream mid-chunk-1 keeps chunk 0 fully
/// readable while any survivor that needs the missing region is corrupt data:
/// the gather must return `Err`, not zero-fill the survivors.
#[test]
fn gather_text_stream_truncation_errors() {
    let strings: Vec<String> = (0..300).map(|i| format!("v{i}")).collect();
    let bytes = String::write(strings, 0, Default::default()).unwrap();
    let total_rows = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let compressed_len = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let raw = zstd::decode_all(Cursor::new(&bytes[16..16 + compressed_len])).unwrap();
    // Halving the payload keeps the version byte and chunk 0 (rows 0..128)
    // and dies partway into chunk 1 (rows 128..256).
    let raw = &raw[..raw.len() / 2];
    let compressed = zstd::encode_all(raw, 3).unwrap();
    let mut blob = Vec::new();
    blob.extend_from_slice(&total_rows.to_le_bytes());
    blob.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
    blob.extend_from_slice(&compressed);

    let read_column = || <String as PcoSerde>::read(&mut Cursor::new(&blob[..]), 0, Default::default()).unwrap();

    // Chunk 0 survived the cut, so rows below it still read exactly.
    let mut bits = vec![false; 300];
    bits[10] = true;
    bits[127] = true;
    let mask = FilterMask::from_bool_slice(&bits);
    let mut gathered = Vec::new();
    <String as PcoSerde>::gather(&mut read_column(), &mask, &mut gathered).unwrap();
    assert_eq!(gathered, vec!["v10".to_string(), "v127".to_string()]);

    // A survivor inside the truncated chunk fails the gather.
    let mut bits = vec![false; 300];
    bits[150] = true;
    let mask = FilterMask::from_bool_slice(&bits);
    let mut gathered = Vec::new();
    let err = <String as PcoSerde>::gather(&mut read_column(), &mask, &mut gathered).unwrap_err();
    assert!(err.to_string().contains("is corrupt"), "{err}");

    // Skipping past the truncated chunk toward a later row fails too.
    let mut bits = vec![false; 300];
    bits[260] = true;
    let mask = FilterMask::from_bool_slice(&bits);
    let mut gathered = Vec::new();
    let err = <String as PcoSerde>::gather(&mut read_column(), &mask, &mut gathered).unwrap_err();
    assert!(err.to_string().contains("is corrupt"), "{err}");

    // An all-set mask fails on its first survivor past the cut rather than
    // zero-filling the tail.
    let mask = FilterMask::ones(300);
    let mut gathered = Vec::new();
    let err = <String as PcoSerde>::gather(&mut read_column(), &mask, &mut gathered).unwrap_err();
    assert!(err.to_string().contains("is corrupt"), "{err}");
}

#[test]
fn gather_option_container_match_get() {
    let opts: Vec<Option<i32>> = (0..257_i32).map(|i| if i % 3 == 0 { Some(i) } else { None }).collect();
    for pct in [1, 25, 100] {
        gather_matches_get("option", &opts, pct, 0);
    }
}

#[test]
fn gather_vec_container_match_get() {
    let vecs: Vec<Vec<i32>> = (0..257_i32).map(|i| (0..(i % 5 + 1)).map(|j| i * 7 + j).collect()).collect();
    for pct in [1, 25, 100] {
        gather_matches_get("vec", &vecs, pct, 0);
    }
}

/// A map's projected inner entry mask's word span can end below a trailing
/// unselected row's entries, so that row extends the value column's row
/// space past the mask's words. The gather must clamp there and still answer
/// every selected row exactly like `get`.
#[test]
fn gather_map_of_vecs_clamps_unselected_entry_tail() {
    let mut maps: Vec<HashMap<SmolStr, Vec<i32>>> =
        (0..3).map(|i| (0..3).map(|j| (SmolStr::new(format!("k{i}{j}")), vec![i * 3 + j])).collect()).collect();
    let big: HashMap<SmolStr, Vec<i32>> = (0..70).map(|j| (SmolStr::new(format!("k3_{j}")), vec![j])).collect();
    maps.push(big);
    let bytes = HashMap::<SmolStr, Vec<i32>>::write(maps.clone(), 0, Default::default()).unwrap();
    let mask = FilterMask::from_bool_slice(&[true, true, false, false]);
    let mut gathered = Vec::new();
    let mut reader =
        <HashMap<SmolStr, Vec<i32>> as PcoSerde>::read(&mut Cursor::new(&bytes[..]), 0, Default::default()).unwrap();
    <HashMap<SmolStr, Vec<i32>> as PcoSerde>::gather(&mut reader, &mask, &mut gathered).unwrap();
    assert_eq!(gathered.len(), 2);
    assert_eq!(gathered, maps[..2]);
}

#[test]
fn gather_map_of_vecs_match_get_across_selectivities() {
    let maps: Vec<HashMap<SmolStr, Vec<i32>>> = (0..200_i32)
        .map(|i| (0..(i % 9)).map(|j| (SmolStr::new(format!("k{i}_{j}")), vec![i * 7 + j])).collect())
        .collect();
    for pct in [1, 25, 100] {
        gather_matches_get("map_of_vecs", &maps, pct, 0);
    }
}

#[test]
fn gather_option_of_map_of_vecs_match_get() {
    let maps: Vec<Option<HashMap<SmolStr, Vec<i32>>>> = (0..200)
        .map(|i| {
            if i % 4 == 0 {
                None
            } else {
                Some((0..(i % 11)).map(|j| (SmolStr::new(format!("k{i}_{j}")), vec![i + j])).collect())
            }
        })
        .collect();
    for pct in [5, 30, 100] {
        gather_matches_get("option_map_of_vecs", &maps, pct, 0);
    }
}

/// Sparse survivor sets over nested containers, on odd row counts: the
/// projection and clamp paths see masks where almost nothing (1%) and a
/// quarter (25%) of the rows survive.
#[test]
fn gather_sparse_containers_match_get() {
    let opts: Vec<Option<Vec<i64>>> = (0..257_i64)
        .map(|i| if i % 5 == 0 { Some((0..(i % 4 + 1)).map(|j| i * 13 + j).collect()) } else { None })
        .collect();
    for pct in [1, 25] {
        gather_matches_get("option_vec_i64", &opts, pct, 0);
    }

    let vecs: Vec<Vec<Option<String>>> = (0..131)
        .map(|i| (0..(i % 3 + 1)).map(|j| if (i + j) % 4 == 0 { None } else { Some(format!("v{i}_{j}")) }).collect())
        .collect();
    for pct in [1, 25] {
        gather_matches_get("vec_option_string", &vecs, pct, 0);
    }
}

/// A buffer shorter than the row count pushes `Default` past its end, byte
/// for byte like `pop_inner` over the same index range. This is the
/// malformed-truncation path, so it is pinned against a hand-built reader.
#[test]
fn gather_short_buffer_defaults_like_get() {
    let full: Vec<i64> = (0..512).collect();
    let mut rng = Rng(0x00C0_FFEE_1234_5678);
    for _round in 0..8 {
        let mut reader =
            NumberReader { values: Arc::from(full[..rng.next_u64() as usize % full.len()].to_vec()), index: 0 };
        let mask = random_mask(512, 50, &mut rng);
        let mut gathered = Vec::new();
        i64::gather(&mut reader, &mask, &mut gathered).unwrap();
        let expected = expected_via_get::<i64>(&mut reader, &mask);
        assert_eq!(gathered, expected);
    }
}

/// The trait default drives every container reader; an empty `get` path still
/// produces exactly one entry per set bit so row assembly never desyncs.
#[test]
fn gather_absent_column_pushes_default_per_survivor() {
    let bytes: Vec<u8> = Vec::new();
    let mut reader = i64::read(&mut Cursor::new(&bytes[..]), 0, Default::default()).unwrap();
    // Three survivors in the first word plus all 64 in the second: 67 rows,
    // every one answered by the trait default.
    let bits: Vec<bool> = (0..128).map(|i| matches!(i, 0 | 1 | 3) || i >= 64).collect();
    let mask = FilterMask::from_bool_slice(&bits);
    let mut gathered = Vec::new();
    i64::gather(&mut reader, &mask, &mut gathered).unwrap();
    assert_eq!(gathered, vec![0i64; 67]);
}

/// Tuple gathers delegate each slot to its own gather over the same row mask.
/// Every arity in `src/tuple.rs` is pinned against per-row `get`. `bool`
/// columns carry no length prefix, so their reader must finish the tuple's
/// byte stream; every test places `bool` in the last slot.
#[test]
fn gather_tuples_match_get_across_arities() {
    let two: Vec<(i64, String)> = (0..130).map(|i| (i, format!("t{i}"))).collect();
    for pct in [1, 40, 100] {
        gather_matches_get("tuple2", &two, pct, 0);
    }
    let three: Vec<(f64, u32, bool)> = (0..130).map(|i| (i as f64 / 2.0, i * 5, i % 3 == 0)).collect();
    for pct in [1, 40, 100] {
        gather_matches_get("tuple3", &three, pct, 0);
    }
    let four: Vec<(u32, SmolStr, f32, i8)> =
        (0..70).map(|i| (i * 3, SmolStr::new(format!("k{i}")), i as f32, (i % 127) as i8 - 60)).collect();
    gather_matches_get("tuple4", &four, 45, 0);
    let five: Vec<(i64, Option<i64>, serde_json::Value, u16, bool)> = (0..70)
        .map(|i| (i, if i % 4 == 0 { None } else { Some(i * 2) }, serde_json::json!([i]), i as u16, i % 2 == 0))
        .collect();
    gather_matches_get("tuple5", &five, 45, 0);
    let six: Vec<(i16, f64, SmolStr, u8, i64, bool)> = (0..66)
        .map(|i| (i as i16 * 3, i as f64 * 0.25, SmolStr::new(format!("s{i}")), (i % 256) as u8, -i, i % 5 < 2))
        .collect();
    gather_matches_get("tuple6", &six, 45, 0);
}

/// A `gather` between two `pop_inner` reads on a public `LazyReader` must
/// leave the reader coherent: the third read sees exactly what the second
/// read would have seen had the gather not happened. The chunked gathers
/// drain their cached chunk, so without the cursor write-back the forward
/// scan would resume mid-stream with a stale `data_pos` and mis-align row
/// offsets. Covers a lazy-msgpack text column, a buffer-backed numeric
/// column, an `Option` column (whose inner text cursor is primed through
/// the container), and a JSON column.
#[test]
fn lazy_reader_get_gather_get_sees_what_get_alone_would() {
    let strings: Vec<String> = (0..300).map(|i| format!("v{i}")).collect();
    get_gather_get_check("string", &strings, 5, &[128, 190, 250], 290, 0);

    let nums: Vec<i64> = (0..300).map(|i| i as i64 * 3).collect();
    get_gather_get_check("i64", &nums, 5, &[128, 190, 250], 290, 0);

    let opts: Vec<Option<String>> = (0..300).map(|i| if i % 3 == 0 { None } else { Some(format!("o{i}")) }).collect();
    get_gather_get_check("option_string", &opts, 5, &[128, 190, 250], 290, 0);

    let json: Vec<serde_json::Value> = (0..300).map(|i| serde_json::json!({ "i": i, "s": format!("j{i}") })).collect();
    get_gather_get_check("json", &json, 5, &[128, 190, 250], 290, 0);
}

/// The `Option::gather` projected-mask guard, pinned: a survivor set that
/// hits only `None` rows must leave the inner vec reader untouched. Without
/// the guard the inner scan runs on an all-zero projection and drives
/// `VecNumberReader`'s row cursor to the projection's word span; the third
/// read then works from a stale element offset and errors "inner missing"
/// instead of returning the present row an uninterrupted reader sees.
#[test]
fn lazy_reader_get_gather_get_all_none_survivors() {
    let rows: Vec<Option<Vec<i32>>> =
        (0..300).map(|i| if i % 3 == 0 { Some(vec![i, i + 1, i + 2]) } else { None }).collect();
    // 130, 190, and 250 are all `None` past the primed row; 297 is present.
    get_gather_get_check("option_vec_all_none", &rows, 5, &[130, 190, 250], 297, 0);
}

fn get_gather_get_check<T>(
    label: &str, values: &[T], prime: usize, gather_rows: &[usize], third: usize, float_round: u32,
) where
    T: PcoSerde + PcoFilter + Default + PartialEq + Debug + Clone,
    T::Reader: Clone + Default,
{
    let rows = values.len();
    let bytes = T::write(values.to_vec(), float_round, Default::default()).unwrap();
    let bits: Vec<bool> = (0..rows).map(|i| gather_rows.contains(&i)).collect();
    let mask = FilterMask::from_bool_slice(&bits);

    // Reference: `prime`, then `third`, with no gather in between.
    let mut reference = LazyReader::<T>::new(bytes.clone(), float_round, Default::default());
    assert_eq!(reference.pop_inner(prime).unwrap(), values[prime].clone(), "{label}: reference prime");
    let reference_third = reference.pop_inner(third).unwrap();

    // Subject: `prime`, a `gather_into` spanning chunk boundaries, then the
    // same third read.
    let mut subject = LazyReader::<T>::new(bytes.clone(), float_round, Default::default());
    assert_eq!(subject.pop_inner(prime).unwrap(), values[prime].clone(), "{label}: subject prime");
    let mut survivors = Vec::new();
    subject.gather_into(&mask, &mut survivors).unwrap();
    let mut fresh = LazyReader::<T>::new(bytes, float_round, Default::default());
    let expected_survivors: Vec<T> = gather_rows.iter().map(|&r| fresh.pop_inner(r).unwrap()).collect();
    assert_eq!(survivors, expected_survivors, "{label}: gather contents");

    // Sanity: the uninterrupted reference sees the true row, and so must the
    // subject after its gather.
    assert_eq!(reference_third, values[third].clone(), "{label}: reference third");
    let subject_third = subject.pop_inner(third).unwrap();
    assert_eq!(subject_third, reference_third, "{label}: third read after gather");
}
