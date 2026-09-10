//! Positional concordance index with original-text verification.
mod builder;
mod maps;
mod tokenizer;
pub use builder::{IndexBuilder, LivePage, Progress, dump_commit};
use honkoku_text::normalize::{self, SEPARATOR};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};
use tantivy::{
    Index, IndexReader, TantivyDocument, Term,
    collector::DocSetCollector,
    query::{BooleanQuery, Occur, PhraseQuery, Query as TantivyQuery, TermQuery},
    schema::*,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("index: {0}")]
    Index(#[from] tantivy::TantivyError),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("metadata: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TSV: {0}")]
    Csv(#[from] csv::Error),
    #[error("dump traversal: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("{0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Strict,
    Folded,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    pub text: String,
    #[serde(default)]
    pub mode: Mode,
    pub project: Option<String>,
    pub entry: Option<String>,
    pub limit: usize,
    pub cursor: Option<String>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Results {
    pub total: u64,
    pub hits: Vec<Hit>,
    pub facets: Vec<(String, u64)>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Hit {
    pub page_id: String,
    pub project_id: String,
    pub entry_id: String,
    pub index: u64,
    pub entry_label: String,
    pub project_title: String,
    pub occurrences: Vec<Occurrence>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Occurrence {
    pub original_start: u32,
    pub original_end: u32,
    pub before: String,
    pub matched: String,
    pub after: String,
    pub column: usize,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Status {
    pub present: bool,
    pub commit: Option<String>,
    pub page_count: u64,
    pub last_build: Option<i64>,
    pub size: u64,
}
#[derive(Clone)]
struct Fields {
    page_id: Field,
    project_id: Field,
    entry_id: Field,
    index: Field,
    strict: Field,
    folded: Field,
    strict2: Field,
    folded2: Field,
    original: Field,
    strict_map: Field,
    folded_map: Field,
    source: Field,
    updated_at: Field,
    text_hash: Field,
    entry_label: Field,
    project_title: Field,
}
fn schema() -> (Schema, Fields) {
    let mut s = Schema::builder();
    let text = |tokenizer| {
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(tokenizer)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                .set_fieldnorms(false),
        )
    };
    let f = Fields {
        page_id: s.add_text_field("page_id", STRING | STORED),
        project_id: s.add_text_field("project_id", STRING | FAST | STORED),
        entry_id: s.add_text_field("entry_id", STRING | FAST | STORED),
        index: s.add_u64_field("index", FAST | STORED),
        strict: s.add_text_field("strict", text("scalar").set_stored()),
        folded: s.add_text_field("folded", text("scalar").set_stored()),
        strict2: s.add_text_field("strict2", text("pair")),
        folded2: s.add_text_field("folded2", text("pair")),
        original: s.add_bytes_field("original", STORED),
        strict_map: s.add_bytes_field("strict_map", STORED),
        folded_map: s.add_bytes_field("folded_map", STORED),
        source: s.add_text_field("source", STRING | STORED),
        updated_at: s.add_i64_field("updated_at", FAST),
        text_hash: s.add_bytes_field("text_hash", STORED),
        entry_label: s.add_text_field("entry_label", STORED),
        project_title: s.add_text_field("project_title", STORED),
    };
    (s.build(), f)
}
fn register(index: &Index) {
    index
        .tokenizers()
        .register("scalar", tokenizer::Characters(1));
    index
        .tokenizers()
        .register("pair", tokenizer::Characters(2));
}
fn string(doc: &TantivyDocument, field: Field) -> &str {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
}
fn bytes(doc: &TantivyDocument, field: Field) -> &[u8] {
    doc.get_first(field)
        .and_then(|v| v.as_bytes())
        .unwrap_or_default()
}
pub fn status(path: impl AsRef<Path>) -> Result<Status> {
    let path = path.as_ref();
    if !path.join("meta.json").exists() {
        return Ok(Status::default());
    }
    let index = Index::open_in_dir(path)?;
    let meta = builder::metadata(path)?;
    let page_count = index.reader()?.searcher().num_docs();
    let mut size = 0;
    for item in walkdir::WalkDir::new(path) {
        let item = item?;
        if item.file_type().is_file() {
            size += std::fs::metadata(item.path())?.len();
        }
    }
    Ok(Status {
        present: true,
        commit: meta.commit,
        page_count,
        last_build: meta.last_build,
        size,
    })
}
pub struct Searcher {
    reader: IndexReader,
    fields: Fields,
}
impl Searcher {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let index = Index::open_in_dir(path)?;
        let (expected, fields) = schema();
        if index.schema() != expected {
            return Err(Error::Invalid(
                "search index format changed; rebuild the index".into(),
            ));
        }
        register(&index);
        Ok(Self {
            reader: index.reader()?,
            fields,
        })
    }
    pub fn search(&self, query: Query) -> Result<Results> {
        if query.limit == 0 || query.limit > 1000 {
            return Err(Error::Invalid("limit must be between 1 and 1000".into()));
        }
        let normalized = match query.mode {
            Mode::Strict => normalize::strict(&query.text),
            Mode::Folded => normalize::folded(&query.text),
        };
        if normalized.text.is_empty() {
            return Ok(Results::default());
        }
        if normalized.text.contains(SEPARATOR) {
            return Err(Error::Invalid("search text must fit on one line".into()));
        }
        let chars: Vec<_> = normalized.text.chars().collect();
        if chars.len() > 256 {
            return Err(Error::Invalid("search text exceeds 256 characters".into()));
        }
        let scope = blake3::hash(&serde_json::to_vec(&(
            &normalized.text,
            query.mode,
            &query.project,
            &query.entry,
        ))?)
        .to_hex()
        .to_string();
        let after_id = if let Some(cursor) = &query.cursor {
            let (hash, page): (String, String) = serde_json::from_str(cursor)
                .map_err(|_| Error::Invalid("invalid search cursor".into()))?;
            if hash != scope {
                return Err(Error::Invalid("cursor belongs to a different query".into()));
            }
            page
        } else {
            String::new()
        };
        let f = &self.fields;
        let (single, pair, map) = if query.mode == Mode::Strict {
            (f.strict, f.strict2, f.strict_map)
        } else {
            (f.folded, f.folded2, f.folded_map)
        };
        let candidate: Box<dyn TantivyQuery> = match chars.len() {
            1 => Box::new(TermQuery::new(
                Term::from_field_text(single, &normalized.text),
                IndexRecordOption::Basic,
            )),
            2 => Box::new(TermQuery::new(
                Term::from_field_text(pair, &normalized.text),
                IndexRecordOption::Basic,
            )),
            _ => Box::new(PhraseQuery::new(
                chars
                    .windows(2)
                    .map(|w| Term::from_field_text(pair, &w.iter().collect::<String>()))
                    .collect(),
            )),
        };
        let mut clauses = vec![(Occur::Must, candidate)];
        for (field, filter) in [(f.project_id, &query.project), (f.entry_id, &query.entry)] {
            if let Some(value) = filter {
                clauses.push((
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(field, value),
                        IndexRecordOption::Basic,
                    )),
                ));
            }
        }
        self.reader.reload()?;
        let searcher = self.reader.searcher();
        let mut addresses: Vec<_> = searcher
            .search(&BooleanQuery::new(clauses), &DocSetCollector)?
            .into_iter()
            .collect();
        addresses.sort_unstable();
        let mut total = 0;
        let mut facets: HashMap<String, u64> = HashMap::new();
        let mut selected: BTreeMap<String, Hit> = BTreeMap::new();
        let mut remaining = 0;
        for address in addresses {
            let doc: TantivyDocument = searcher.doc(address)?;
            let normalized_page = string(&doc, single);
            if !normalized_page.contains(&normalized.text) {
                continue;
            }
            total += 1;
            *facets
                .entry(string(&doc, f.project_id).to_owned())
                .or_default() += 1;
            let id = string(&doc, f.page_id);
            if id <= after_id.as_str() {
                continue;
            }
            remaining += 1;
            if selected.len() == query.limit
                && selected
                    .last_key_value()
                    .is_some_and(|(last, _)| id > last.as_str())
            {
                continue;
            }
            let original = std::str::from_utf8(bytes(&doc, f.original))
                .map_err(|_| Error::Invalid("invalid stored UTF-8".into()))?;
            let offsets = maps::decode(bytes(&doc, map))?;
            let occurrences = occurrences(original, normalized_page, &normalized.text, &offsets)?;
            let hit = Hit {
                page_id: id.to_owned(),
                project_id: string(&doc, f.project_id).into(),
                entry_id: string(&doc, f.entry_id).into(),
                index: doc
                    .get_first(f.index)
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default(),
                entry_label: string(&doc, f.entry_label).into(),
                project_title: string(&doc, f.project_title).into(),
                occurrences,
            };
            selected.insert(id.into(), hit);
            if selected.len() > query.limit {
                selected.pop_last();
            }
        }
        let next_cursor = if remaining > selected.len() {
            selected
                .last_key_value()
                .map(|(id, _)| serde_json::to_string(&(&scope, id)))
                .transpose()?
        } else {
            None
        };
        let mut facets: Vec<_> = facets.into_iter().collect();
        facets.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        facets.truncate(10);
        Ok(Results {
            total,
            hits: selected.into_values().collect(),
            facets,
            next_cursor,
        })
    }
}
fn occurrences(
    original: &str,
    normalized: &str,
    needle: &str,
    offsets: &[normalize::Span],
) -> Result<Vec<Occurrence>> {
    let original: Vec<_> = original.chars().collect();
    let width = needle.chars().count();
    let mut out = Vec::new();
    for (position, (byte, _)) in normalized.char_indices().enumerate() {
        if !normalized[byte..].starts_with(needle) {
            continue;
        }
        let start = offsets
            .get(position)
            .ok_or_else(|| Error::Invalid("missing match offset".into()))?
            .start as usize;
        let end = offsets
            .get(position + width - 1)
            .ok_or_else(|| Error::Invalid("missing match end".into()))?
            .end as usize;
        if start >= end || end > original.len() {
            return Err(Error::Invalid("match outside original text".into()));
        }
        if out.last().is_some_and(|o: &Occurrence| {
            o.original_start == start as u32 && o.original_end == end as u32
        }) {
            continue;
        }
        let prefix: String = original[..start].iter().collect();
        let completed_lines = prefix
            .rsplit_once(['\r', '\n'])
            .map_or("", |(lines, _)| lines);
        let column = completed_lines
            .split(['\r', '\n'])
            .filter(|line| {
                !line.trim().is_empty() && !matches!(line.trim(), "【右丁】" | "【左丁】")
            })
            .count();
        out.push(Occurrence {
            original_start: start as u32,
            original_end: end as u32,
            before: original[start.saturating_sub(20)..start].iter().collect(),
            matched: original[start..end].iter().collect(),
            after: original[end..(end + 20).min(original.len())]
                .iter()
                .collect(),
            column,
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests;
