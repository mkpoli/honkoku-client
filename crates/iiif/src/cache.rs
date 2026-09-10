use crate::{Bytes, Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(crate) const FRESH_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Clone, Debug)]
pub struct Cached {
    pub path: PathBuf,
    pub content_type: String,
    /// A snapshot retained while the response is in use, even if LRU eviction runs.
    pub bytes: Option<Bytes>,
}

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub cached: Cached,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_at: i64,
}

impl CacheEntry {
    pub fn is_fresh(&self) -> bool {
        let age = now().saturating_sub(self.fetched_at);
        age >= 0 && age < FRESH_FOR.as_millis() as i64
    }
}

#[derive(Clone)]
pub struct ImageCache {
    dir: Arc<PathBuf>,
    db: Arc<Mutex<Connection>>,
    max_bytes: u64,
}

impl ImageCache {
    /// Synchronous disk operations; async callers must use a blocking worker.
    pub fn open(dir: impl AsRef<Path>, max_bytes: u64) -> Result<Self> {
        fs::create_dir_all(dir.as_ref())?;
        let db = Connection::open(dir.as_ref().join("index.sqlite"))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS images (
                url TEXT PRIMARY KEY, key TEXT NOT NULL UNIQUE,
                content_type TEXT NOT NULL, size INTEGER NOT NULL,
                etag TEXT, last_modified TEXT,
                fetched_at INTEGER NOT NULL, last_used_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS images_lru ON images(last_used_at);",
        )?;
        let cache = Self {
            dir: Arc::new(dir.as_ref().to_path_buf()),
            db: Arc::new(Mutex::new(db)),
            max_bytes,
        };
        cache.evict(&*cache.lock()?)?;
        Ok(cache)
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| Error::Worker("cache lock poisoned".into()))
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(&key[..2]).join(&key[2..])
    }

    pub fn get(&self, url: &str) -> Result<Option<CacheEntry>> {
        let db = self.lock()?;
        let entry = db
            .query_row(
                "SELECT key,content_type,etag,last_modified,fetched_at FROM images WHERE url=?",
                [url],
                |row| {
                    let key: String = row.get(0)?;
                    Ok(CacheEntry {
                        cached: Cached {
                            path: self.path(&key),
                            content_type: row.get(1)?,
                            bytes: None,
                        },
                        etag: row.get(2)?,
                        last_modified: row.get(3)?,
                        fetched_at: row.get(4)?,
                    })
                },
            )
            .optional()?;
        let Some(mut entry) = entry else {
            return Ok(None);
        };
        // Keep the snapshot alive across concurrent eviction or replacement.
        match fs::read(&entry.cached.path) {
            Ok(bytes) => entry.cached.bytes = Some(Bytes::from(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                db.execute("DELETE FROM images WHERE url=?", [url])?;
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        }
        db.execute("UPDATE images SET last_used_at=MAX(?1,(SELECT COALESCE(MAX(last_used_at),0)+1 FROM images)) WHERE url=?2", params![now(), url])?;
        Ok(Some(entry))
    }

    pub fn put(
        &self,
        url: &str,
        content_type: &str,
        bytes: Bytes,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<Cached> {
        if bytes.len() as u64 > self.max_bytes {
            return Err(Error::TooLarge);
        }
        let db = self.lock()?;
        let key = format!("{:x}", Sha256::digest(url.as_bytes()));
        let path = self.path(&key);
        let parent = path
            .parent()
            .ok_or_else(|| Error::Invalid("cache path has no parent".into()))?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&path)
            .map_err(|error| Error::Io(error.error))?;
        db.execute(
            "INSERT INTO images(url,key,content_type,size,etag,last_modified,fetched_at,last_used_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,MAX(?7,(SELECT COALESCE(MAX(last_used_at),0)+1 FROM images)))
             ON CONFLICT(url) DO UPDATE SET content_type=excluded.content_type,size=excluded.size,
             etag=excluded.etag,last_modified=excluded.last_modified,
             fetched_at=excluded.fetched_at,last_used_at=excluded.last_used_at",
            params![url,key,content_type,bytes.len() as i64,etag,last_modified,now()],
        )?;
        self.evict(&db)?;
        Ok(Cached {
            path,
            content_type: content_type.into(),
            bytes: Some(bytes),
        })
    }

    pub(crate) fn revalidated(
        &self,
        url: &str,
        etag: Option<String>,
        modified: Option<String>,
    ) -> Result<()> {
        self.lock()?.execute(
            "UPDATE images SET fetched_at=?1,last_used_at=?1,
             etag=COALESCE(?2,etag),last_modified=COALESCE(?3,last_modified) WHERE url=?4",
            params![now(), etag, modified, url],
        )?;
        Ok(())
    }

    fn evict(&self, db: &Connection) -> Result<()> {
        let mut total: i64 =
            db.query_row("SELECT COALESCE(SUM(size),0) FROM images", [], |row| {
                row.get(0)
            })?;
        while total as u64 > self.max_bytes {
            let (url, key, size): (String, String, i64) = db.query_row(
                "SELECT url,key,size FROM images ORDER BY last_used_at,rowid LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            match fs::remove_file(self.path(&key)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            db.execute("DELETE FROM images WHERE url=?", [url])?;
            total = total.saturating_sub(size);
        }
        Ok(())
    }
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
