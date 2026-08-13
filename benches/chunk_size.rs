include!("bench_common.rs");

use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::PcoPack;
use serde_json::json;

/// Number of rows per test. Large enough for chunk boundaries to affect behavior.
const N_ROWS: usize = 262_144;

/// Small struct (~32 bytes): pure scalars + Option<i32>
macro_rules! small_struct {
    ($name:ident, $chunk:expr) => {
        #[derive(Clone, PcoPack)]
        #[pco_pack(chunk_size = $chunk)]
        pub struct $name {
            id: i64,
            value: f64,
            active: bool,
            tag: Option<i32>,
            color: u8,
        }
    };
}
small_struct!(Small13, 8192);
small_struct!(Small14, 16384);
small_struct!(Small15, 32768);
small_struct!(Small16, 65536);
small_struct!(Small17, 131072);

/// Medium struct (~96 bytes): String + Vec<i32> + scalars
macro_rules! medium_struct {
    ($name:ident, $chunk:expr) => {
        #[derive(Clone, PcoPack)]
        #[pco_pack(chunk_size = $chunk)]
        pub struct $name {
            id: i64,
            label: String,
            value1: f64,
            value2: f32,
            active: bool,
            tag: Option<i32>,
            tags: Vec<i32>,
            color: u8,
        }
    };
}
medium_struct!(Medium13, 8192);
medium_struct!(Medium14, 16384);
medium_struct!(Medium15, 32768);
medium_struct!(Medium16, 65536);
medium_struct!(Medium17, 131072);

/// Large struct (~184 bytes): Strings + multiple Vecs + Options
macro_rules! large_struct {
    ($name:ident, $chunk:expr) => {
        #[derive(Clone, PcoPack)]
        #[pco_pack(chunk_size = $chunk)]
        pub struct $name {
            id: i64,
            name: String,
            score: f64,
            weight: f32,
            active: bool,
            created_at: i64,
            updated_at: Option<i64>,
            category_id: i64,
            tag: Option<String>,
            tags: Vec<i32>,
            scores: Vec<f64>,
            metadata: Option<Vec<u8>>,
            color: u8,
            priority: i16,
        }
    };
}
large_struct!(Large13, 8192);
large_struct!(Large14, 16384);
large_struct!(Large15, 32768);
large_struct!(Large16, 65536);
large_struct!(Large17, 131072);

/// Estimate the total memory footprint (stack + heap) of a single instance.
/// Uses `.capacity()` on String/Vec fields so we count what's actually allocated,
/// not just what's used. This gives a more realistic upper-bound per-row cost
/// for structs with heap-allocated fields.
trait ApproxHeapSize {
    fn approx_total_size(&self) -> usize;
}

// Small variants: no heap fields, so stack-only is exact
impl ApproxHeapSize for Small13 {
    fn approx_total_size(&self) -> usize {
        std::mem::size_of::<Small13>()
    }
}
impl ApproxHeapSize for Small14 {
    fn approx_total_size(&self) -> usize {
        std::mem::size_of::<Small14>()
    }
}
impl ApproxHeapSize for Small15 {
    fn approx_total_size(&self) -> usize {
        std::mem::size_of::<Small15>()
    }
}
impl ApproxHeapSize for Small16 {
    fn approx_total_size(&self) -> usize {
        std::mem::size_of::<Small16>()
    }
}
impl ApproxHeapSize for Small17 {
    fn approx_total_size(&self) -> usize {
        std::mem::size_of::<Small17>()
    }
}

// Medium variants: String + Vec<i32>
impl ApproxHeapSize for Medium13 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Medium13>();
        size += self.label.capacity();
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size
    }
}
impl ApproxHeapSize for Medium14 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Medium14>();
        size += self.label.capacity();
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size
    }
}
impl ApproxHeapSize for Medium15 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Medium15>();
        size += self.label.capacity();
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size
    }
}
impl ApproxHeapSize for Medium16 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Medium16>();
        size += self.label.capacity();
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size
    }
}
impl ApproxHeapSize for Medium17 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Medium17>();
        size += self.label.capacity();
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size
    }
}

// Large variants: multiple Strings + Vecs + Options
impl ApproxHeapSize for Large13 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Large13>();
        size += self.name.capacity();
        if let Some(ref s) = self.tag {
            size += s.capacity();
        }
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size += self.scores.capacity() * std::mem::size_of::<f64>();
        if let Some(ref v) = self.metadata {
            size += v.capacity();
        }
        size
    }
}
impl ApproxHeapSize for Large14 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Large14>();
        size += self.name.capacity();
        if let Some(ref s) = self.tag {
            size += s.capacity();
        }
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size += self.scores.capacity() * std::mem::size_of::<f64>();
        if let Some(ref v) = self.metadata {
            size += v.capacity();
        }
        size
    }
}
impl ApproxHeapSize for Large15 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Large15>();
        size += self.name.capacity();
        if let Some(ref s) = self.tag {
            size += s.capacity();
        }
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size += self.scores.capacity() * std::mem::size_of::<f64>();
        if let Some(ref v) = self.metadata {
            size += v.capacity();
        }
        size
    }
}
impl ApproxHeapSize for Large16 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Large16>();
        size += self.name.capacity();
        if let Some(ref s) = self.tag {
            size += s.capacity();
        }
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size += self.scores.capacity() * std::mem::size_of::<f64>();
        if let Some(ref v) = self.metadata {
            size += v.capacity();
        }
        size
    }
}
impl ApproxHeapSize for Large17 {
    #[inline]
    fn approx_total_size(&self) -> usize {
        let mut size = std::mem::size_of::<Large17>();
        size += self.name.capacity();
        if let Some(ref s) = self.tag {
            size += s.capacity();
        }
        size += self.tags.capacity() * std::mem::size_of::<i32>();
        size += self.scores.capacity() * std::mem::size_of::<f64>();
        if let Some(ref v) = self.metadata {
            size += v.capacity();
        }
        size
    }
}

/// Compute the average total (stack + heap) size per instance from generated data.
fn avg_total_size<T: ApproxHeapSize>(data: &[T]) -> usize {
    if data.is_empty() {
        return 0;
    }
    let sum = data.iter().map(|r| r.approx_total_size()).sum::<usize>();
    sum / data.len()
}

trait FromIndex {
    fn from_index(i: usize) -> Self;
}

// Small variants
impl FromIndex for Small13 {
    fn from_index(i: usize) -> Self {
        Small13 {
            id: (i as i64 / 1000) * 1000,
            value: (i as f64) / 100.0 + ((i % 37) as f64) / 100.0,
            active: i % 3 != 0,
            tag: if i % 2 == 0 { Some((i % 500) as i32) } else { None },
            color: (i % 4) as u8,
        }
    }
}
impl FromIndex for Small14 {
    fn from_index(i: usize) -> Self {
        Small14 {
            id: (i as i64 / 1000) * 1000,
            value: (i as f64) / 100.0 + ((i % 37) as f64) / 100.0,
            active: i % 3 != 0,
            tag: if i % 2 == 0 { Some((i % 500) as i32) } else { None },
            color: (i % 4) as u8,
        }
    }
}
impl FromIndex for Small15 {
    fn from_index(i: usize) -> Self {
        Small15 {
            id: (i as i64 / 1000) * 1000,
            value: (i as f64) / 100.0 + ((i % 37) as f64) / 100.0,
            active: i % 3 != 0,
            tag: if i % 2 == 0 { Some((i % 500) as i32) } else { None },
            color: (i % 4) as u8,
        }
    }
}
impl FromIndex for Small16 {
    fn from_index(i: usize) -> Self {
        Small16 {
            id: (i as i64 / 1000) * 1000,
            value: (i as f64) / 100.0 + ((i % 37) as f64) / 100.0,
            active: i % 3 != 0,
            tag: if i % 2 == 0 { Some((i % 500) as i32) } else { None },
            color: (i % 4) as u8,
        }
    }
}
impl FromIndex for Small17 {
    fn from_index(i: usize) -> Self {
        Small17 {
            id: (i as i64 / 1000) * 1000,
            value: (i as f64) / 100.0 + ((i % 37) as f64) / 100.0,
            active: i % 3 != 0,
            tag: if i % 2 == 0 { Some((i % 500) as i32) } else { None },
            color: (i % 4) as u8,
        }
    }
}

// Medium variants
impl FromIndex for Medium13 {
    fn from_index(i: usize) -> Self {
        Self {
            id: (i as i64 / 100) * 100,
            label: format!("item_{}", i % 500),
            value1: (i as f64) / 10.0 + ((i % 7) as f64) / 100.0,
            value2: ((i * 3) % 1000) as f32 / 10.0,
            active: i % 5 != 0,
            tag: if i % 3 == 0 { Some(i as i32 % 100) } else { None },
            tags: (0..((i % 4) + 1)).map(|j| (i * 10 + j) as i32).collect(),
            color: (i % 8) as u8,
        }
    }
}
impl FromIndex for Medium14 {
    fn from_index(i: usize) -> Self {
        Self {
            id: (i as i64 / 100) * 100,
            label: format!("item_{}", i % 500),
            value1: (i as f64) / 10.0 + ((i % 7) as f64) / 100.0,
            value2: ((i * 3) % 1000) as f32 / 10.0,
            active: i % 5 != 0,
            tag: if i % 3 == 0 { Some(i as i32 % 100) } else { None },
            tags: (0..((i % 4) + 1)).map(|j| (i * 10 + j) as i32).collect(),
            color: (i % 8) as u8,
        }
    }
}
impl FromIndex for Medium15 {
    fn from_index(i: usize) -> Self {
        Self {
            id: (i as i64 / 100) * 100,
            label: format!("item_{}", i % 500),
            value1: (i as f64) / 10.0 + ((i % 7) as f64) / 100.0,
            value2: ((i * 3) % 1000) as f32 / 10.0,
            active: i % 5 != 0,
            tag: if i % 3 == 0 { Some(i as i32 % 100) } else { None },
            tags: (0..((i % 4) + 1)).map(|j| (i * 10 + j) as i32).collect(),
            color: (i % 8) as u8,
        }
    }
}
impl FromIndex for Medium16 {
    fn from_index(i: usize) -> Self {
        Self {
            id: (i as i64 / 100) * 100,
            label: format!("item_{}", i % 500),
            value1: (i as f64) / 10.0 + ((i % 7) as f64) / 100.0,
            value2: ((i * 3) % 1000) as f32 / 10.0,
            active: i % 5 != 0,
            tag: if i % 3 == 0 { Some(i as i32 % 100) } else { None },
            tags: (0..((i % 4) + 1)).map(|j| (i * 10 + j) as i32).collect(),
            color: (i % 8) as u8,
        }
    }
}
impl FromIndex for Medium17 {
    fn from_index(i: usize) -> Self {
        Self {
            id: (i as i64 / 100) * 100,
            label: format!("item_{}", i % 500),
            value1: (i as f64) / 10.0 + ((i % 7) as f64) / 100.0,
            value2: ((i * 3) % 1000) as f32 / 10.0,
            active: i % 5 != 0,
            tag: if i % 3 == 0 { Some(i as i32 % 100) } else { None },
            tags: (0..((i % 4) + 1)).map(|j| (i * 10 + j) as i32).collect(),
            color: (i % 8) as u8,
        }
    }
}

// Large variants
impl FromIndex for Large13 {
    fn from_index(i: usize) -> Self {
        Self {
            id: i as i64,
            name: format!("record_{}_{}", i % 200, i % 5),
            score: (i as f64) / 50.0 + ((i % 11) as f64) / 100.0,
            weight: ((i * 7) % 1000) as f32 / 100.0,
            active: i % 7 != 0,
            created_at: 1_700_000_000 + (i as i64 * 60),
            updated_at: if i % 4 != 0 { Some(1_700_000_000 + (i as i64 * 60) + 300) } else { None },
            category_id: (i / 50) as i64,
            tag: if i % 2 == 0 { Some(format!("tag_{}", i % 20)) } else { None },
            tags: (0..((i % 6) + 1)).map(|j| (i * 7 + j * 3) as i32).collect(),
            scores: vec![(i as f64 / 10.0), ((i + 1) as f64 / 10.0)],
            metadata: if i % 5 != 0 { Some(vec![0xAB, 0xCD, (i % 256) as u8]) } else { None },
            color: (i % 16) as u8,
            priority: ((i % 10) + 1) as i16,
        }
    }
}
impl FromIndex for Large14 {
    fn from_index(i: usize) -> Self {
        Self {
            id: i as i64,
            name: format!("record_{}_{}", i % 200, i % 5),
            score: (i as f64) / 50.0 + ((i % 11) as f64) / 100.0,
            weight: ((i * 7) % 1000) as f32 / 100.0,
            active: i % 7 != 0,
            created_at: 1_700_000_000 + (i as i64 * 60),
            updated_at: if i % 4 != 0 { Some(1_700_000_000 + (i as i64 * 60) + 300) } else { None },
            category_id: (i / 50) as i64,
            tag: if i % 2 == 0 { Some(format!("tag_{}", i % 20)) } else { None },
            tags: (0..((i % 6) + 1)).map(|j| (i * 7 + j * 3) as i32).collect(),
            scores: vec![(i as f64 / 10.0), ((i + 1) as f64 / 10.0)],
            metadata: if i % 5 != 0 { Some(vec![0xAB, 0xCD, (i % 256) as u8]) } else { None },
            color: (i % 16) as u8,
            priority: ((i % 10) + 1) as i16,
        }
    }
}
impl FromIndex for Large15 {
    fn from_index(i: usize) -> Self {
        Self {
            id: i as i64,
            name: format!("record_{}_{}", i % 200, i % 5),
            score: (i as f64) / 50.0 + ((i % 11) as f64) / 100.0,
            weight: ((i * 7) % 1000) as f32 / 100.0,
            active: i % 7 != 0,
            created_at: 1_700_000_000 + (i as i64 * 60),
            updated_at: if i % 4 != 0 { Some(1_700_000_000 + (i as i64 * 60) + 300) } else { None },
            category_id: (i / 50) as i64,
            tag: if i % 2 == 0 { Some(format!("tag_{}", i % 20)) } else { None },
            tags: (0..((i % 6) + 1)).map(|j| (i * 7 + j * 3) as i32).collect(),
            scores: vec![(i as f64 / 10.0), ((i + 1) as f64 / 10.0)],
            metadata: if i % 5 != 0 { Some(vec![0xAB, 0xCD, (i % 256) as u8]) } else { None },
            color: (i % 16) as u8,
            priority: ((i % 10) + 1) as i16,
        }
    }
}
impl FromIndex for Large16 {
    fn from_index(i: usize) -> Self {
        Self {
            id: i as i64,
            name: format!("record_{}_{}", i % 200, i % 5),
            score: (i as f64) / 50.0 + ((i % 11) as f64) / 100.0,
            weight: ((i * 7) % 1000) as f32 / 100.0,
            active: i % 7 != 0,
            created_at: 1_700_000_000 + (i as i64 * 60),
            updated_at: if i % 4 != 0 { Some(1_700_000_000 + (i as i64 * 60) + 300) } else { None },
            category_id: (i / 50) as i64,
            tag: if i % 2 == 0 { Some(format!("tag_{}", i % 20)) } else { None },
            tags: (0..((i % 6) + 1)).map(|j| (i * 7 + j * 3) as i32).collect(),
            scores: vec![(i as f64 / 10.0), ((i + 1) as f64 / 10.0)],
            metadata: if i % 5 != 0 { Some(vec![0xAB, 0xCD, (i % 256) as u8]) } else { None },
            color: (i % 16) as u8,
            priority: ((i % 10) + 1) as i16,
        }
    }
}
impl FromIndex for Large17 {
    fn from_index(i: usize) -> Self {
        Self {
            id: i as i64,
            name: format!("record_{}_{}", i % 200, i % 5),
            score: (i as f64) / 50.0 + ((i % 11) as f64) / 100.0,
            weight: ((i * 7) % 1000) as f32 / 100.0,
            active: i % 7 != 0,
            created_at: 1_700_000_000 + (i as i64 * 60),
            updated_at: if i % 4 != 0 { Some(1_700_000_000 + (i as i64 * 60) + 300) } else { None },
            category_id: (i / 50) as i64,
            tag: if i % 2 == 0 { Some(format!("tag_{}", i % 20)) } else { None },
            tags: (0..((i % 6) + 1)).map(|j| (i * 7 + j * 3) as i32).collect(),
            scores: vec![(i as f64 / 10.0), ((i + 1) as f64 / 10.0)],
            metadata: if i % 5 != 0 { Some(vec![0xAB, 0xCD, (i % 256) as u8]) } else { None },
            color: (i % 16) as u8,
            priority: ((i % 10) + 1) as i16,
        }
    }
}

// Generic generator using FromIndex trait
fn generate<T: FromIndex>() -> Vec<T> {
    (0..N_ROWS).map(T::from_index).collect()
}

struct ChunkResult {
    power: u32,
    chunk_size: usize,
    serial_ms: f64,
    deserial_ms: f64,
    size_bytes: usize,
    num_chunks: usize,
    /// Estimated total in-memory footprint of one chunk's worth of rows (stack + heap)
    memory_footprint: usize,
}

struct ResultRow<'a> {
    r: &'a ChunkResult,
}

impl MarkdownTableRow for ResultRow<'_> {
    fn column_names() -> Vec<&'static str> {
        vec!["Chunk size", "Serialize", "Deserialize", "Size", "Chunks", "Memory per chunk"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![
            format!("2^{} = {}", self.r.power, self.r.chunk_size.to_string()),
            format_ms(self.r.serial_ms),
            format_ms(self.r.deserial_ms),
            format_bytes(self.r.size_bytes),
            self.r.num_chunks.to_string(),
            format_bytes(self.r.memory_footprint),
        ]
    }
}

fn print_group_results<T>(label: &str, results: &[ChunkResult]) {
    let row_size = std::mem::size_of::<T>();
    println!("### {} ({} bytes/row)\n", label, row_size);

    if results.is_empty() {
        return;
    }

    let rows: Vec<_> = results.iter().map(|r| ResultRow { r }).collect();
    println!("{}", as_table(&rows));
}

fn main() {
    println!("## chunk_size\n");

    // ---- Small structs ----
    let mut small_results = Vec::new();

    {
        let data = generate::<Small13>();
        let row_memory_s13 = avg_total_size(&data);
        let chunks_c = Small13::write(data.clone()).unwrap();
        let buf = Small13::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Small13::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Small13::from_bytes(&buf).unwrap();
            let r = Small13::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        small_results.push(ChunkResult {
            power: 13,
            chunk_size: Small13::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_s13 * Small13::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Small14>();
        let row_memory_s14 = avg_total_size(&data);
        let chunks_c = Small14::write(data.clone()).unwrap();
        let buf = Small14::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Small14::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Small14::from_bytes(&buf).unwrap();
            let r = Small14::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        small_results.push(ChunkResult {
            power: 14,
            chunk_size: Small14::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_s14 * Small14::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Small15>();
        let row_memory_s15 = avg_total_size(&data);
        let chunks_c = Small15::write(data.clone()).unwrap();
        let buf = Small15::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Small15::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Small15::from_bytes(&buf).unwrap();
            let r = Small15::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        small_results.push(ChunkResult {
            power: 15,
            chunk_size: Small15::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_s15 * Small15::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Small16>();
        let row_memory_s16 = avg_total_size(&data);
        let chunks_c = Small16::write(data.clone()).unwrap();
        let buf = Small16::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Small16::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Small16::from_bytes(&buf).unwrap();
            let r = Small16::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        small_results.push(ChunkResult {
            power: 16,
            chunk_size: Small16::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_s16 * Small16::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Small17>();
        let row_memory_s17 = avg_total_size(&data);
        let chunks_c = Small17::write(data.clone()).unwrap();
        let buf = Small17::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Small17::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Small17::from_bytes(&buf).unwrap();
            let r = Small17::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        small_results.push(ChunkResult {
            power: 17,
            chunk_size: Small17::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_s17 * Small17::CHUNK_SIZE,
        });
    }

    print_group_results::<Small15>("SmallStruct", &small_results);

    // ---- Medium structs ----
    let mut medium_results = Vec::new();

    {
        let data = generate::<Medium13>();
        let row_memory_m13 = avg_total_size(&data);
        let chunks_c = Medium13::write(data.clone()).unwrap();
        let buf = Medium13::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Medium13::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Medium13::from_bytes(&buf).unwrap();
            let r = Medium13::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        medium_results.push(ChunkResult {
            power: 13,
            chunk_size: Medium13::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_m13 * Medium13::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Medium14>();
        let row_memory_m14 = avg_total_size(&data);
        let chunks_c = Medium14::write(data.clone()).unwrap();
        let buf = Medium14::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Medium14::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Medium14::from_bytes(&buf).unwrap();
            let r = Medium14::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        medium_results.push(ChunkResult {
            power: 14,
            chunk_size: Medium14::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_m14 * Medium14::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Medium15>();
        let row_memory_m15 = avg_total_size(&data);
        let chunks_c = Medium15::write(data.clone()).unwrap();
        let buf = Medium15::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Medium15::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Medium15::from_bytes(&buf).unwrap();
            let r = Medium15::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        medium_results.push(ChunkResult {
            power: 15,
            chunk_size: Medium15::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_m15 * Medium15::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Medium16>();
        let row_memory_m16 = avg_total_size(&data);
        let chunks_c = Medium16::write(data.clone()).unwrap();
        let buf = Medium16::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Medium16::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Medium16::from_bytes(&buf).unwrap();
            let r = Medium16::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        medium_results.push(ChunkResult {
            power: 16,
            chunk_size: Medium16::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_m16 * Medium16::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Medium17>();
        let row_memory_m17 = avg_total_size(&data);
        let chunks_c = Medium17::write(data.clone()).unwrap();
        let buf = Medium17::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Medium17::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Medium17::from_bytes(&buf).unwrap();
            let r = Medium17::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        medium_results.push(ChunkResult {
            power: 17,
            chunk_size: Medium17::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_m17 * Medium17::CHUNK_SIZE,
        });
    }

    print_group_results::<Medium15>("MediumStruct", &medium_results);

    // ---- Large structs ----
    let mut large_results = Vec::new();

    {
        let data = generate::<Large13>();
        let row_memory_l13 = avg_total_size(&data);
        let chunks_c = Large13::write(data.clone()).unwrap();
        let buf = Large13::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Large13::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Large13::from_bytes(&buf).unwrap();
            let r = Large13::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        large_results.push(ChunkResult {
            power: 13,
            chunk_size: Large13::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_l13 * Large13::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Large14>();
        let row_memory_l14 = avg_total_size(&data);
        let chunks_c = Large14::write(data.clone()).unwrap();
        let buf = Large14::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Large14::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Large14::from_bytes(&buf).unwrap();
            let r = Large14::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        large_results.push(ChunkResult {
            power: 14,
            chunk_size: Large14::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_l14 * Large14::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Large15>();
        let row_memory_l15 = avg_total_size(&data);
        let chunks_c = Large15::write(data.clone()).unwrap();
        let buf = Large15::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Large15::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Large15::from_bytes(&buf).unwrap();
            let r = Large15::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        large_results.push(ChunkResult {
            power: 15,
            chunk_size: Large15::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_l15 * Large15::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Large16>();
        let row_memory_l16 = avg_total_size(&data);
        let chunks_c = Large16::write(data.clone()).unwrap();
        let buf = Large16::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Large16::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Large16::from_bytes(&buf).unwrap();
            let r = Large16::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        large_results.push(ChunkResult {
            power: 16,
            chunk_size: Large16::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_l16 * Large16::CHUNK_SIZE,
        });
    }

    {
        let data = generate::<Large17>();
        let row_memory_l17 = avg_total_size(&data);
        let chunks_c = Large17::write(data.clone()).unwrap();
        let buf = Large17::to_bytes(&chunks_c).unwrap();
        let serial_ms = avg_ms(|| Large17::write(data.clone()));
        let deserial_ms = avg_ms(|| {
            let c = Large17::from_bytes(&buf).unwrap();
            let r = Large17::filter(&c, json!({}), &vec![] as &[&str]).unwrap();
            assert_eq!(r.len(), data.len());
        });
        large_results.push(ChunkResult {
            power: 17,
            chunk_size: Large17::CHUNK_SIZE,
            serial_ms,
            deserial_ms,
            size_bytes: buf.len(),
            num_chunks: chunks_c.len(),
            memory_footprint: row_memory_l17 * Large17::CHUNK_SIZE,
        });
    }

    print_group_results::<Large15>("LargeStruct", &large_results);
}
