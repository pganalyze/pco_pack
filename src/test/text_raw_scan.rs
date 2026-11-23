//! The raw-bytes text scan (`lazy_msgpack::helpers::scan_chunk`): every
//! chunk it accepts must land exactly the same predicate bits as the
//! deserialize-and-predicate path, byte-for-byte across the mask. Chunks it
//! declines (non-string elements) leave `pos` untouched so the caller's
//! fallback re-reads from the chunk start. Nil is accepted grammar and never
//! matches, agreeing with `Option::filter_match`, whose `None` arm is false.
//! Hand-built blob families below force each decline trigger (oversize array,
//! short mask, truncation, foreign mid-stream byte) and assert the decode
//! fallback lands the same bits.
use crate::lazy_msgpack::{self, TextTargets};
use crate::{Filter, FilterMask, PcoFilter, PcoSerde};

fn encoded_chunk(values: &[&str]) -> Vec<u8> {
    let mut buf = Vec::new();
    rmp_serde::encode::write(&mut buf, values).unwrap();
    buf
}

/// One msgpack array over element payloads where `None` encodes as nil, so
/// tests can hand the scan a grammar a `String` column's writer never emits.
fn encode_mixed(values: &[Option<&str>]) -> Vec<u8> {
    let mut body = Vec::new();
    for v in values {
        match v {
            Some(s) => rmp_serde::encode::write(&mut body, s).unwrap(),
            None => body.push(0xc0),
        }
    }
    let mut out = Vec::new();
    if values.len() < 16 {
        out.push(0x90 + values.len() as u8);
    } else {
        out.push(0xdc);
        out.extend_from_slice(&(values.len() as u16).to_be_bytes());
    }
    out.extend(body);
    out
}

/// Array32 header over already-encoded string payloads: claims whatever row
/// count it's handed, including the CHUNK_SIZE+1 the writer never emits and
/// the scan must refuse.
fn array32_over(values: &[&str]) -> Vec<u8> {
    let mut out = vec![0xdd];
    out.extend_from_slice(&(values.len() as u32).to_be_bytes());
    for v in values {
        rmp_serde::encode::write(&mut out, v).unwrap();
    }
    out
}

/// Whole-frame blob around pre-encoded msgpack chunks:
/// `[u64 rows][u64 block_len][zstd([1][chunks])]`, the exact layout
/// `write()` produces, so hand-rolled element streams can drive the reader's
/// chunk walk.
fn raw_frame(total_rows: u64, chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut raw = vec![1u8];
    for c in chunks {
        raw.extend_from_slice(c);
    }
    let compressed = zstd::encode_all(raw.as_slice(), 3).unwrap();
    let mut out = Vec::new();
    out.extend_from_slice(&total_rows.to_le_bytes());
    out.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
    out.extend_from_slice(&compressed);
    out
}

fn reader_of(bytes: &[u8]) -> <String as PcoSerde>::Reader {
    let mut cursor = std::io::Cursor::new(bytes);
    <String as PcoSerde>::read(&mut cursor, 0, chrono::Duration::zero()).unwrap()
}

/// Strings straddling every msgpack string header width: fixstr (0..=31),
/// str8 (32..=255), str16 (256..=65535), str32 (65536..).
fn boundary_values() -> Vec<String> {
    vec![
        String::new(),
        "a".into(),
        "b".repeat(31),
        "c".repeat(32),
        "d".repeat(255),
        "e".repeat(256),
        "f".repeat(65_535),
        "g".repeat(65_536),
    ]
}

#[test]
fn scan_chunk_lands_bits_exactly_like_deserialize() {
    let strs = boundary_values();
    let values: Vec<&str> = strs.iter().map(String::as_str).collect();
    let bytes = encoded_chunk(&values);
    let n = values.len();
    for target in [values[0], values[1], values[3], values[4], values[6], values[7], "absent"] {
        let mut scanned = FilterMask::ones(n);
        let mut pos = 0usize;
        let rows =
            lazy_msgpack::helpers::scan_chunk(&bytes, &mut pos, &TextTargets::Eq(target.as_bytes()), 0, &mut scanned)
                .unwrap();
        assert_eq!(rows, n, "chunk carries {n} rows");
        assert_eq!(pos, bytes.len(), "scan consumed the whole chunk");

        let mut truth = FilterMask::ones(n);
        truth.build_into(0, &strs, |v| v.as_str() == target);
        assert_eq!(scanned.as_raw_slice(), truth.as_raw_slice(), "eq {target}");
    }
}

#[test]
fn scan_chunk_in_targets_land_bits_exactly() {
    let strs = boundary_values();
    let values: Vec<&str> = strs.iter().map(String::as_str).collect();
    let bytes = encoded_chunk(&values);
    let mut set: ahash::HashSet<&[u8]> = ahash::HashSet::default();
    set.insert(values[0].as_bytes()); // the empty string is a valid IN target
    set.insert(values[5].as_bytes());

    let mut scanned = FilterMask::ones(values.len());
    let mut pos = 0usize;
    lazy_msgpack::helpers::scan_chunk(&bytes, &mut pos, &TextTargets::In(set), 0, &mut scanned).unwrap();

    let mut truth = FilterMask::ones(values.len());
    truth.build_into(0, &strs, |v| v.is_empty() || v.len() == 256);
    assert_eq!(scanned.as_raw_slice(), truth.as_raw_slice(), "IN {{\"\", 256-bytes}}");
}

#[test]
fn scan_chunk_declines_non_string_elements_and_keeps_pos() {
    // fixarray(2) [ "ab", 5 ]: the int declines, pos untouched.
    let bytes = [0x92u8, 0xa2, b'a', b'b', 0x05];
    let mut matches = FilterMask::ones(2);
    let mut pos = 0usize;
    assert_eq!(lazy_msgpack::helpers::scan_chunk(&bytes, &mut pos, &TextTargets::Eq(b"ab"), 0, &mut matches), None);
    assert_eq!(pos, 0, "a declined chunk cannot move the cursor");

    // fixarray(2) [ nil, "ab" ]: nil is accepted grammar and never matches.
    let bytes = [0x92u8, 0xc0, 0xa2, b'a', b'b'];
    let mut matches = FilterMask::ones(2);
    let mut pos = 0usize;
    let rows = lazy_msgpack::helpers::scan_chunk(&bytes, &mut pos, &TextTargets::Eq(b"ab"), 0, &mut matches).unwrap();
    assert_eq!(rows, 2);
    assert_eq!(pos, bytes.len());
    assert_eq!(matches.as_raw_slice()[0] & 0b11, 0b10, "row 0 is nil: cleared; row 1 matches");
}

#[test]
fn whole_frame_filter_bulk_scans_mixed_length_rows() {
    // Whole-frame layout with strings straddling header widths; unique
    // values keep the blob guard unhelpful, so every chunk reaches
    // the scan.
    let mut data: Vec<String> = Vec::new();
    for i in 0..400usize {
        data.push(if i % 50 == 0 { format!("fat-{i}-{}", "z".repeat(300)) } else { format!("s{i}") });
    }
    let n = data.len();
    let bytes = <String as PcoSerde>::write(data.clone(), 0, chrono::Duration::zero()).unwrap();
    assert_eq!(bytes[16], 0x28, "sub-frame-count blobs stay whole-frame zstd");

    let targets: Vec<String> = vec!["s1".into(), "s399".into(), data[350].clone(), "absent-entirely".into()];
    for target in &targets {
        let mut matches = FilterMask::ones(n);
        <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String(target.clone()), &mut matches)
            .unwrap();
        let truth: Vec<usize> = (0..n).filter(|&i| &data[i] == target).collect();
        assert_eq!(matches.count_ones(), truth.len(), "eq {target}");
        for row in truth {
            assert!(matches.any_set_in_range(row..row + 1), "row {row} must survive eq {target}");
        }
    }
}

#[test]
fn scan_preserves_mask_padding_beyond_total_rows() {
    // 200 rows: the final chunk fills only part of the last word. Pad that
    // word's invalid bits with ones, then verify the scan leaves them set
    // while landing exactly the predicate bits below.
    let data: Vec<String> = (0..200usize).map(|i| if i % 7 == 0 { "hit".into() } else { "pass".into() }).collect();
    let n = data.len();
    let bytes = <String as PcoSerde>::write(data.clone(), 0, chrono::Duration::zero()).unwrap();

    let mut mask = FilterMask::ones(n);
    let padding_bits = (64 - n % 64) % 64;
    assert!(padding_bits > 0, "test needs a word-truncated tail");
    *mask.as_raw_mut_slice().last_mut().unwrap() |= !((1u64 << (n % 64)) - 1);

    <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String("hit".into()), &mut mask).unwrap();

    let truth: usize = data.iter().filter(|v| v.as_str() == "hit").count();
    assert_eq!(mask.count_ones(), truth);
    let padding_mask = !((1u64 << (n % 64)) - 1);
    assert_eq!(mask.as_raw_slice().last().unwrap() & padding_mask, padding_mask, "every padding bit stays poisoned");
    for (row, value) in data.iter().enumerate() {
        assert_eq!(mask.any_set_in_range(row..row + 1), value == "hit", "row {row}");
    }
}

#[test]
fn scan_commits_final_single_row_chunk_and_preserves_padding() {
    // 129 rows: the tail chunk carries n = 1, a commit-side write of one
    // predicate bit into a word whose other 63 bits are padding (the
    // 200-row test above only exercises the preserve side). Poison the
    // padding, run the scan, and require raw-word equality with a
    // decoded build_into of the same poison: the committed bit lands and
    // no padding bit is disturbed, whether the tail chunk matches or not.
    let data: Vec<String> = (0..129usize).map(|i| if i == 128 { "hit".into() } else { format!("row-{i}") }).collect();
    let n = data.len();
    let bytes = <String as PcoSerde>::write(data.clone(), 0, chrono::Duration::zero()).unwrap();
    let padding_mask = !((1u64 << (n % 64)) - 1);

    for target in ["hit", "row-5"] {
        let mut mask = FilterMask::ones(n);
        *mask.as_raw_mut_slice().last_mut().unwrap() |= padding_mask;
        <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String(target.into()), &mut mask)
            .unwrap();

        let mut truth = FilterMask::ones(n);
        *truth.as_raw_mut_slice().last_mut().unwrap() |= padding_mask;
        truth.build_into(0, &data, |v| v.as_str() == target);
        assert_eq!(mask.as_raw_slice(), truth.as_raw_slice(), "eq {target} across a 1-row tail chunk");
        assert_eq!(
            mask.as_raw_slice().last().unwrap() & padding_mask,
            padding_mask,
            "padding survives the 1-row commit"
        );
    }
}

#[test]
fn scan_matches_decoded_predicate_on_mixed_nil_string_column() {
    // A nil + string element stream straddling the word boundary, compared
    // against the decoded predicate over the same rows with
    // Option::filter_match semantics: a nil row answers no, every string
    // row answers the eq.
    let s31 = "y".repeat(31);
    let s200 = "z".repeat(200);
    let values: Vec<Option<&str>> = (0..70)
        .map(|i| match i % 5 {
            0 => None,
            1 => Some(""),
            2 => Some("needle"),
            3 => Some(s31.as_str()),
            _ => Some(s200.as_str()),
        })
        .collect();
    let bytes = encode_mixed(&values);
    for target in ["needle", "", "absent"] {
        let mut scanned = FilterMask::ones(values.len());
        let mut pos = 0usize;
        let rows =
            lazy_msgpack::helpers::scan_chunk(&bytes, &mut pos, &TextTargets::Eq(target.as_bytes()), 0, &mut scanned)
                .unwrap();
        assert_eq!(rows, values.len());
        assert_eq!(pos, bytes.len(), "scan consumed the whole mixed chunk");

        let mut truth = FilterMask::ones(values.len());
        truth.build_into(0, &values, |v| v.is_some_and(|s| s == target));
        assert_eq!(scanned.as_raw_slice(), truth.as_raw_slice(), "eq {target} over mixed column");
    }
}

#[test]
fn scan_walks_past_nils_to_a_later_target() {
    // A CHUNK_SIZE-row chunk with 5 leading nils, a pure-string chunk, and a
    // tail chunk whose last row is the target. Every row start stays
    // word-aligned (0, 128, 256), so the scan takes each chunk: the nils
    // must clear their own rows (they start set, so a cleared bit proves the
    // scan wrote them) without ending the walk, and the target three chunks
    // in must still land. Had nil declined instead, the decode path would
    // fail on the nil and stop at chunk 0, leaving the whole mask set.
    let fillers: Vec<String> = (0..123).map(|i| format!("filler-{i}")).collect();
    let junk: Vec<String> = (0..128).map(|i| format!("junk-{i}")).collect();
    let tails: Vec<String> = (0..59).map(|i| format!("tail-{i}")).collect();
    let mut chunk0: Vec<Option<&str>> = vec![None; 5];
    chunk0.extend(fillers.iter().map(|s| Some(s.as_str())));
    let junk_refs: Vec<&str> = junk.iter().map(String::as_str).collect();
    let mut chunk2: Vec<Option<&str>> = tails.iter().map(|s| Some(s.as_str())).collect();
    chunk2.push(Some("needle"));
    let n = 316usize;
    let bytes = raw_frame(n as u64, &[encode_mixed(&chunk0), encoded_chunk(&junk_refs), encode_mixed(&chunk2)]);

    let mut mask = FilterMask::ones(n);
    <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String("needle".into()), &mut mask).unwrap();

    assert_eq!(mask.count_ones(), 1, "only the later non-nil target survives");
    assert!(mask.any_set_in_range(315..316), "the walk must reach the target row");
    for row in 0..5 {
        assert!(!mask.any_set_in_range(row..row + 1), "nil row {row} must be cleared, never matched");
    }
    for row in 5..315 {
        assert!(!mask.any_set_in_range(row..row + 1), "non-target row {row} must be cleared");
    }
}

#[test]
fn oversize_and_short_mask_declines_land_decode_identical_bits() {
    // A second chunk whose array32 header claims CHUNK_SIZE + 1 rows: the
    // writer never emits it and the scan refuses it, so the decode path
    // takes the chunk whole and must match a full deserialize-and-predicate
    // read of the blob, first chunk scanned as usual.
    let strs: Vec<String> = (0..257).map(|i| format!("v{i}")).collect();
    let head: Vec<&str> = strs[..128].iter().map(String::as_str).collect();
    let over: Vec<&str> = strs[128..].iter().map(String::as_str).collect();
    assert_eq!(over.len(), 129);
    let bytes = raw_frame(257, &[encoded_chunk(&head), array32_over(&over)]);
    for target in ["v0", "v200"] {
        let mut mask = FilterMask::ones(257);
        <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String(target.into()), &mut mask)
            .unwrap();
        let mut truth = FilterMask::ones(257);
        truth.build_into(0, &strs, |v| v.as_str() == target);
        assert_eq!(mask.as_raw_slice(), truth.as_raw_slice(), "eq {target} across the oversize chunk");
    }

    // A mask shorter than the first chunk's 128 rows: the chunk runs past
    // the mask's last row, the scan declines it, and the decode path's
    // clamped writes take it.
    let one_chunk = raw_frame(128, &[encoded_chunk(&head)]);
    for len in [100, 37] {
        let mut mask = FilterMask::ones(len);
        <String as PcoFilter>::filter_bulk(&mut reader_of(&one_chunk), 0, &Filter::String("v99".into()), &mut mask)
            .unwrap();
        let mut truth = FilterMask::ones(len);
        truth.build_into(0, &strs, |v| v.as_str() == "v99");
        assert_eq!(mask.as_raw_slice(), truth.as_raw_slice(), "eq v99 with mask len {len}");
    }
}

#[test]
fn truncated_mid_stream_declines_and_breaks_like_decode() {
    // The first chunk scans as usual, then the blob goes bad: the second
    // chunk's bytes are cut mid-payload. The scan declines with `pos`
    // intact, the decode path then fails on the same bytes and stops, so
    // rows past the anomaly keep their prior mask bits exactly as they
    // would with no scan at all.
    let strs: Vec<String> = (0..160).map(|i| format!("v{i}")).collect();
    let head: Vec<&str> = strs[..128].iter().map(String::as_str).collect();
    let tail: Vec<&str> = strs[128..].iter().map(String::as_str).collect();
    let mut truncated = encoded_chunk(&tail);
    truncated.truncate(truncated.len() - 20);
    let bytes = raw_frame(160, &[encoded_chunk(&head), truncated]);
    for target in ["v100", "v140"] {
        let mut mask = FilterMask::ones(160);
        <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String(target.into()), &mut mask)
            .unwrap();
        let mut truth = FilterMask::ones(160);
        truth.build_into(0, &strs[..128], |v| v.as_str() == target);
        assert_eq!(mask.as_raw_slice(), truth.as_raw_slice(), "eq {target} across a truncated tail");
    }
}

#[test]
fn foreign_mid_stream_byte_declines_like_decode() {
    // A fixint byte where the second chunk's array header should be: neither
    // pass interprets it, and both stop after the first chunk with the same
    // mask.
    let strs: Vec<String> = (0..160).map(|i| format!("v{i}")).collect();
    let head: Vec<&str> = strs[..128].iter().map(String::as_str).collect();
    let mut foreign = encoded_chunk(&head);
    foreign.push(0x05);
    let bytes = raw_frame(160, &[foreign]);
    let mut mask = FilterMask::ones(160);
    <String as PcoFilter>::filter_bulk(&mut reader_of(&bytes), 0, &Filter::String("v100".into()), &mut mask).unwrap();
    let mut truth = FilterMask::ones(160);
    truth.build_into(0, &strs[..128], |v| v.as_str() == "v100");
    assert_eq!(mask.as_raw_slice(), truth.as_raw_slice(), "walk stops at the foreign byte, as decode does");
}

#[test]
fn scan_filter_bulk_leaves_the_reader_fresh_for_gather_and_get() {
    // A 400-row blob spanning three scan chunks. The scan pass must never
    // advance the reader's forward cursor or chunk cache, so gathers and
    // gets on the scanned reader answer exactly what a freshly-opened
    // reader answers: gather, get a survivor, then gather again and the
    // second gather must still see it. If filter_bulk ever takes &mut self
    // and leaves cursor state behind, the repeats diverge from the fresh
    // reader.
    let mut data: Vec<String> = Vec::new();
    for i in 0..400usize {
        data.push(if i % 50 == 0 { format!("fat-{i}-{}", "z".repeat(300)) } else { format!("s{i}") });
    }
    let n = data.len();
    let bytes = <String as PcoSerde>::write(data.clone(), 0, chrono::Duration::zero()).unwrap();

    let mut matches = FilterMask::ones(n);
    let mut reader = reader_of(&bytes);
    <String as PcoFilter>::filter_bulk(&mut reader, 0, &Filter::String("s249".into()), &mut matches).unwrap();
    assert_eq!(matches.count_ones(), 1);
    let survivor = (0..n).find(|&r| matches.any_set_in_range(r..r + 1)).unwrap();

    let mut gathered: Vec<String> = Vec::new();
    <String as PcoSerde>::gather(&mut reader, &matches, &mut gathered).unwrap();
    let seen = <String as PcoSerde>::get(&mut reader, survivor).unwrap();
    let mut gathered_again: Vec<String> = Vec::new();
    <String as PcoSerde>::gather(&mut reader, &matches, &mut gathered_again).unwrap();

    let mut fresh = reader_of(&bytes);
    let mut expected: Vec<String> = Vec::new();
    <String as PcoSerde>::gather(&mut fresh, &matches, &mut expected).unwrap();
    assert_eq!(gathered, expected, "gather after a scan matches a fresh reader's");
    assert_eq!(gathered_again, expected, "a second gather after a get still sees the survivor");
    assert_eq!(seen.as_deref(), Some(data[survivor].as_str()), "get after a scan sees the survivor");
}
