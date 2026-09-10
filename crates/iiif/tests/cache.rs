use honkoku_iiif::{Bytes, Error, ImageCache};
use sha2::{Digest, Sha256};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn put_get_lru_and_reopen() -> Result {
    let dir = tempfile::tempdir()?;
    let cache = ImageCache::open(dir.path(), 6)?;
    let url = "https://images.example/a";
    let cached = cache.put(
        url,
        "image/jpeg",
        Bytes::from_static(b"abc"),
        Some("etag"),
        Some("date"),
    )?;
    let key = format!("{:x}", Sha256::digest(url.as_bytes()));
    assert_eq!(cached.path, dir.path().join(&key[..2]).join(&key[2..]));
    assert_eq!(std::fs::read(&cached.path)?, b"abc");
    cache.put("b", "image/png", Bytes::from_static(b"def"), None, None)?;
    let entry = cache.get(url)?.ok_or("missing cache entry")?;
    assert!(entry.is_fresh());
    assert_eq!(entry.etag.as_deref(), Some("etag"));
    assert_eq!(entry.last_modified.as_deref(), Some("date"));
    cache.put("c", "image/png", Bytes::from_static(b"ghi"), None, None)?;
    assert!(cache.get("b")?.is_none());
    assert!(cache.get(url)?.is_some());
    drop(cache);
    let reopened = ImageCache::open(dir.path(), 3)?;
    assert!(reopened.get(url)?.is_some());
    assert!(reopened.get("c")?.is_none());
    let db = rusqlite::Connection::open(dir.path().join("index.sqlite"))?;
    let total: i64 = db.query_row("SELECT SUM(size) FROM images", [], |row| row.get(0))?;
    assert_eq!(total, 3);
    Ok(())
}

#[test]
fn replacement_missing_file_and_oversize() -> Result {
    let dir = tempfile::tempdir()?;
    let cache = ImageCache::open(dir.path(), 4)?;
    let first = cache.put("a", "image/jpeg", Bytes::from_static(b"old"), None, None)?;
    let new = cache.put(
        "a",
        "image/png",
        Bytes::from_static(b"new!"),
        Some("v2"),
        None,
    )?;
    assert_eq!(new.path, first.path);
    let entry = cache.get("a")?.ok_or("missing cache entry")?;
    assert_eq!(entry.cached.bytes.as_deref(), Some(&b"new!"[..]));
    assert_eq!(first.bytes.as_deref(), Some(&b"old"[..]));
    assert_eq!(entry.cached.content_type, "image/png");
    assert!(matches!(
        cache.put("b", "image/png", Bytes::from_static(b"large"), None, None),
        Err(Error::TooLarge)
    ));
    std::fs::remove_file(new.path)?;
    assert!(cache.get("a")?.is_none());
    Ok(())
}
