//! Shape profile of the query_explains capture, to freeze the arena schema
//! (tmp/explain_struct_of_vecs_plan.md) against the full 237,651 rows.
//!
//! Reads the same blob the postgres bench reads (zstd msgpack array of rows,
//! field order mirrors `benches/postgres/query_explains.rs::QueryExplain`):
//!
//!   cargo run --release --example profile_explains > tmp/arena_probe.txt
//!
//! Reports: envelope key vocabulary, the full per-node key vocabulary with
//! value-type histograms, numeric ranges, string-key cardinality (and the
//! complete dictionary for low-cardinality keys), array length stats, object
//! subkey vocabularies, and plan tree depth/node-count distributions.

use pco_pack::chrono::{DateTime, Utc};
use pco_pack::serde::Deserialize;
use pco_pack::serde_json::Value;
use pco_pack::{rmp_serde, zstd};
use std::collections::{HashMap, HashSet};
use std::io::BufReader;

/// Exact field order of the captured msgpack rows.
#[derive(Debug, Deserialize)]
struct ExplainRow {
    #[allow(dead_code)]
    server_id: i64,
    #[allow(dead_code)]
    database_id: i64,
    #[allow(dead_code)]
    postgres_role_id: i64,
    #[allow(dead_code)]
    query_fingerprint: i64,
    #[allow(dead_code)]
    format: i32,
    #[allow(dead_code)]
    source: i32,
    #[allow(dead_code)]
    error: String,
    #[allow(dead_code)]
    seen_at: DateTime<Utc>,
    #[allow(dead_code)]
    runtime_ms: f32,
    #[allow(dead_code)]
    total_cost: Option<f32>,
    #[allow(dead_code)]
    total_shared_blks_hit: Option<i64>,
    #[allow(dead_code)]
    total_shared_blks_read: Option<i64>,
    #[allow(dead_code)]
    total_shared_blks_written: Option<i64>,
    #[allow(dead_code)]
    total_shared_blks_dirtied: Option<i64>,
    #[allow(dead_code)]
    total_blk_read_time: Option<f32>,
    #[allow(dead_code)]
    total_blk_write_time: Option<f32>,
    #[allow(dead_code)]
    plan_node_types: Vec<smol_str::SmolStr>,
    #[allow(dead_code)]
    plan_fingerprint: Option<i64>,
    #[allow(dead_code)]
    output_text: Option<String>,
    output_json: Option<Value>,
    node_insights: Option<Value>,
}

const DICT_CAP: usize = 4096;

#[derive(Default)]
struct KeyStat {
    n: u64,
    // Value-type histograms keyed by serde_json type names.
    ty: HashMap<&'static str, u64>,
    bool_true: u64,
    i_min: Option<i64>,
    i_max: Option<i64>,
    f_min: Option<f64>,
    f_max: Option<f64>,
    frac: u64, // float values with a nonzero fractional part
    // Strings: cardinality set (dropped once capped), length stats.
    dict: HashSet<String>,
    dict_capped: bool,
    str_len_sum: u64,
    str_len_max: u64,
    // Arrays: length stats, element-type histogram, string element bytes.
    arr_len_sum: u64,
    arr_len_max: u64,
    arr_elem: HashMap<&'static str, u64>,
    arr_str_len_sum: u64,
    // Subkeys for object-valued keys and arrays of objects (e.g. Workers).
    children: HashMap<String, KeyStat>,
}

impl KeyStat {
    fn bump(&mut self, ty: &'static str) -> &mut u64 {
        self.n += 1;
        self.ty.entry(ty).or_insert(0)
    }

    fn record(&mut self, v: &Value) {
        match v {
            Value::Null => {
                *self.bump("null") += 1;
            }
            Value::Bool(b) => {
                *self.bump("bool") += 1;
                self.bool_true += *b as u64;
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    *self.bump("i64") += 1;
                    self.i_min = Some(self.i_min.map_or(i, |m| m.min(i)));
                    self.i_max = Some(self.i_max.map_or(i, |m| m.max(i)));
                } else if n.is_u64() {
                    *self.bump("u64") += 1;
                } else if let Some(f) = n.as_f64() {
                    *self.bump("f64") += 1;
                    self.frac += (f.fract() != 0.0) as u64;
                    self.f_min = Some(self.f_min.map_or(f, |m| m.min(f)));
                    self.f_max = Some(self.f_max.map_or(f, |m| m.max(f)));
                }
            }
            Value::String(s) => {
                *self.bump("string") += 1;
                self.str_len_sum += s.len() as u64;
                self.str_len_max = self.str_len_max.max(s.len() as u64);
                if self.dict_capped {
                    return;
                }
                self.dict.insert(s.clone());
                if self.dict.len() > DICT_CAP {
                    self.dict_capped = true;
                    self.dict.clear();
                }
            }
            Value::Array(a) => {
                *self.bump("array") += 1;
                self.arr_len_sum += a.len() as u64;
                self.arr_len_max = self.arr_len_max.max(a.len() as u64);
                for e in a {
                    let et = match e {
                        Value::Null => "null",
                        Value::Bool(_) => "bool",
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::Array(_) => "array",
                        Value::Object(_) => "object",
                    };
                    *self.arr_elem.entry(et).or_insert(0) += 1;
                    if let Some(s) = e.as_str() {
                        self.arr_str_len_sum += s.len() as u64;
                    }
                    if let Some(o) = e.as_object() {
                        for (k, cv) in o {
                            if k == "Plans" {
                                // Tree links are handled by arena parent_idx,
                                // not by recursion here.
                                *self.children.entry("Plans".into()).or_default().bump("array") += 1;
                                continue;
                            }
                            self.children.entry(k.clone()).or_default().record(cv);
                        }
                    }
                }
            }
            Value::Object(o) => {
                *self.bump("object") += 1;
                for (k, cv) in o {
                    if k == "Plans" {
                        *self.children.entry("Plans".into()).or_default().bump("array") += 1;
                        continue;
                    }
                    self.children.entry(k.clone()).or_default().record(cv);
                }
            }
        }
    }
}

#[derive(Default)]
struct Stats {
    rows_total: u64,
    rows_json_null: u64,
    // Envelope (top-level [ {Plan, Triggers, Query Identifier} ]).
    envelope_len: HashMap<u64, u64>,
    envelope: HashMap<String, KeyStat>,
    triggers_nonempty: u64,
    // Plan trees.
    nodes_total: u64,
    depth_hist: HashMap<u64, u64>,
    nodes_count_hist: HashMap<u64, u64>,
    keys: HashMap<String, KeyStat>,
    // node_insights payloads.
    insights_rows: u64,
    insights_elems: HashMap<String, KeyStat>,
}

fn walk(
    node: &Value, depth: u64, nodes_total: &mut u64, depth_hist: &mut HashMap<u64, u64>,
    keys: &mut HashMap<String, KeyStat>,
) {
    *nodes_total += 1;
    *depth_hist.entry(depth).or_insert(0) += 1;
    for (k, v) in node.as_object().into_iter().flatten() {
        if k == "Plans" {
            let st = keys.entry(k.clone()).or_default();
            st.n += 1;
            *st.ty.entry("array").or_insert(0) += 1;
            if let Some(a) = v.as_array() {
                st.arr_len_sum += a.len() as u64;
                st.arr_len_max = st.arr_len_max.max(a.len() as u64);
            }
            continue;
        }
        keys.entry(k.clone()).or_default().record(v);
    }
    if let Some(children) = node.get("Plans").and_then(Value::as_array) {
        for c in children {
            walk(c, depth + 1, nodes_total, depth_hist, keys);
        }
    }
}

fn bucket_nodes(n: u64) -> &'static str {
    match n {
        0 => "0",
        1 => "1",
        2..=4 => "2-4",
        5..=9 => "5-9",
        10..=19 => "10-19",
        20..=49 => "20-49",
        50..=99 => "50-99",
        100..=249 => "100-249",
        250..=499 => "250-499",
        _ => "500+",
    }
}

fn pct(n: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { (n as f64 * 1000.0 / total as f64).round() / 10.0 }
}

fn dump(stats: &HashMap<String, KeyStat>, total: u64) {
    let mut names: Vec<&String> = stats.keys().collect();
    names.sort();
    for name in names {
        let s = &stats[name];
        let mut types: Vec<(&str, u64)> = s.ty.iter().map(|(k, v)| (*k, *v)).collect();
        types.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
        let tys: Vec<String> = types.iter().map(|(t, v)| format!("{t} {}", pct(*v, s.n))).collect();
        print!("{name}\tn={}\t{}\t", s.n, tys.join(" "));
        print!("\t{}", pct(s.n, total));
        if s.i_min.is_some() || s.f_min.is_some() {
            match (s.i_min, s.i_max) {
                (Some(a), Some(b)) => print!("\tint=[{a},{b}]"),
                _ => {}
            }
            match (s.f_min, s.f_max) {
                (Some(a), Some(b)) => print!("\tfloat=[{a},{b}] frac={}", pct(s.frac, s.n)),
                _ => {}
            }
        }
        if s.n - s.bool_true > 0 || s.bool_true > 0 {
            if let Some(&nbool) = s.ty.get("bool") {
                print!("\tbool_true={}", pct(s.bool_true, nbool));
            }
        }
        if s.dict_capped {
            print!(
                "\tdistinct>{DICT_CAP} strlen_avg={:.1} strlen_max={}",
                s.str_len_sum as f64 / s.n.max(1) as f64,
                s.str_len_max
            );
        } else if !s.dict.is_empty() {
            print!("\tdistinct={} [", s.dict.len());
            let mut vals: Vec<&String> = s.dict.iter().collect();
            vals.sort();
            print!("{}", vals.iter().map(|v| v.as_str()).collect::<Vec<_>>().join("|"));
            print!("]");
        }
        if s.arr_len_sum > 0 {
            print!(
                "\tarr_len_avg={:.1} arr_len_max={} bytes={}",
                s.arr_len_sum as f64 / s.n.max(1) as f64,
                s.arr_len_max,
                s.arr_str_len_sum
            );
            if !s.arr_elem.is_empty() {
                let mut et: Vec<(&str, u64)> = s.arr_elem.iter().map(|(k, v)| (*k, *v)).collect();
                et.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
                print!("\telem={}", et.iter().map(|(t, v)| format!("{t}:{v}")).collect::<Vec<_>>().join(","));
            }
        }
        if !s.children.is_empty() {
            println!();
            dump(&s.children, total);
        } else {
            println!();
        }
    }
}

fn main() -> anyhow::Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data/query_explains.msgpack.zst");
    let file = std::fs::File::open(&path)?;
    let decoder = zstd::stream::read::Decoder::new(BufReader::new(file))?;
    let rows: Vec<ExplainRow> = rmp_serde::decode::from_read(decoder)?;
    println!("# profile of {} rows", rows.len());

    let mut st = Stats::default();
    st.rows_total = rows.len() as u64;
    let mut row_counts: Vec<u64> = Vec::with_capacity(rows.len());
    for r in &rows {
        let Some(doc0) = r.output_json.as_ref().and_then(|j| j.get(0)) else {
            st.rows_json_null += 1;
            row_counts.push(0);
            continue;
        };
        *st.envelope_len
            .entry(r.output_json.as_ref().map(Value::as_array).flatten().map_or(0, |a| a.len() as u64))
            .or_insert(0) += 1;
        for (k, v) in doc0.as_object().into_iter().flatten() {
            st.envelope.entry(k.clone()).or_default().record(v);
        }
        if doc0.get("Triggers").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
            st.triggers_nonempty += 1;
        }
        let mut n_before = st.nodes_total;
        if let Some(plan) = doc0.get("Plan") {
            walk(plan, 0, &mut st.nodes_total, &mut st.depth_hist, &mut st.keys);
        }
        row_counts.push(st.nodes_total - n_before);
        if let Some(ins) = &r.node_insights {
            st.insights_rows += 1;
            if let Value::Array(a) = ins {
                for e in a {
                    for (k, cv) in e.as_object().into_iter().flatten() {
                        st.insights_elems.entry(k.clone()).or_default().record(cv);
                    }
                }
            }
        }
    }

    let docs = st.rows_total - st.rows_json_null;
    println!("\n== envelope ==");
    println!("rows_total={}\tdocs={}\tjson_null={}", st.rows_total, docs, st.rows_json_null);
    let mut el: Vec<(u64, u64)> = st.envelope_len.iter().map(|(k, v)| (*k, *v)).collect();
    el.sort();
    println!("toplevel_len={:?}", el);
    println!("triggers_nonempty={}", st.triggers_nonempty);
    println!("-- envelope keys --");
    dump(&st.envelope, docs);

    println!("\n== plan trees ==");
    let mut dh: Vec<(u64, u64)> = st.depth_hist.iter().map(|(k, v)| (*k, *v)).collect();
    dh.sort();
    println!("nodes_total={}\tnodes_per_doc_avg={:.2}", st.nodes_total, st.nodes_total as f64 / docs.max(1) as f64);
    println!("depth_hist={dh:?}");
    let mut nc: HashMap<&str, u64> = HashMap::new();
    for c in &row_counts {
        *nc.entry(bucket_nodes(*c)).or_insert(0) += 1;
    }
    let mut ncb: Vec<(&str, u64)> = nc.iter().map(|(k, v)| (*k, *v)).collect();
    ncb.sort_by_key(|(k, _)| k.parse::<u64>().unwrap_or(u64::MAX));
    println!("nodes_per_doc_buckets={ncb:?}");

    println!("\n== node keys ==");
    dump(&st.keys, st.nodes_total);

    println!("\n== node_insights ==");
    println!("rows={}", st.insights_rows);
    dump(&st.insights_elems, st.insights_rows.max(1));
    Ok(())
}
