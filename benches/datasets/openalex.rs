//! openalex: OpenAlex `works` snapshot (CC0) via the Mearman/OpenAlex
//! HuggingFace mirror, revision-pinned at `updated_date=2026-03-30`,
//! two JSONL parts (~1.5 GB gzipped). OpenAlex rewrites its live S3
//! partitions in place during maintenance, so the mirror's frozen git
//! revision is the durable byte source. Row counts are fixed by the pin.
//! Nested-structure showcase: 3-level nesting (`Work` -> `Authorship` -> `Institution`),
//! `Vec<struct>` fields, `Option` on most fields, an inverted-abstract map,
//! 5 unit enums, f32 scores rounded to 4 decimals in the lossy mirror
//! (deliberate fidelity cut).

use chrono::{DateTime, Utc};
use pco_pack::PcoPack;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::io::BufRead;

use super::common::{self, CodecResults, Ds};

// OpenAlex's live S3 tree rewrites partition bytes in place (2026-10-14
// swept the whole updated_date tree), so its URLs are durable but not
// immutable. This HuggingFace mirror froze the tree and the resolve URL is
// pinned to a git revision, which is immutable; the mirror's sync ends at
// 2026-03-30. HF ETags are xet hashes (not md5), so `size` pins them.
pub const DS_0: Ds = Ds {
    name: "openalex_part_0000.gz",
    url: "https://huggingface.co/datasets/Mearman/OpenAlex/resolve/ef02effac13bfbd0991612f444cebcef8a882453/data/works/updated_date=2026-03-30/part_0000.jsonl.gz",
    size: 845_002_113,
    md5: None,
};
pub const DS_1: Ds = Ds {
    name: "openalex_part_0001.gz",
    url: "https://huggingface.co/datasets/Mearman/OpenAlex/resolve/ef02effac13bfbd0991612f444cebcef8a882453/data/works/updated_date=2026-03-30/part_0001.jsonl.gz",
    size: 713_214_350,
    md5: None,
};

// ---- data model: PcoPack rows, the bench's own serde wire ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkType {
    #[default]
    Article,
    Preprint,
    #[serde(rename = "conference-paper")]
    ConferencePaper,
    #[serde(rename = "conference-abstract")]
    ConferenceAbstract,
    Other,
    Review,
    #[serde(rename = "book-chapter")]
    BookChapter,
    Erratum,
    Paratext,
    Report,
    #[serde(rename = "data-paper")]
    DataPaper,
    Book,
    Dataset,
    #[serde(rename = "peer-review")]
    PeerReview,
    #[serde(rename = "book-review")]
    BookReview,
    Dissertation,
    Editorial,
    Software,
    Letter,
    #[serde(rename = "reference-entry")]
    ReferenceEntry,
    #[serde(rename = "supplementary-materials")]
    SupplementaryMaterials,
    Retraction,
    Standard,
    #[serde(rename = "software-paper")]
    SoftwarePaper,
    Libguides,
    Database,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OaStatus {
    #[default]
    Gold,
    Green,
    Hybrid,
    Bronze,
    Diamond,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    #[default]
    Journal,
    Repository,
    Conference,
    #[serde(rename = "book series")]
    BookSeries,
    #[serde(rename = "ebook platform")]
    EbookPlatform,
    Metadata,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    #[default]
    First,
    Last,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstType {
    #[default]
    Education,
    Company,
    Government,
    Nonprofit,
    Healthcare,
    Facility,
    Funder,
    Archive,
    Other,
}

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Work {
    pub id: i64,             // digit tail after the last `/`, e.g. W2741809807
    pub doi: Option<String>, // payload after `https://doi.org/`
    pub title: Option<String>,
    pub publication_year: Option<i32>,
    pub publication_date: Option<DateTime<Utc>>, // "2018-02-13" -> midnight
    pub language: Option<SmolStr>,               // ISO-639-3, ~100 values
    pub work_type: WorkType,                     // 26 values; the api always sends it
    pub cited_by_count: i32,                     // zero-spike + right skew
    pub fwci: Option<f32>,
    pub is_retracted: Option<bool>, // null on 209/558,403 records
    pub oa_status: OaStatus,        // 6 values incl. `closed`; the api always sends it
    pub is_oa: bool,
    pub oa_url: Option<String>,
    pub source: Option<String>, // primary_location.source.display_name
    pub source_type: Option<SourceType>,
    pub authorships: Vec<Authorship>,                                // up to 5,000
    pub topics: Vec<Topic>,                                          // <= 3
    pub keywords: Vec<Kw>,                                           // <= 31
    pub abstract_inverted_index: Option<HashMap<SmolStr, Vec<i32>>>, // ~75% present
    pub updated_date: DateTime<Utc>,
    pub created_date: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Authorship {
    pub author_id: Option<i64>, // null on ~12% of authorships (unresolved authors)
    pub author_name: String,
    pub position: Position,
    pub institutions: Vec<Institution>,
    pub countries: Vec<SmolStr>, // ISO-2
    pub is_corresponding: bool,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Institution {
    pub id: Option<i64>,          // null on unresolved institutions
    pub name: Option<String>,     // display_name; null on unresolved institutions
    pub ror: Option<SmolStr>,     // payload after `https://ror.org/` (variable length)
    pub country: Option<SmolStr>, // ISO-2
    pub inst_type: Option<InstType>,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Topic {
    pub id: i64,
    pub name: String,
    pub score: f32, // 0..1
    pub subfield_id: Option<i64>,
    pub field_id: Option<i64>,
    pub domain_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Kw {
    pub id: SmolStr, // the keyword slug (not numeric)
    pub name: String,
    pub score: f32, // 0..1, up to 7 fractional digits in the source
}

// ---- bench entrypoint ----

pub fn bench() {
    let data = load();
    let n = data.len();

    // `work_type` equality filters: selective (`Retraction` = 21), moderate
    // (`Preprint` = 1); PcoPack filters by discriminant.
    let picks: [(WorkType, &str); 2] = [(WorkType::Retraction, "retraction"), (WorkType::Preprint, "preprint")];
    let keys: Vec<(u8, &str)> = picks.iter().map(|(t, name)| (work_type_u8(t), *name)).collect();
    let exps: Vec<usize> = picks.iter().map(|(t, _)| data.iter().filter(|w| w.work_type == *t).count()).collect();
    let labels: Vec<String> = exps.iter().map(|&e| common::filter_label(e, n)).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    println!("### openalex ({n} works)\n");

    let queries: Vec<serde_json::Value> =
        keys.iter().map(|(disc, _)| serde_json::json!({ "work_type": disc })).collect();
    let pco_filters: Vec<(&serde_json::Value, usize)> = queries.iter().zip(exps.iter()).map(|(q, e)| (q, *e)).collect();

    let pco = common::bench_pco_pack(data.to_vec(), &pco_filters, |back| verify(&data, back));

    let dataf: Vec<WorkF> = data.iter().map(WorkF::from).collect();
    let pcof = common::bench_pco_pack(dataf, &pco_filters, |back| verify_lossy(&data, back));

    let key_filters: Vec<((u8, &str), usize)> = keys.iter().zip(exps.iter()).map(|(k, e)| (*k, *e)).collect();

    let col = common::bench_columnar(
        n,
        || data.iter().map(WorkCol::from).collect(),
        &key_filters,
        |decoded: &columnar::BorrowedOf<WorkCol>, k: &(u8, &str)| {
            let count = columnar::Len::len(decoded);
            (0..count)
                .filter(|&i| {
                    let r = columnar::Index::get(decoded, i);
                    *r.work_type == k.0
                })
                .count()
        },
    );

    let sc = common::bench_serde_columnar(
        n,
        || openalex_sc::WorkScStore { data: data.iter().map(openalex_sc::WorkScRow::from).collect() },
        &key_filters,
        |s: &openalex_sc::WorkScStore, k: &(u8, &str)| s.data.iter().filter(|r| r.work_type == k.0).count(),
    );

    let mp = common::bench_msgpack(&data, &key_filters, |w: &Work, k: &(u8, &str)| work_type_u8(&w.work_type) == k.0);

    let data_vx: Vec<WorkVx> = data.iter().map(WorkVx::from).collect();
    let vx = common::bench_vortex_ds(&data_vx, &key_filters, |k: &(u8, &str)| {
        vortex::array::expr::eq(vortex::array::expr::col("work_type"), vortex::array::expr::lit(k.1))
    });

    CodecResults {
        pco_pack: pco,
        pco_pack_lossy: Some(pcof),
        columnar: col,
        serde_columnar: sc,
        msgpack: mp,
        vortex: vx,
    }
    .print(&label_refs);
}

// ---- lossy mirror: `float_round = 4` on structs owning floats;
// Authorship/Institution own none, reused as-is ----

#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(float_round = 4)]
pub struct WorkF {
    pub id: i64,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub publication_year: Option<i32>,
    pub publication_date: Option<DateTime<Utc>>,
    pub language: Option<SmolStr>,
    pub work_type: WorkType,
    pub cited_by_count: i32,
    pub fwci: Option<f32>,
    pub is_retracted: Option<bool>,
    pub oa_status: OaStatus,
    pub is_oa: bool,
    pub oa_url: Option<String>,
    pub source: Option<String>,
    pub source_type: Option<SourceType>,
    pub authorships: Vec<Authorship>,
    pub topics: Vec<TopicF>,
    pub keywords: Vec<KwF>,
    pub abstract_inverted_index: Option<HashMap<SmolStr, Vec<i32>>>,
    pub updated_date: DateTime<Utc>,
    pub created_date: DateTime<Utc>,
}

impl From<&Work> for WorkF {
    fn from(r: &Work) -> Self {
        WorkF {
            id: r.id,
            doi: r.doi.clone(),
            title: r.title.clone(),
            publication_year: r.publication_year,
            publication_date: r.publication_date.clone(),
            language: r.language.clone(),
            work_type: r.work_type,
            cited_by_count: r.cited_by_count,
            fwci: r.fwci,
            is_retracted: r.is_retracted,
            oa_status: r.oa_status,
            is_oa: r.is_oa,
            oa_url: r.oa_url.clone(),
            source: r.source.clone(),
            source_type: r.source_type,
            authorships: r.authorships.clone(),
            topics: r.topics.iter().map(TopicF::from).collect(),
            keywords: r.keywords.iter().map(KwF::from).collect(),
            abstract_inverted_index: r.abstract_inverted_index.clone(),
            updated_date: r.updated_date.clone(),
            created_date: r.created_date.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(float_round = 4)]
pub struct TopicF {
    pub id: i64,
    pub name: String,
    pub score: f32,
    pub subfield_id: Option<i64>,
    pub field_id: Option<i64>,
    pub domain_id: Option<i64>,
}

impl From<&Topic> for TopicF {
    fn from(r: &Topic) -> Self {
        TopicF {
            id: r.id,
            name: r.name.clone(),
            score: r.score,
            subfield_id: r.subfield_id,
            field_id: r.field_id,
            domain_id: r.domain_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(float_round = 4)]
pub struct KwF {
    pub id: SmolStr,
    pub name: String,
    pub score: f32,
}

impl From<&Kw> for KwF {
    fn from(r: &Kw) -> Self {
        KwF { id: r.id.clone(), name: r.name.clone(), score: r.score }
    }
}

// ---- columnar mirror: DateTime -> i64 us, enums -> u8 discriminants,
// SmolStr -> String, map -> Vec<(K, V)> (no chrono/enum/map impl) ----

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct WorkCol {
    pub id: i64,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub publication_year: Option<i32>,
    pub publication_date: Option<i64>,
    pub language: Option<String>,
    pub work_type: u8,
    pub cited_by_count: i32,
    pub fwci: Option<f32>,
    pub is_retracted: Option<bool>,
    pub oa_status: u8,
    pub is_oa: bool,
    pub oa_url: Option<String>,
    pub source: Option<String>,
    pub source_type: Option<u8>,
    pub authorships: Vec<AuthorshipCol>,
    pub topics: Vec<TopicCol>,
    pub keywords: Vec<KwCol>,
    pub abstract_inverted_index: Option<Vec<(String, Vec<i32>)>>,
    pub updated_date: i64,
    pub created_date: i64,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct AuthorshipCol {
    pub author_id: Option<i64>,
    pub author_name: String,
    pub position: u8,
    pub institutions: Vec<InstitutionCol>,
    pub countries: Vec<String>,
    pub is_corresponding: bool,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct InstitutionCol {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub ror: Option<String>,
    pub country: Option<String>,
    pub inst_type: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct TopicCol {
    pub id: i64,
    pub name: String,
    pub score: f32,
    pub subfield_id: Option<i64>,
    pub field_id: Option<i64>,
    pub domain_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct KwCol {
    pub id: String,
    pub name: String,
    pub score: f32,
}

fn work_type_u8(t: &WorkType) -> u8 {
    match t {
        WorkType::Article => 0,
        WorkType::Preprint => 1,
        WorkType::ConferencePaper => 2,
        WorkType::ConferenceAbstract => 3,
        WorkType::Other => 4,
        WorkType::Review => 5,
        WorkType::BookChapter => 6,
        WorkType::Erratum => 7,
        WorkType::Paratext => 8,
        WorkType::Report => 9,
        WorkType::DataPaper => 10,
        WorkType::Book => 11,
        WorkType::Dataset => 12,
        WorkType::PeerReview => 13,
        WorkType::BookReview => 14,
        WorkType::Dissertation => 15,
        WorkType::Editorial => 16,
        WorkType::Software => 17,
        WorkType::Letter => 18,
        WorkType::ReferenceEntry => 19,
        WorkType::SupplementaryMaterials => 20,
        WorkType::Retraction => 21,
        WorkType::Standard => 22,
        WorkType::SoftwarePaper => 23,
        WorkType::Libguides => 24,
        WorkType::Database => 25,
    }
}

fn oa_status_u8(t: &OaStatus) -> u8 {
    match t {
        OaStatus::Gold => 0,
        OaStatus::Green => 1,
        OaStatus::Hybrid => 2,
        OaStatus::Bronze => 3,
        OaStatus::Diamond => 4,
        OaStatus::Closed => 5,
    }
}

fn source_type_u8(t: &SourceType) -> u8 {
    match t {
        SourceType::Journal => 0,
        SourceType::Repository => 1,
        SourceType::Conference => 2,
        SourceType::BookSeries => 3,
        SourceType::EbookPlatform => 4,
        SourceType::Metadata => 5,
        SourceType::Other => 6,
    }
}

fn position_u8(t: &Position) -> u8 {
    match t {
        Position::First => 0,
        Position::Last => 1,
        Position::Middle => 2,
    }
}

fn inst_type_u8(t: &InstType) -> u8 {
    match t {
        InstType::Education => 0,
        InstType::Company => 1,
        InstType::Government => 2,
        InstType::Nonprofit => 3,
        InstType::Healthcare => 4,
        InstType::Facility => 5,
        InstType::Funder => 6,
        InstType::Archive => 7,
        InstType::Other => 8,
    }
}

fn abstract_kv(m: &Option<HashMap<SmolStr, Vec<i32>>>) -> Option<Vec<(String, Vec<i32>)>> {
    m.as_ref().map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

impl From<&Work> for WorkCol {
    fn from(r: &Work) -> Self {
        WorkCol {
            id: r.id,
            doi: r.doi.clone(),
            title: r.title.clone(),
            publication_year: r.publication_year,
            publication_date: r.publication_date.map(|d| d.timestamp_micros()),
            language: r.language.as_deref().map(str::to_string),
            work_type: work_type_u8(&r.work_type),
            cited_by_count: r.cited_by_count,
            fwci: r.fwci,
            is_retracted: r.is_retracted,
            oa_status: oa_status_u8(&r.oa_status),
            is_oa: r.is_oa,
            oa_url: r.oa_url.clone(),
            source: r.source.clone(),
            source_type: r.source_type.map(|t| source_type_u8(&t)),
            authorships: r.authorships.iter().map(AuthorshipCol::from).collect(),
            topics: r.topics.iter().map(TopicCol::from).collect(),
            keywords: r.keywords.iter().map(KwCol::from).collect(),
            abstract_inverted_index: abstract_kv(&r.abstract_inverted_index),
            updated_date: r.updated_date.timestamp_micros(),
            created_date: r.created_date.timestamp_micros(),
        }
    }
}

impl From<&Authorship> for AuthorshipCol {
    fn from(r: &Authorship) -> Self {
        AuthorshipCol {
            author_id: r.author_id,
            author_name: r.author_name.clone(),
            position: position_u8(&r.position),
            institutions: r.institutions.iter().map(InstitutionCol::from).collect(),
            countries: r.countries.iter().map(|c| c.to_string()).collect(),
            is_corresponding: r.is_corresponding,
        }
    }
}

impl From<&Institution> for InstitutionCol {
    fn from(r: &Institution) -> Self {
        InstitutionCol {
            id: r.id,
            name: r.name.clone(),
            ror: r.ror.as_deref().map(str::to_string),
            country: r.country.as_deref().map(str::to_string),
            inst_type: r.inst_type.map(|t| inst_type_u8(&t)),
        }
    }
}

impl From<&Topic> for TopicCol {
    fn from(r: &Topic) -> Self {
        TopicCol {
            id: r.id,
            name: r.name.clone(),
            score: r.score,
            subfield_id: r.subfield_id,
            field_id: r.field_id,
            domain_id: r.domain_id,
        }
    }
}

impl From<&Kw> for KwCol {
    fn from(r: &Kw) -> Self {
        KwCol { id: r.id.to_string(), name: r.name.clone(), score: r.score }
    }
}

// ---- serde_columnar mirror: ints -> DeltaRle, String/f32 -> Rle,
// bool -> BoolRle, enums -> u8, `class = "vec"` for Vec<struct>;
// Option/map -> per-row blobs ----

mod openalex_sc {
    use super::*;
    use serde_columnar::columnar;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct WorkScRow {
        #[columnar(strategy = "DeltaRle")]
        pub id: i64,
        pub doi: Option<String>,
        pub title: Option<String>,
        pub publication_year: Option<i32>,
        pub publication_date: Option<i64>,
        pub language: Option<String>,
        #[columnar(strategy = "DeltaRle")]
        pub work_type: u8,
        #[columnar(strategy = "DeltaRle")]
        pub cited_by_count: i32,
        pub fwci: Option<f32>,
        pub is_retracted: Option<bool>,
        #[columnar(strategy = "DeltaRle")]
        pub oa_status: u8,
        #[columnar(strategy = "BoolRle")]
        pub is_oa: bool,
        pub oa_url: Option<String>,
        pub source: Option<String>,
        pub source_type: Option<u8>,
        #[columnar(class = "vec")]
        pub authorships: Vec<AuthorshipScRow>,
        #[columnar(class = "vec")]
        pub topics: Vec<TopicScRow>,
        #[columnar(class = "vec")]
        pub keywords: Vec<KwScRow>,
        pub abstract_inverted_index: Option<HashMap<String, Vec<i32>>>,
        #[columnar(strategy = "DeltaRle")]
        pub updated_date: i64,
        #[columnar(strategy = "DeltaRle")]
        pub created_date: i64,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct AuthorshipScRow {
        pub author_id: Option<i64>,
        #[columnar(strategy = "Rle")]
        pub author_name: String,
        #[columnar(strategy = "DeltaRle")]
        pub position: u8,
        #[columnar(class = "vec")]
        pub institutions: Vec<InstitutionScRow>,
        pub countries: Vec<String>,
        #[columnar(strategy = "BoolRle")]
        pub is_corresponding: bool,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct InstitutionScRow {
        pub id: Option<i64>,
        pub name: Option<String>,
        pub ror: Option<String>,
        pub country: Option<String>,
        pub inst_type: Option<u8>,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct TopicScRow {
        #[columnar(strategy = "DeltaRle")]
        pub id: i64,
        #[columnar(strategy = "Rle")]
        pub name: String,
        #[columnar(strategy = "Rle")]
        pub score: f32,
        pub subfield_id: Option<i64>,
        pub field_id: Option<i64>,
        pub domain_id: Option<i64>,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct KwScRow {
        #[columnar(strategy = "Rle")]
        pub id: String,
        #[columnar(strategy = "Rle")]
        pub name: String,
        #[columnar(strategy = "Rle")]
        pub score: f32,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct WorkScStore {
        #[columnar(class = "vec")]
        pub data: Vec<WorkScRow>,
    }

    impl From<&Work> for WorkScRow {
        fn from(r: &Work) -> Self {
            WorkScRow {
                id: r.id,
                doi: r.doi.clone(),
                title: r.title.clone(),
                publication_year: r.publication_year,
                publication_date: r.publication_date.map(|d| d.timestamp_micros()),
                language: r.language.as_deref().map(str::to_string),
                work_type: super::work_type_u8(&r.work_type),
                cited_by_count: r.cited_by_count,
                fwci: r.fwci,
                is_retracted: r.is_retracted,
                oa_status: super::oa_status_u8(&r.oa_status),
                is_oa: r.is_oa,
                oa_url: r.oa_url.clone(),
                source: r.source.clone(),
                source_type: r.source_type.map(|t| super::source_type_u8(&t)),
                authorships: r.authorships.iter().map(AuthorshipScRow::from).collect(),
                topics: r.topics.iter().map(TopicScRow::from).collect(),
                keywords: r.keywords.iter().map(KwScRow::from).collect(),
                abstract_inverted_index: r
                    .abstract_inverted_index
                    .as_ref()
                    .map(|m| m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()),
                updated_date: r.updated_date.timestamp_micros(),
                created_date: r.created_date.timestamp_micros(),
            }
        }
    }

    impl From<&Authorship> for AuthorshipScRow {
        fn from(r: &Authorship) -> Self {
            AuthorshipScRow {
                author_id: r.author_id,
                author_name: r.author_name.clone(),
                position: super::position_u8(&r.position),
                institutions: r.institutions.iter().map(InstitutionScRow::from).collect(),
                countries: r.countries.iter().map(|c| c.to_string()).collect(),
                is_corresponding: r.is_corresponding,
            }
        }
    }

    impl From<&Institution> for InstitutionScRow {
        fn from(r: &Institution) -> Self {
            InstitutionScRow {
                id: r.id,
                name: r.name.clone(),
                ror: r.ror.as_deref().map(str::to_string),
                country: r.country.as_deref().map(str::to_string),
                inst_type: r.inst_type.map(|t| super::inst_type_u8(&t)),
            }
        }
    }

    impl From<&Topic> for TopicScRow {
        fn from(r: &Topic) -> Self {
            TopicScRow {
                id: r.id,
                name: r.name.clone(),
                score: r.score,
                subfield_id: r.subfield_id,
                field_id: r.field_id,
                domain_id: r.domain_id,
            }
        }
    }

    impl From<&Kw> for KwScRow {
        fn from(r: &Kw) -> Self {
            KwScRow { id: r.id.to_string(), name: r.name.clone(), score: r.score }
        }
    }
}

impl common::ScStore for openalex_sc::WorkScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

// ---- vortex mirror: SmolStr -> String, map -> Vec<(K, V)>,
// enums -> utf8 serde names ----
// rare variants miss the 100-row sample: Union, not encodable

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkVx {
    pub id: i64,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub publication_year: Option<i32>,
    pub publication_date: Option<DateTime<Utc>>,
    pub language: Option<String>,
    pub work_type: String,
    pub cited_by_count: i32,
    pub fwci: Option<f32>,
    pub is_retracted: Option<bool>,
    pub oa_status: String,
    pub is_oa: bool,
    pub oa_url: Option<String>,
    pub source: Option<String>,
    pub source_type: Option<String>,
    pub authorships: Vec<AuthorshipVx>,
    pub topics: Vec<TopicVx>,
    pub keywords: Vec<KwVx>,
    pub abstract_inverted_index: Option<Vec<(String, Vec<i32>)>>,
    pub updated_date: DateTime<Utc>,
    pub created_date: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AuthorshipVx {
    pub author_id: Option<i64>,
    pub author_name: String,
    pub position: String,
    pub institutions: Vec<InstitutionVx>,
    pub countries: Vec<String>,
    pub is_corresponding: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InstitutionVx {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub ror: Option<String>,
    pub country: Option<String>,
    pub inst_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TopicVx {
    pub id: i64,
    pub name: String,
    pub score: f32,
    pub subfield_id: Option<i64>,
    pub field_id: Option<i64>,
    pub domain_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KwVx {
    pub id: String,
    pub name: String,
    pub score: f32,
}

fn work_type_str(t: &WorkType) -> &'static str {
    match t {
        WorkType::Article => "article",
        WorkType::Preprint => "preprint",
        WorkType::ConferencePaper => "conference-paper",
        WorkType::ConferenceAbstract => "conference-abstract",
        WorkType::Other => "other",
        WorkType::Review => "review",
        WorkType::BookChapter => "book-chapter",
        WorkType::Erratum => "erratum",
        WorkType::Paratext => "paratext",
        WorkType::Report => "report",
        WorkType::DataPaper => "data-paper",
        WorkType::Book => "book",
        WorkType::Dataset => "dataset",
        WorkType::PeerReview => "peer-review",
        WorkType::BookReview => "book-review",
        WorkType::Dissertation => "dissertation",
        WorkType::Editorial => "editorial",
        WorkType::Software => "software",
        WorkType::Letter => "letter",
        WorkType::ReferenceEntry => "reference-entry",
        WorkType::SupplementaryMaterials => "supplementary-materials",
        WorkType::Retraction => "retraction",
        WorkType::Standard => "standard",
        WorkType::SoftwarePaper => "software-paper",
        WorkType::Libguides => "libguides",
        WorkType::Database => "database",
    }
}

fn oa_status_str(t: &OaStatus) -> &'static str {
    match t {
        OaStatus::Gold => "gold",
        OaStatus::Green => "green",
        OaStatus::Hybrid => "hybrid",
        OaStatus::Bronze => "bronze",
        OaStatus::Diamond => "diamond",
        OaStatus::Closed => "closed",
    }
}

fn source_type_str(t: &SourceType) -> &'static str {
    match t {
        SourceType::Journal => "journal",
        SourceType::Repository => "repository",
        SourceType::Conference => "conference",
        SourceType::BookSeries => "book series",
        SourceType::EbookPlatform => "ebook platform",
        SourceType::Metadata => "metadata",
        SourceType::Other => "other",
    }
}

fn position_str(t: &Position) -> &'static str {
    match t {
        Position::First => "first",
        Position::Last => "last",
        Position::Middle => "middle",
    }
}

fn inst_type_str(t: &InstType) -> &'static str {
    match t {
        InstType::Education => "education",
        InstType::Company => "company",
        InstType::Government => "government",
        InstType::Nonprofit => "nonprofit",
        InstType::Healthcare => "healthcare",
        InstType::Facility => "facility",
        InstType::Funder => "funder",
        InstType::Archive => "archive",
        InstType::Other => "other",
    }
}

impl From<&Work> for WorkVx {
    fn from(r: &Work) -> Self {
        WorkVx {
            id: r.id,
            doi: r.doi.clone(),
            title: r.title.clone(),
            publication_year: r.publication_year,
            publication_date: r.publication_date.clone(),
            language: r.language.as_deref().map(str::to_string),
            work_type: work_type_str(&r.work_type).to_string(),
            cited_by_count: r.cited_by_count,
            fwci: r.fwci,
            is_retracted: r.is_retracted,
            oa_status: oa_status_str(&r.oa_status).to_string(),
            is_oa: r.is_oa,
            oa_url: r.oa_url.clone(),
            source: r.source.clone(),
            source_type: r.source_type.map(|t| source_type_str(&t).to_string()),
            authorships: r.authorships.iter().map(AuthorshipVx::from).collect(),
            topics: r.topics.iter().map(TopicVx::from).collect(),
            keywords: r.keywords.iter().map(KwVx::from).collect(),
            abstract_inverted_index: abstract_kv(&r.abstract_inverted_index),
            updated_date: r.updated_date.clone(),
            created_date: r.created_date.clone(),
        }
    }
}

impl From<&Authorship> for AuthorshipVx {
    fn from(r: &Authorship) -> Self {
        AuthorshipVx {
            author_id: r.author_id,
            author_name: r.author_name.clone(),
            position: position_str(&r.position).to_string(),
            institutions: r.institutions.iter().map(InstitutionVx::from).collect(),
            countries: r.countries.iter().map(|c| c.to_string()).collect(),
            is_corresponding: r.is_corresponding,
        }
    }
}

impl From<&Institution> for InstitutionVx {
    fn from(r: &Institution) -> Self {
        InstitutionVx {
            id: r.id,
            name: r.name.clone(),
            ror: r.ror.as_deref().map(str::to_string),
            country: r.country.as_deref().map(str::to_string),
            inst_type: r.inst_type.map(|t| inst_type_str(&t).to_string()),
        }
    }
}

impl From<&Topic> for TopicVx {
    fn from(r: &Topic) -> Self {
        TopicVx {
            id: r.id,
            name: r.name.clone(),
            score: r.score,
            subfield_id: r.subfield_id,
            field_id: r.field_id,
            domain_id: r.domain_id,
        }
    }
}

impl From<&Kw> for KwVx {
    fn from(r: &Kw) -> Self {
        KwVx { id: r.id.to_string(), name: r.name.clone(), score: r.score }
    }
}

// ---- raw JSON input (OpenAlex API shape, verbatim) -> bench structs:
// flattens open_access.* / primary_location.source.*, ids/urls -> payloads ----

#[derive(serde::Deserialize)]
struct WorkJson {
    id: String,
    doi: Option<String>,
    title: Option<String>,
    publication_year: Option<i32>,
    publication_date: Option<String>,
    language: Option<String>,
    #[serde(rename = "type")]
    work_type: String,
    cited_by_count: i32,
    fwci: Option<f32>,
    is_retracted: Option<bool>,
    open_access: OaJson,
    primary_location: Option<PrimaryLocationJson>,
    #[serde(default)]
    authorships: Vec<AuthorshipJson>,
    #[serde(default)]
    topics: Vec<TopicJson>,
    #[serde(default)]
    keywords: Vec<KwJson>,
    abstract_inverted_index: Option<HashMap<String, Vec<i32>>>,
    updated_date: String,
    created_date: String,
}

#[derive(serde::Deserialize)]
struct OaJson {
    oa_status: String,
    is_oa: bool,
    oa_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct PrimaryLocationJson {
    source: Option<SourceJson>,
}

#[derive(serde::Deserialize)]
struct SourceJson {
    display_name: String,
    #[serde(rename = "type")]
    source_type: Option<String>,
}

#[derive(serde::Deserialize)]
struct AuthorshipJson {
    author: AuthorJson,
    author_position: String,
    institutions: Option<Vec<InstitutionJson>>,
    countries: Option<Vec<String>>,
    is_corresponding: bool,
}

#[derive(serde::Deserialize)]
struct AuthorJson {
    id: Option<String>,
    display_name: String,
}

#[derive(serde::Deserialize)]
struct InstitutionJson {
    id: Option<String>,
    display_name: Option<String>,
    ror: Option<String>,
    country_code: Option<String>,
    #[serde(rename = "type")]
    inst_type: Option<String>,
}

#[derive(serde::Deserialize)]
struct TopicJson {
    id: String,
    display_name: String,
    score: f32,
    subfield: Option<IdJson>,
    field: Option<IdJson>,
    domain: Option<IdJson>,
}

#[derive(serde::Deserialize)]
struct KwJson {
    id: String,
    display_name: String,
    score: f32,
}

#[derive(serde::Deserialize)]
struct IdJson {
    id: String,
}

/// ".../W2741809807" -> 2741809807: digit tail after the last `/`, with a
/// type-letter prefix (works/authors/sources) or bare digits
/// (domains/fields/subfields).
fn openalex_id(s: &str) -> i64 {
    let tail = s.rsplit('/').next().unwrap_or(s);
    let digits = tail.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    digits.parse().unwrap_or_else(|_| panic!("openalex id {s:?} has no numeric tail"))
}

fn strip_prefix(s: &str, prefix: &str) -> String {
    s.strip_prefix(prefix).unwrap_or(s).to_string()
}

fn date_midnight(s: &str) -> DateTime<Utc> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .unwrap_or_else(|_| panic!("publication_date {s:?} is not YYYY-MM-DD"))
        .and_hms_opt(0, 0, 0)
        .expect("midnight exists")
        .and_utc()
}

fn rfc3339(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap_or_else(|_| panic!("timestamp {s:?} is not RFC 3339")).with_timezone(&Utc)
}

/// Unknown vocab values panic with the offending value: the enums have no
/// catchall variant by design.
fn work_type(s: &str) -> WorkType {
    match s {
        "article" => WorkType::Article,
        "preprint" => WorkType::Preprint,
        "conference-paper" => WorkType::ConferencePaper,
        "conference-abstract" => WorkType::ConferenceAbstract,
        "other" => WorkType::Other,
        "review" => WorkType::Review,
        "book-chapter" => WorkType::BookChapter,
        "erratum" => WorkType::Erratum,
        "paratext" => WorkType::Paratext,
        "report" => WorkType::Report,
        "data-paper" => WorkType::DataPaper,
        "book" => WorkType::Book,
        "dataset" => WorkType::Dataset,
        "peer-review" => WorkType::PeerReview,
        "book-review" => WorkType::BookReview,
        "dissertation" => WorkType::Dissertation,
        "editorial" => WorkType::Editorial,
        "software" => WorkType::Software,
        "letter" => WorkType::Letter,
        "reference-entry" => WorkType::ReferenceEntry,
        "supplementary-materials" => WorkType::SupplementaryMaterials,
        "retraction" => WorkType::Retraction,
        "standard" => WorkType::Standard,
        "software-paper" => WorkType::SoftwarePaper,
        "libguides" => WorkType::Libguides,
        "database" => WorkType::Database,
        other => panic!("unknown work_type: {other:?}"),
    }
}

fn oa_status(s: &str) -> OaStatus {
    match s {
        "gold" => OaStatus::Gold,
        "green" => OaStatus::Green,
        "hybrid" => OaStatus::Hybrid,
        "bronze" => OaStatus::Bronze,
        "diamond" => OaStatus::Diamond,
        "closed" => OaStatus::Closed,
        other => panic!("unknown oa_status: {other:?}"),
    }
}

fn source_type(s: &str) -> SourceType {
    match s {
        "journal" => SourceType::Journal,
        "repository" => SourceType::Repository,
        "conference" => SourceType::Conference,
        "book series" => SourceType::BookSeries,
        "ebook platform" => SourceType::EbookPlatform,
        "metadata" => SourceType::Metadata,
        "other" => SourceType::Other,
        other => panic!("unknown source_type: {other:?}"),
    }
}

fn position(s: &str) -> Position {
    match s {
        "first" => Position::First,
        "last" => Position::Last,
        "middle" => Position::Middle,
        other => panic!("unknown author_position: {other:?}"),
    }
}

fn inst_type(s: &str) -> InstType {
    match s {
        "education" => InstType::Education,
        "company" => InstType::Company,
        "government" => InstType::Government,
        "nonprofit" => InstType::Nonprofit,
        "healthcare" => InstType::Healthcare,
        "facility" => InstType::Facility,
        "funder" => InstType::Funder,
        "archive" => InstType::Archive,
        "other" => InstType::Other,
        other => panic!("unknown institution type: {other:?}"),
    }
}

impl From<WorkJson> for Work {
    fn from(j: WorkJson) -> Self {
        let source = j.primary_location.and_then(|p| p.source);
        Work {
            id: openalex_id(&j.id),
            doi: j.doi.map(|d| strip_prefix(&d, "https://doi.org/")),
            title: j.title,
            publication_year: j.publication_year,
            publication_date: j.publication_date.map(|d| date_midnight(&d)),
            language: j.language.map(SmolStr::new),
            work_type: work_type(&j.work_type),
            cited_by_count: j.cited_by_count,
            fwci: j.fwci,
            is_retracted: j.is_retracted,
            oa_status: oa_status(&j.open_access.oa_status),
            is_oa: j.open_access.is_oa,
            oa_url: j.open_access.oa_url,
            source: source.as_ref().map(|s| s.display_name.clone()),
            source_type: source.as_ref().and_then(|s| s.source_type.as_deref()).map(|t| source_type(t)),
            authorships: j.authorships.into_iter().map(Authorship::from).collect(),
            topics: j.topics.into_iter().map(Topic::from).collect(),
            keywords: j.keywords.into_iter().map(Kw::from).collect(),
            abstract_inverted_index: j
                .abstract_inverted_index
                .map(|m| m.iter().map(|(k, v)| (SmolStr::new(k), v.clone())).collect()),
            updated_date: rfc3339(&j.updated_date),
            created_date: rfc3339(&j.created_date),
        }
    }
}

impl From<AuthorshipJson> for Authorship {
    fn from(j: AuthorshipJson) -> Self {
        Authorship {
            author_id: j.author.id.as_deref().map(openalex_id),
            author_name: j.author.display_name,
            position: position(&j.author_position),
            institutions: j.institutions.unwrap_or_default().into_iter().map(Institution::from).collect(),
            countries: j.countries.unwrap_or_default().into_iter().map(SmolStr::new).collect(),
            is_corresponding: j.is_corresponding,
        }
    }
}

impl From<InstitutionJson> for Institution {
    fn from(j: InstitutionJson) -> Self {
        Institution {
            id: j.id.as_deref().map(openalex_id),
            name: j.display_name,
            ror: j.ror.map(|r| SmolStr::new(strip_prefix(&r, "https://ror.org/").as_str())),
            country: j.country_code.map(SmolStr::new),
            inst_type: j.inst_type.as_deref().map(inst_type),
        }
    }
}

impl From<TopicJson> for Topic {
    fn from(j: TopicJson) -> Self {
        Topic {
            id: openalex_id(&j.id),
            name: j.display_name,
            score: j.score,
            subfield_id: j.subfield.map(|s| openalex_id(&s.id)),
            field_id: j.field.map(|s| openalex_id(&s.id)),
            domain_id: j.domain.map(|s| openalex_id(&s.id)),
        }
    }
}

impl From<KwJson> for Kw {
    fn from(j: KwJson) -> Self {
        Kw {
            id: SmolStr::new(strip_prefix(&j.id, "https://openalex.org/keywords/").as_str()),
            name: j.display_name,
            score: j.score,
        }
    }
}

// ---- load: gzip + serde_json per line, two pinned parts ----

fn load() -> Vec<Work> {
    let mut works: Vec<Work> = Vec::new();
    for (ds, expected) in [(DS_0, 400_000usize), (DS_1, 232_276)] {
        let path = ds.ensure_downloaded();
        let file = std::fs::File::open(&path).unwrap();
        let gz = flate2::read::GzDecoder::new(file);
        let reader = std::io::BufReader::new(gz);
        let mut n = 0usize;
        for line in reader.lines() {
            let line = line.unwrap_or_else(|e| panic!("gzip line in {}: {e}", ds.name));
            let wj: WorkJson =
                serde_json::from_str(&line).unwrap_or_else(|e| panic!("work json in {}: {e}\n{line}", ds.name));
            works.push(Work::from(wj));
            n += 1;
        }
        // The part files are immutable, so a pinned-count mismatch means
        // a corrupt download.
        assert_eq!(n, expected, "{} record count", ds.name);
    }
    works
}

// ---- verify: work ids are unique, so sorting both sides by id is order-safe ----

fn verify(data: &[Work], back: &[Work]) {
    let mut a = back.to_vec();
    let mut b = data.to_vec();
    a.sort_by_key(|w| w.id);
    b.sort_by_key(|w| w.id);
    assert_eq!(a, b, "openalex roundtrip mismatch");
}

fn verify_lossy(data: &[Work], back: &[WorkF]) {
    let mut expected: Vec<WorkF> = data
        .iter()
        .map(|w| {
            let mut f = WorkF::from(w);
            f.fwci = f.fwci.map(|v| common::round_f32(v, 4));
            for t in &mut f.topics {
                t.score = common::round_f32(t.score, 4);
            }
            for k in &mut f.keywords {
                k.score = common::round_f32(k.score, 4);
            }
            f
        })
        .collect();
    let mut got = back.to_vec();
    expected.sort_by_key(|w| w.id);
    got.sort_by_key(|w| w.id);
    assert_eq!(got, expected, "openalex lossy roundtrip mismatch");
}
