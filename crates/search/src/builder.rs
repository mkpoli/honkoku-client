use crate::{Error, Fields, Result, bytes, normalize, register, schema, string};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tantivy::{
    Index, IndexWriter, TantivyDocument, Term, collector::DocSetCollector, query::AllQuery,
};
use walkdir::WalkDir;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LivePage {
    pub page_id: String,
    pub project_id: String,
    pub entry_id: String,
    pub index: u64,
    pub text: String,
    pub updated_at: i64,
    #[serde(default)]
    pub entry_label: String,
    #[serde(default)]
    pub project_title: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub indexed: u64,
}
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Metadata {
    pub commit: Option<String>,
    pub last_build: Option<i64>,
}
pub(crate) fn metadata(path: &Path) -> Result<Metadata> {
    match std::fs::read(path.join("build.json")) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Metadata::default()),
        Err(e) => Err(e.into()),
    }
}
pub fn dump_commit(path: impl AsRef<Path>) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path.as_ref())
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err(Error::Invalid("dump folder must be a git clone".into()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn labels(path: &Path, field: &str) -> Result<HashMap<String, String>> {
    let info = path.join("info.tsv");
    let mut labels = HashMap::new();
    if !info.exists() {
        return Ok(labels);
    }
    // The dump writes literal newlines in unused attribution columns.
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .quoting(false)
        .flexible(true)
        .from_path(info)?;
    let headers = reader.headers()?.clone();
    let id = headers
        .iter()
        .position(|h| h == "id")
        .ok_or_else(|| Error::Invalid("metadata has no id column".into()))?;
    let label = headers
        .iter()
        .position(|h| h == field)
        .ok_or_else(|| Error::Invalid(format!("metadata has no {field} column")))?;
    for row in reader.records() {
        let row = row?;
        if let (Some(id), Some(value)) = (row.get(id), row.get(label))
            && !id.is_empty()
            && !id.contains(['/', '\\'])
            && path.join(id).is_dir()
        {
            labels.insert(id.to_owned(), value.to_owned());
        }
    }
    Ok(labels)
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
struct Record {
    id: String,
    hash: Vec<u8>,
    source: String,
}
pub struct IndexBuilder {
    path: PathBuf,
    fields: Fields,
    writer: IndexWriter,
    ledger: Connection,
    pending: Vec<Record>,
}
impl IndexBuilder {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)?;
        let (expected, fields) = schema();
        let index = if path.join("meta.json").exists() {
            Index::open_in_dir(path)?
        } else {
            Index::create_in_dir(path, expected.clone())?
        };
        if index.schema() != expected {
            return Err(Error::Invalid(
                "search index format changed; rebuild the index".into(),
            ));
        }
        register(&index);
        let writer = index.writer_with_options(
            tantivy::indexer::IndexWriterOptions::builder()
                .num_worker_threads(1)
                .num_merge_threads(1)
                .memory_budget_per_thread(64_000_000)
                .build(),
        )?;
        let ledger = Connection::open(path.join("pages.sqlite"))?;
        ledger.execute_batch("PRAGMA journal_mode=WAL; PRAGMA cache_size=-4096; CREATE TABLE IF NOT EXISTS pages(id TEXT PRIMARY KEY,hash BLOB NOT NULL,source TEXT NOT NULL); CREATE TABLE IF NOT EXISTS checkpoint(stamp INTEGER NOT NULL);")?;
        let saved: Option<i64> = ledger
            .query_row("SELECT stamp FROM checkpoint", [], |r| r.get(0))
            .optional()?;
        if saved != Some(index.load_metas()?.opstamp as i64) {
            let transaction = ledger.unchecked_transaction()?;
            transaction.execute("DELETE FROM pages", [])?;
            let searcher = index.reader()?.searcher();
            let mut docs: Vec<_> = searcher
                .search(&AllQuery, &DocSetCollector)?
                .into_iter()
                .collect();
            docs.sort_unstable();
            for address in docs {
                let doc: TantivyDocument = searcher.doc(address)?;
                transaction.execute(
                    "INSERT OR REPLACE INTO pages VALUES(?1,?2,?3)",
                    params![
                        string(&doc, fields.page_id),
                        bytes(&doc, fields.text_hash),
                        string(&doc, fields.source)
                    ],
                )?;
            }
            transaction.execute("DELETE FROM checkpoint", [])?;
            transaction.execute(
                "INSERT INTO checkpoint VALUES(?)",
                [index.load_metas()?.opstamp as i64],
            )?;
            transaction.commit()?;
        }
        Ok(Self {
            path: path.to_owned(),
            fields,
            writer,
            ledger,
            pending: Vec::new(),
        })
    }
    fn upsert(&mut self, page: &LivePage, source: &str) -> Result<bool> {
        if page.page_id != format!("{}_{}", page.entry_id, page.index) || page.project_id.is_empty()
        {
            return Err(Error::Invalid("invalid page identity".into()));
        }
        let hash = blake3::hash(page.text.as_bytes());
        let old: Option<(Vec<u8>, String)> = if let Some(record) = self
            .pending
            .iter()
            .rev()
            .find(|record| record.id == page.page_id)
        {
            Some((record.hash.clone(), record.source.clone()))
        } else {
            self.ledger
                .query_row(
                    "SELECT hash,source FROM pages WHERE id=?",
                    [&page.page_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?
        };
        if let Some((old_hash, old_source)) = old
            && (source == "dump" && old_source == "live"
                || old_hash == hash.as_bytes() && old_source == source)
        {
            return Ok(false);
        }
        let strict = normalize::strict(&page.text);
        let folded = normalize::fold(&strict);
        let f = &self.fields;
        let mut doc = TantivyDocument::default();
        for (field, value) in [
            (f.page_id, page.page_id.as_str()),
            (f.project_id, &page.project_id),
            (f.entry_id, &page.entry_id),
            (f.strict, &strict.text),
            (f.strict2, &strict.text),
            (f.folded, &folded.text),
            (f.folded2, &folded.text),
            (f.source, source),
            (f.entry_label, &page.entry_label),
            (f.project_title, &page.project_title),
        ] {
            doc.add_text(field, value);
        }
        doc.add_u64(f.index, page.index);
        doc.add_i64(f.updated_at, page.updated_at);
        doc.add_bytes(f.original, page.text.as_bytes());
        doc.add_bytes(f.strict_map, &crate::maps::encode(&strict.offsets));
        doc.add_bytes(f.folded_map, &crate::maps::encode(&folded.offsets));
        doc.add_bytes(f.text_hash, hash.as_bytes());
        self.writer
            .delete_term(Term::from_field_text(f.page_id, &page.page_id));
        self.writer.add_document(doc)?;
        self.pending.push(Record {
            id: page.page_id.clone(),
            hash: hash.as_bytes().to_vec(),
            source: source.into(),
        });
        Ok(true)
    }
    fn commit(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let stamp = self.writer.commit()?;
        let transaction = self.ledger.unchecked_transaction()?;
        for record in &self.pending {
            transaction.execute(
                "INSERT OR REPLACE INTO pages VALUES(?1,?2,?3)",
                params![record.id, record.hash, record.source],
            )?;
        }
        transaction.execute("DELETE FROM checkpoint", [])?;
        transaction.execute("INSERT INTO checkpoint VALUES(?)", [stamp as i64])?;
        transaction.commit()?;
        self.pending.clear();
        Ok(())
    }
    /// Import only numeric page files directly below project/entry directories.
    pub fn from_dump(
        &mut self,
        path: impl AsRef<Path>,
        commit: &str,
        mut progress: impl FnMut(Progress),
    ) -> Result<Progress> {
        let root = path.as_ref().join("v3");
        if !root.is_dir() {
            return Err(Error::Invalid("dump folder has no v3 directory".into()));
        }
        let page_index = |entry: &walkdir::DirEntry| -> Option<u64> {
            if entry.depth() != 3
                || !entry.file_type().is_file()
                || entry.path().extension()?.to_str()? != "txt"
            {
                return None;
            }
            let stem = entry.path().file_stem()?.to_str()?;
            if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            stem.parse().ok()
        };
        let mut total = 0;
        for entry in WalkDir::new(&root).min_depth(3).max_depth(3) {
            if page_index(&entry?).is_some() {
                total += 1;
            }
        }
        let mut result = Progress {
            done: 0,
            total,
            indexed: 0,
        };
        progress(result.clone());
        let mut entry_labels = HashMap::new();
        let project_titles = labels(&root, "title")?;
        let mut previous_project = String::new();
        for item in WalkDir::new(&root)
            .min_depth(3)
            .max_depth(3)
            .sort_by_file_name()
        {
            let item = item?;
            let Some(index) = page_index(&item) else {
                continue;
            };
            let relative = item
                .path()
                .strip_prefix(&root)
                .map_err(|_| Error::Invalid("invalid dump path".into()))?;
            let parts: Vec<_> = relative.iter().collect();
            let project_id = parts[0].to_string_lossy().into_owned();
            let entry_id = parts[1].to_string_lossy().into_owned();
            if project_id != previous_project {
                entry_labels = labels(&root.join(&project_id), "label")?;
                previous_project = project_id.clone();
            }
            let page = LivePage {
                page_id: format!("{entry_id}_{index}"),
                entry_label: entry_labels
                    .get(&entry_id)
                    .cloned()
                    .unwrap_or_else(|| entry_id.clone()),
                project_title: project_titles
                    .get(&project_id)
                    .cloned()
                    .unwrap_or_else(|| project_id.clone()),
                project_id,
                entry_id,
                index,
                text: std::fs::read_to_string(item.path())?,
                updated_at: 0,
            };
            if self.upsert(&page, "dump")? {
                result.indexed += 1;
            }
            result.done += 1;
            if self.pending.len() >= 2000 {
                self.commit()?;
            }
            if result.done.is_multiple_of(1000) {
                progress(result.clone());
            }
        }
        self.commit()?;
        let meta = Metadata {
            commit: Some(commit.into()),
            last_build: Some(now()),
        };
        std::fs::write(self.path.join("build.json.tmp"), serde_json::to_vec(&meta)?)?;
        std::fs::rename(
            self.path.join("build.json.tmp"),
            self.path.join("build.json"),
        )?;
        progress(result.clone());
        Ok(result)
    }
    pub fn apply_live(&mut self, pages: impl IntoIterator<Item = LivePage>) -> Result<u64> {
        let mut count = 0;
        for page in pages {
            if self.upsert(&page, "live")? {
                count += 1;
            }
            if self.pending.len() >= 2000 {
                self.commit()?;
            }
        }
        self.commit()?;
        Ok(count)
    }
    pub fn remove(&mut self, page_id: &str) -> Result<()> {
        self.commit()?;
        self.writer
            .delete_term(Term::from_field_text(self.fields.page_id, page_id));
        let stamp = self.writer.commit()?;
        let transaction = self.ledger.unchecked_transaction()?;
        transaction.execute("DELETE FROM pages WHERE id=?", [page_id])?;
        transaction.execute("DELETE FROM checkpoint", [])?;
        transaction.execute("INSERT INTO checkpoint VALUES(?)", [stamp as i64])?;
        transaction.commit()?;
        Ok(())
    }
    pub fn finish(self) -> Result<()> {
        self.writer.wait_merging_threads()?;
        self.ledger
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        Ok(())
    }
}
