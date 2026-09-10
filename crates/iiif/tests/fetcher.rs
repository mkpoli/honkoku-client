use honkoku_iiif::{Bytes, Fetcher, ImageCache};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn test_fetcher(dir: &std::path::Path) -> Result<Fetcher> {
    Ok(Fetcher::new(
        ImageCache::open(dir, 1024 * 1024)?,
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
    ))
}

fn age(dir: &std::path::Path) -> Result {
    let db = rusqlite::Connection::open(dir.join("index.sqlite"))?;
    db.execute("UPDATE images SET fetched_at=0", [])?;
    Ok(())
}

#[tokio::test]
async fn deduplicates_and_serves_fresh_cache_without_network() -> Result {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/image"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"image")
                .insert_header("Content-Type", "image/jpeg")
                .set_delay(Duration::from_millis(100)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    let url = format!("{}/image", server.uri());
    let (a, b) = tokio::join!(fetcher.get(&url), fetcher.get(&url));
    assert_eq!(a?.read().await?, b?.read().await?);
    assert_eq!(fetcher.get(&url).await?.content_type, "image/jpeg");
    assert_eq!(fetcher.request_count(), 1);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn rejects_unregistered_hosts_even_when_cached() -> Result {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir()?;
    let url = format!("{}/image", server.uri());
    ImageCache::open(dir.path(), 1024)?.put(
        &url,
        "image/jpeg",
        Bytes::from_static(b"cached"),
        None,
        None,
    )?;
    let fetcher = test_fetcher(dir.path())?;
    assert!(fetcher.get(&url).await.is_err());
    for url in [
        "file:///etc/passwd",
        "https://user:pass@images.example/a",
        "ftp://images.example/a",
    ] {
        assert!(fetcher.get(url).await.is_err());
    }
    assert_eq!(fetcher.request_count(), 0);
    assert!(
        server
            .received_requests()
            .await
            .ok_or("request recording disabled")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn limits_each_host_to_four_downloads() -> Result {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .set_delay(Duration::from_millis(500)),
        )
        .expect(8)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..8 {
        let fetcher = fetcher.clone();
        let url = format!("{}/{index}", server.uri());
        tasks.spawn(async move { fetcher.get(&url).await });
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while fetcher.request_count() < 4 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(fetcher.request_count(), 4);
    while let Some(task) = tasks.join_next().await {
        task??;
    }
    assert_eq!(fetcher.request_count(), 8);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn cancelling_one_consumer_preserves_the_shared_download() -> Result {
    let server = MockServer::start().await;
    Mock::given(path("/image"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .set_delay(Duration::from_millis(200)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    let url = format!("{}/image", server.uri());
    let worker = {
        let fetcher = fetcher.clone();
        let url = url.clone();
        tokio::spawn(async move { fetcher.get(&url).await })
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        while fetcher.request_count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    worker.abort();
    assert_eq!(fetcher.get(&url).await?.read().await?, b"ok"[..]);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn conditional_revalidation_updates_freshness_and_preserves_body() -> Result {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir()?;
    let url = format!("{}/image", server.uri());
    ImageCache::open(dir.path(), 1024)?.put(
        &url,
        "image/jpeg",
        Bytes::from_static(b"cached"),
        Some("\"v1\""),
        Some("Wed, 21 Oct 2015 07:28:00 GMT"),
    )?;
    age(dir.path())?;
    Mock::given(method("GET"))
        .and(path("/image"))
        .and(header("if-none-match", "\"v1\""))
        .and(|request: &wiremock::Request| {
            request
                .headers
                .get("if-modified-since")
                .and_then(|value| value.to_str().ok())
                == Some("Wed, 21 Oct 2015 07:28:00 GMT")
        })
        .respond_with(ResponseTemplate::new(304).insert_header("etag", "\"v2\""))
        .expect(1)
        .mount(&server)
        .await;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    assert_eq!(fetcher.get(&url).await?.read().await?, b"cached"[..]);
    fetcher.get(&url).await?;
    assert_eq!(fetcher.request_count(), 1);
    let entry = ImageCache::open(dir.path(), 1024)?
        .get(&url)?
        .ok_or("missing entry")?;
    assert_eq!(entry.etag.as_deref(), Some("\"v2\""));
    assert!(entry.is_fresh());
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn redirects_validate_each_host_and_scheme() -> Result {
    let server = MockServer::start().await;
    Mock::given(path("/allowed"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/target"))
        .mount(&server)
        .await;
    Mock::given(path("/target"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/blocked"))
        .respond_with(ResponseTemplate::new(302).insert_header(
            "location",
            format!("{}/secret", server.uri().replace("127.0.0.1", "localhost")),
        ))
        .mount(&server)
        .await;
    Mock::given(path("/secret"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(path("/file"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "file:///etc/passwd"))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    assert_eq!(
        fetcher
            .get(&format!("{}/allowed", server.uri()))
            .await?
            .read()
            .await?,
        b"ok"[..]
    );
    assert!(
        fetcher
            .get(&format!("{}/blocked", server.uri()))
            .await
            .is_err()
    );
    assert!(
        fetcher
            .get(&format!("{}/file", server.uri()))
            .await
            .is_err()
    );
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn retry_after_delays_retry_and_failures_are_shared() -> Result {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let count = hits.clone();
    Mock::given(path("/retry"))
        .respond_with(move |_: &wiremock::Request| {
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(429).insert_header("Retry-After", "1")
            } else {
                ResponseTemplate::new(200).set_body_string("ok")
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/missing"))
        .respond_with(ResponseTemplate::new(404).set_delay(Duration::from_millis(100)))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    let start = tokio::time::Instant::now();
    fetcher.get(&format!("{}/retry", server.uri())).await?;
    assert!(start.elapsed() >= Duration::from_secs(1));
    let url = format!("{}/missing", server.uri());
    let (a, b) = tokio::join!(fetcher.get(&url), fetcher.get(&url));
    assert!(a.is_err());
    assert!(b.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn manifests_and_image_builders_use_the_cache() -> Result {
    let server = MockServer::start().await;
    Mock::given(path("/manifest"))
        .respond_with(ResponseTemplate::new(200).set_body_string(include_str!("v3.json")))
        .expect(1)
        .mount(&server)
        .await;
    for endpoint in [
        "/service/info.json",
        "/service/full/160,/0/default.jpg",
        "/service/full/max/0/default.jpg",
    ] {
        Mock::given(path(endpoint))
            .respond_with(ResponseTemplate::new(200).set_body_string("image"))
            .expect(1)
            .mount(&server)
            .await;
    }
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("127.0.0.1")?;
    let manifest = fetcher
        .get_manifest(&format!("{}/manifest", server.uri()))
        .await?;
    assert_eq!(manifest.canvases.len(), 2);
    let service = honkoku_iiif::ImageService {
        id: format!("{}/service", server.uri()),
        version: honkoku_iiif::ImageVersion::V3,
        profile: serde_json::Value::Null,
    };
    fetcher.info(&service).await?;
    fetcher.thumbnail(&service, 160).await?;
    fetcher.full(&service, Some(160)).await?;
    fetcher.full(&service, None).await?;
    server.verify().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires network access to Ryukoku University's IIIF service"]
async fn real_ryukoku_info_is_cached() -> Result {
    let dir = tempfile::tempdir()?;
    let fetcher = test_fetcher(dir.path())?;
    fetcher.allow_host("da2.library.ryukoku.ac.jp")?;
    let url =
        "https://da2.library.ryukoku.ac.jp/image/?IIIF=/17%2F170161%2F170161-0001.tif/info.json";
    let first = fetcher.get(url).await?;
    let json: serde_json::Value = serde_json::from_slice(&first.read().await?)?;
    assert!(json["width"].as_u64().is_some_and(|width| width > 0));
    let requests = fetcher.request_count();
    let second = fetcher.get(url).await?;
    assert_eq!(first.read().await?, second.read().await?);
    assert_eq!(fetcher.request_count(), requests);
    println!(
        "Ryukoku info.json: first fetch {requests} upstream request(s), second fetch 0 upstream requests"
    );
    Ok(())
}
