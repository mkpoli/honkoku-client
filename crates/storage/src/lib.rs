//! Synchronous SQLite persistence. Callers schedule access off the async runtime.
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, path::Path, time::Duration};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid cache record: {0}")]
    Invalid(String),
    #[error("timestamp: {0}")]
    Time(#[from] time::error::Format),
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Storage {
    connection: Connection,
}
impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }
    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);")?;
        let transaction = connection.transaction()?;
        let version: u32 = transaction.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;
        let migrations = [
            (1, include_str!("../migrations/001_cache.sql")),
            (2, include_str!("../migrations/002_home.sql")),
            (3, include_str!("../migrations/003_progress.sql")),
            (4, include_str!("../migrations/004_ocr.sql")),
        ];
        if version > migrations.len() as u32 {
            return Err(Error::Invalid(
                "database schema is newer than this client".into(),
            ));
        }
        for (number, sql) in migrations {
            if number > version {
                transaction.execute_batch(sql)?;
                transaction.execute("INSERT INTO schema_version (version) VALUES (?)", [number])?;
            }
        }
        transaction.commit()?;
        Ok(Self { connection })
    }
    /// Atomic cache update; failure rolls back rows and response freshness together.
    pub fn transaction<T>(&mut self, operation: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        let transaction = self.connection.unchecked_transaction()?;
        let result = operation(self)?;
        transaction.commit()?;
        Ok(result)
    }
    pub fn put_ocr<T: Serialize>(
        &self,
        page_id: &str,
        model_version: &str,
        payload: &T,
    ) -> Result<()> {
        self.connection.execute("INSERT INTO ocr_results(page_id,model_version,payload,created_at) VALUES (?1,?2,?3,?4) ON CONFLICT(page_id,model_version) DO UPDATE SET payload=excluded.payload,created_at=excluded.created_at", params![page_id,model_version,serde_json::to_string(payload)?,now()?])?;
        Ok(())
    }
    pub fn ocr_result<T: DeserializeOwned>(&self, page_id: &str) -> Result<Option<T>> {
        let payload: Option<String> = self.connection.query_row("SELECT payload FROM ocr_results WHERE page_id=? ORDER BY created_at DESC,model_version DESC LIMIT 1", [page_id], |row| row.get(0)).optional()?;
        payload
            .map(|text| serde_json::from_str(&text).map_err(Error::from))
            .transpose()
    }
    fn put<T: Serialize>(&self, table: &str, record: &T) -> Result<()> {
        let value = serde_json::to_value(record)?;
        let field = |name: &str| {
            value[name]
                .as_str()
                .ok_or_else(|| Error::Invalid(format!("missing {name}")))
        };
        let id = field(if table == "users" { "uid" } else { "id" })?;
        let payload = serde_json::to_string(&value)?;
        let updated = value["updatedAt"].as_str();
        let fetched = now()?;
        match table {
            "projects" | "users" => {
                self.connection.execute(&format!("INSERT INTO {table} (id,payload,updated_at,fetched_at) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,updated_at=excluded.updated_at,fetched_at=excluded.fetched_at"), params![id,payload,updated,fetched])?;
            }
            "pages" => {
                self.connection.execute("INSERT INTO pages (id,parent_id,entry_id,idx,status,payload,updated_at,fetched_at) VALUES (?1,?2,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,entry_id=excluded.entry_id,idx=excluded.idx,status=excluded.status,payload=excluded.payload,updated_at=excluded.updated_at,fetched_at=excluded.fetched_at", params![id,field("entryId")?,value["index"].as_i64().ok_or_else(|| Error::Invalid("missing index".into()))?,field("status")?,payload,updated,fetched])?;
            }
            _ => {
                let parent = field(if table == "collections" || table == "timeline_events" {
                    "projectId"
                } else {
                    "collectionId"
                })?;
                self.connection.execute(&format!("INSERT INTO {table} (id,parent_id,payload,updated_at,fetched_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,payload=excluded.payload,updated_at=excluded.updated_at,fetched_at=excluded.fetched_at"), params![id,parent,payload,updated,fetched])?;
            }
        }
        Ok(())
    }
    fn get<T: DeserializeOwned>(&self, table: &str, id: &str) -> Result<Option<T>> {
        let payload: Option<String> = self
            .connection
            .query_row(
                &format!("SELECT payload FROM {table} WHERE id=?"),
                [id],
                |row| row.get(0),
            )
            .optional()?;
        payload
            .map(|text| serde_json::from_str(&text).map_err(Error::from))
            .transpose()
    }
    fn list<T: DeserializeOwned>(&self, table: &str, parent: Option<&str>) -> Result<Vec<T>> {
        let order = if table == "pages" {
            "idx, id"
        } else if table == "entries" {
            "json_extract(payload, '$.index'), id"
        } else if table == "timeline_events" {
            "json_extract(payload, '$.createdAt') DESC, id DESC"
        } else {
            "id"
        };
        let clause = if parent.is_some() {
            " WHERE parent_id=?"
        } else {
            ""
        };
        let mut statement = self.connection.prepare(&format!(
            "SELECT payload FROM {table}{clause} ORDER BY {order}"
        ))?;
        let rows = statement.query_map(rusqlite::params_from_iter(parent), |row| {
            row.get::<_, String>(0)
        })?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn page_status_counts(&self, entry_id: &str) -> Result<BTreeMap<String, u64>> {
        let mut statement = self.connection.prepare(
            "SELECT status,COUNT(*) FROM pages WHERE entry_id=? GROUP BY status ORDER BY status",
        )?;
        Ok(statement
            .query_map([entry_id], |row| {
                Ok((row.get(0)?, row.get::<_, i64>(1)? as u64))
            })?
            .collect::<std::result::Result<_, _>>()?)
    }
    pub fn replace_pages<T: Serialize>(&self, entry_id: &str, pages: &[T]) -> Result<()> {
        self.connection
            .execute("DELETE FROM pages WHERE entry_id=?", [entry_id])?;
        for page in pages {
            self.put_page(page)?;
        }
        Ok(())
    }
    pub fn fresh_response<T: DeserializeOwned>(
        &self,
        key: &str,
        max_age: Duration,
    ) -> Result<Option<T>> {
        let (table, column, id) = if let Some(id) = key.strip_prefix("collection-progress/") {
            ("collection_progress", "id", id)
        } else if let Some(id) = key.strip_prefix("entry-progress/") {
            ("entry_progress", "id", id)
        } else {
            ("responses", "key", key)
        };
        let row: Option<(String, String)> = self
            .connection
            .query_row(
                &format!("SELECT payload,fetched_at FROM {table} WHERE {column}=?"),
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((payload, fetched_at)) = row {
            let fetched = OffsetDateTime::parse(&fetched_at, &Rfc3339)
                .map_err(|error| Error::Invalid(error.to_string()))?;
            let age = OffsetDateTime::now_utc() - fetched;
            if !age.is_negative() && age.unsigned_abs() < max_age {
                return Ok(Some(serde_json::from_str(&payload)?));
            }
        }
        Ok(None)
    }
    pub fn put_response<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        if key.starts_with("collection-progress/") {
            return self.put_collection_progress(value);
        }
        if key.starts_with("entry-progress/") {
            return self.put_entry_progress(value);
        }
        self.connection.execute("INSERT INTO responses(key,payload,fetched_at) VALUES (?1,?2,?3) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload,fetched_at=excluded.fetched_at", params![key,serde_json::to_string(value)?,now()?])?;
        Ok(())
    }
}
fn now() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

macro_rules! record_api {
    ($put:ident, $get:ident, $list:ident, $table:literal) => {
        impl Storage {
            pub fn $put<T: Serialize>(&self, value: &T) -> Result<()> {
                self.put($table, value)
            }
            pub fn $get<T: DeserializeOwned>(&self, id: &str) -> Result<Option<T>> {
                self.get($table, id)
            }
            pub fn $list<T: DeserializeOwned>(&self, parent_id: &str) -> Result<Vec<T>> {
                self.list($table, Some(parent_id))
            }
        }
    };
}
record_api!(
    put_collection,
    get_collection,
    list_collections,
    "collections"
);
record_api!(put_entry, get_entry, list_entries, "entries");
record_api!(put_page, get_page, list_pages, "pages");
record_api!(
    put_timeline_event,
    get_timeline_event,
    list_timeline_events,
    "timeline_events"
);
impl Storage {
    pub fn put_user<T: Serialize>(&self, value: &T) -> Result<()> {
        self.put("users", value)
    }
    pub fn get_user<T: DeserializeOwned>(&self, uid: &str) -> Result<Option<T>> {
        self.get("users", uid)
    }
    pub fn list_users<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.list("users", None)
    }
    pub fn put_project<T: Serialize>(&self, value: &T) -> Result<()> {
        self.put("projects", value)
    }
    pub fn get_project<T: DeserializeOwned>(&self, id: &str) -> Result<Option<T>> {
        self.get("projects", id)
    }
    pub fn list_projects<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.list("projects", None)
    }
}

impl Storage {
    fn put_progress<T: Serialize>(&self, table: &str, id_field: &str, record: &T) -> Result<()> {
        let value = serde_json::to_value(record)?;
        let id = value[id_field]
            .as_str()
            .ok_or_else(|| Error::Invalid(format!("missing {id_field}")))?;
        let fetched = value["fetchedAt"]
            .as_str()
            .ok_or_else(|| Error::Invalid("missing fetchedAt".into()))?;
        self.connection.execute(&format!("INSERT INTO {table}(id,payload,fetched_at) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,fetched_at=excluded.fetched_at"), params![id,serde_json::to_string(&value)?,fetched])?;
        Ok(())
    }
    pub fn put_collection_progress<T: Serialize>(&self, value: &T) -> Result<()> {
        self.put_progress("collection_progress", "collectionId", value)
    }
    pub fn put_entry_progress<T: Serialize>(&self, value: &T) -> Result<()> {
        self.put_progress("entry_progress", "entryId", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn progress_snapshots_expire_at_ten_minutes_and_reject_future_dates() -> Result<()> {
        let db = Storage::in_memory()?;
        for (seconds, fresh) in [(599, true), (600, false), (-60, false)] {
            let fetched =
                (OffsetDateTime::now_utc() - time::Duration::seconds(seconds)).format(&Rfc3339)?;
            let c = json!({"collectionId":"c","entries":1,"size":10,"completed":3,"initiated":2,"editing":1,"fetchedAt":fetched});
            let e = json!({"entryId":"e","size":10,"completed":3,"initiated":2,"editing":1,"fetchedAt":fetched});
            db.put_collection_progress(&c)?;
            db.put_entry_progress(&e)?;
            for key in ["collection-progress/c", "entry-progress/e"] {
                let result = db.fresh_response::<Value>(key, Duration::from_secs(600))?;
                assert_eq!(result.is_some(), fresh);
                if let Some(value) = result {
                    assert_eq!(value["editing"], 1);
                }
            }
        }
        Ok(())
    }
    #[test]
    fn home_migration_upgrades_existing_cache_and_preserves_payloads() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch("CREATE TABLE schema_version (version INTEGER PRIMARY KEY); INSERT INTO schema_version VALUES(1);")?;
        connection.execute_batch(include_str!("../migrations/001_cache.sql"))?;
        connection.execute(
            "INSERT INTO projects(id,payload,fetched_at) VALUES('p','{\"id\":\"p\"}',?)",
            [now()?],
        )?;
        let db = Storage::initialize(connection)?;
        assert!(db.get_project::<Value>("p")?.is_some());
        db.put_user(&json!({"uid":"u","displayName":"名","future":1}))?;
        db.put_timeline_event(&json!({"id":"a","projectId":"p","createdAt":"2026-09-09T00:00:00Z","data":{"text":"文"}}))?;
        db.put_timeline_event(
            &json!({"id":"b","projectId":"p","createdAt":"2026-09-10T00:00:00Z"}),
        )?;
        assert_eq!(
            db.get_user::<Value>("u")?.map(|u| u["future"].clone()),
            Some(json!(1))
        );
        assert_eq!(db.list_users::<Value>()?.len(), 1);
        assert_eq!(db.list_timeline_events::<Value>("p")?[0]["id"], "b");
        assert!(db.list_timeline_events::<Value>("other")?.is_empty());
        assert_eq!(
            db.get_timeline_event::<Value>("a")?
                .map(|e| e["data"]["text"].clone()),
            Some(json!("文"))
        );
        let fetched: String =
            db.connection
                .query_row("SELECT fetched_at FROM users WHERE id='u'", [], |r| {
                    r.get(0)
                })?;
        assert!(OffsetDateTime::parse(&fetched, &Rfc3339).is_ok());
        let db = Storage::initialize(db.connection)?;
        assert_eq!(db.list_users::<Value>()?.len(), 1);
        Ok(())
    }
    #[test]
    fn migrations_crud_counts_and_freshness() -> Result<()> {
        let mut db = Storage::in_memory()?;
        db.put_project(&json!({"id":"p","future":{"x":1}}))?;
        db.put_collection(&json!({"id":"c","projectId":"p"}))?;
        db.put_entry(&json!({"id":"e","collectionId":"c","index":0}))?;
        assert_eq!(db.list_projects::<Value>()?.len(), 1);
        assert_eq!(
            db.get_project::<Value>("p")?
                .map(|p| p["future"]["x"].clone()),
            Some(json!(1))
        );
        assert_eq!(db.list_collections::<Value>("p")?.len(), 1);
        assert_eq!(db.list_entries::<Value>("c")?.len(), 1);
        assert!(db.get_collection::<Value>("missing")?.is_none());
        assert!(db.get_entry::<Value>("e")?.is_some());
        for (index, status) in [(2, "completed"), (0, "default"), (1, "new-status")] {
            db.put_page(
                &json!({"id":format!("e_{index}"),"entryId":"e","index":index,"status":status}),
            )?;
        }
        db.put_page(&json!({"id":"e_0","entryId":"e","index":0,"status":"completed"}))?;
        assert_eq!(db.page_status_counts("e")?.get("completed"), Some(&2));
        assert_eq!(db.list_pages::<Value>("e")?[0]["index"], 0);
        assert_eq!(
            db.get_page::<Value>("e_1")?.map(|p| p["status"].clone()),
            Some(json!("new-status"))
        );
        db.put_response("empty", &Vec::<Value>::new())?;
        assert_eq!(
            db.fresh_response::<Vec<Value>>("empty", Duration::from_secs(600))?,
            Some(vec![])
        );
        assert!(
            db.fresh_response::<Value>("empty", Duration::ZERO)?
                .is_none()
        );
        let failed: Result<()> = db.transaction(|db| {
            db.replace_pages::<Value>("e", &[])?;
            Err(Error::Invalid("rollback".into()))
        });
        assert!(failed.is_err());
        assert_eq!(db.list_pages::<Value>("e")?.len(), 3);
        let versions: i64 =
            db.connection
                .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))?;
        assert_eq!(versions, 4);
        Ok(())
    }
    #[test]
    fn ocr_results_keep_model_versions_and_replace_a_rerun() -> Result<()> {
        let db = Storage::in_memory()?;
        db.put_ocr(
            "page_3",
            "v17",
            &serde_json::json!({"model":"v17","text":"old"}),
        )?;
        db.put_ocr(
            "page_3",
            "v18",
            &serde_json::json!({"model":"v18","text":"new"}),
        )?;
        db.put_ocr(
            "page_3",
            "v18",
            &serde_json::json!({"model":"v18","text":"rerun"}),
        )?;
        let result: serde_json::Value = db
            .ocr_result("page_3")?
            .ok_or_else(|| Error::Invalid("missing OCR result".into()))?;
        assert_eq!(result["text"], "rerun");
        assert_eq!(
            db.connection
                .query_row("SELECT COUNT(*) FROM ocr_results", [], |row| row
                    .get::<_, i64>(0))?,
            2
        );
        assert!(db.ocr_result::<serde_json::Value>("page_4")?.is_none());
        Ok(())
    }
}
