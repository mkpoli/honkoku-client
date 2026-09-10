use crate::{Bytes, CacheEntry, Cached, Error, ImageCache, ImageService, Manifest, Result};
use reqwest::{Client, StatusCode, header};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};
use tokio::sync::{Semaphore, watch};
use url::Url;

type SharedResult = std::result::Result<Cached, Arc<Error>>;
type Flight = watch::Receiver<Option<SharedResult>>;

struct Inner {
    cache: ImageCache,
    client: Client,
    allowed: Mutex<HashSet<String>>,
    hosts: Mutex<HashMap<String, Arc<Semaphore>>>,
    flights: Mutex<HashMap<String, Flight>>,
    requests: AtomicU64,
}

#[derive(Clone)]
pub struct Fetcher(Arc<Inner>);

impl Fetcher {
    /// Construct a fetcher using a client with automatic redirects disabled.
    ///
    /// Build `client` with `Client::builder().redirect(reqwest::redirect::Policy::none())`.
    /// Reqwest cannot inspect or override an already-built client's redirect policy;
    /// this precondition is required for the per-hop host allowlist and concurrency limit.
    pub fn new(cache: ImageCache, client: Client) -> Self {
        Self(Arc::new(Inner {
            cache,
            client,
            allowed: Mutex::new(HashSet::new()),
            hosts: Mutex::new(HashMap::new()),
            flights: Mutex::new(HashMap::new()),
            requests: AtomicU64::new(0),
        }))
    }

    pub fn allow_host(&self, host: &str) -> Result<()> {
        let host = url::Host::parse(host)
            .map_err(|_| Error::Invalid("invalid host".into()))?
            .to_string();
        self.0.allowed.lock().map_err(poisoned)?.insert(host);
        Ok(())
    }

    /// Validate an HTTP(S) URL before registering it or returning it to a viewer.
    pub fn allow_url(&self, url: &str) -> Result<()> {
        let url = parse_url(url)?;
        self.allow_host(
            url.host_str()
                .ok_or_else(|| Error::Invalid("missing host".into()))?,
        )
    }

    fn checked_url(&self, url: &str) -> Result<Url> {
        let url = parse_url(url)?;
        let host = url
            .host_str()
            .ok_or_else(|| Error::Invalid("missing host".into()))?;
        if !self.0.allowed.lock().map_err(poisoned)?.contains(host) {
            return Err(Error::Forbidden(host.into()));
        }
        Ok(url)
    }

    /// Number of actual upstream attempts, including redirects and retries.
    pub fn request_count(&self) -> u64 {
        self.0.requests.load(Ordering::Relaxed)
    }

    pub async fn get(&self, url: &str) -> Result<Cached> {
        self.checked_url(url)?;
        let mut receiver = {
            let mut flights = self.0.flights.lock().map_err(poisoned)?;
            if let Some(receiver) = flights.get(url) {
                receiver.clone()
            } else {
                let (sender, receiver) = watch::channel(None);
                flights.insert(url.into(), receiver.clone());
                let this = self.clone();
                let url = url.to_owned();
                // A cancelled consumer must not cancel the shared download or strand waiters.
                tokio::spawn(async move {
                    let result =
                        match tokio::time::timeout(Duration::from_secs(30), this.fetch(&url)).await
                        {
                            Ok(result) => result,
                            Err(_) => Err(Error::Timeout),
                        }
                        .map_err(Arc::new);
                    sender.send_replace(Some(result));
                    if let Ok(mut flights) = this.0.flights.lock() {
                        flights.remove(&url);
                    }
                });
                receiver
            }
        };
        loop {
            if let Some(result) = receiver.borrow_and_update().clone() {
                return result.map_err(Error::Shared);
            }
            receiver
                .changed()
                .await
                .map_err(|_| Error::Worker("download worker stopped".into()))?;
        }
    }

    async fn fetch(&self, original: &str) -> Result<Cached> {
        let cache = self.0.cache.clone();
        let key = original.to_owned();
        let previous = blocking(move || cache.get(&key)).await?;
        if let Some(entry) = &previous
            && entry.is_fresh()
        {
            return Ok(entry.cached.clone());
        }
        let mut url = self.checked_url(original)?;
        let mut redirects = 0;
        let mut retries = 0;
        loop {
            let host = url
                .host_str()
                .ok_or_else(|| Error::Invalid("missing host".into()))?;
            let semaphore = self
                .0
                .hosts
                .lock()
                .map_err(poisoned)?
                .entry(host.into())
                .or_insert_with(|| Arc::new(Semaphore::new(4)))
                .clone();
            let permit = semaphore
                .acquire()
                .await
                .map_err(|_| Error::Worker("host queue closed".into()))?;
            let mut request = self
                .0
                .client
                .get(url.clone())
                .timeout(Duration::from_secs(30));
            if let Some(entry) = &previous {
                if let Some(etag) = &entry.etag {
                    request = request.header(header::IF_NONE_MATCH, etag);
                }
                if let Some(modified) = &entry.last_modified {
                    request = request.header(header::IF_MODIFIED_SINCE, modified);
                }
            }
            self.0.requests.fetch_add(1, Ordering::Relaxed);
            let mut response = request.send().await?;
            let status = response.status();
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
                if redirects >= 10 {
                    return Err(Error::Invalid("too many redirects".into()));
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| Error::Invalid("redirect has no valid location".into()))?;
                let next = url
                    .join(location)
                    .map_err(|_| Error::Invalid("invalid redirect URL".into()))?;
                url = self.checked_url(next.as_str())?;
                redirects += 1;
                continue;
            }
            if matches!(
                status,
                StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
            ) && retries < 3
            {
                let delay = retry_after(
                    response
                        .headers()
                        .get(header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok()),
                )
                .unwrap_or(Duration::from_secs(1 << retries));
                retries += 1;
                drop(response);
                drop(permit);
                tokio::time::sleep(delay).await;
                continue;
            }
            let text_header = |name| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
            };
            let etag = text_header(header::ETAG);
            let modified = text_header(header::LAST_MODIFIED);
            if status == StatusCode::NOT_MODIFIED {
                let CacheEntry { cached, .. } = previous
                    .ok_or_else(|| Error::Invalid("304 without a cached response".into()))?;
                let cache = self.0.cache.clone();
                let key = original.to_owned();
                blocking(move || cache.revalidated(&key, etag, modified)).await?;
                return Ok(cached);
            }
            if !status.is_success() {
                return Err(Error::Status(status.as_u16()));
            }
            let content_type = text_header(header::CONTENT_TYPE)
                .unwrap_or_else(|| "application/octet-stream".into());
            if response
                .content_length()
                .is_some_and(|size| size > self.0.cache.max_bytes())
            {
                return Err(Error::TooLarge);
            }
            let mut bytes = bytes::BytesMut::new();
            while let Some(chunk) = response.chunk().await? {
                if (bytes.len() as u64).saturating_add(chunk.len() as u64)
                    > self.0.cache.max_bytes()
                {
                    return Err(Error::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            drop(permit);
            let cache = self.0.cache.clone();
            let key = original.to_owned();
            return blocking(move || {
                cache.put(
                    &key,
                    &content_type,
                    bytes.freeze(),
                    etag.as_deref(),
                    modified.as_deref(),
                )
            })
            .await;
        }
    }

    pub async fn get_manifest(&self, url: &str) -> Result<Manifest> {
        let cached = self.get(url).await?;
        Manifest::from_value(&serde_json::from_slice(&cached.read().await?)?)
    }

    pub async fn info(&self, image_service: &ImageService) -> Result<Cached> {
        self.get(&format!(
            "{}/info.json",
            image_service.id.trim_end_matches('/')
        ))
        .await
    }

    pub async fn thumbnail(&self, image_service: &ImageService, width: u32) -> Result<Cached> {
        self.get(&image_service.thumbnail(width)).await
    }

    pub async fn full(
        &self,
        image_service: &ImageService,
        max_width: Option<u32>,
    ) -> Result<Cached> {
        self.get(&max_width.map_or_else(
            || image_service.full(),
            |width| image_service.thumbnail(width),
        ))
        .await
    }
}

impl Cached {
    pub async fn read(&self) -> Result<Bytes> {
        match &self.bytes {
            Some(bytes) => Ok(bytes.clone()),
            None => Ok(Bytes::from(tokio::fs::read(&self.path).await?)),
        }
    }
}

pub(crate) fn parse_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|_| Error::Invalid("invalid upstream URL".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid(
            "expected an HTTP(S) URL without credentials or fragment".into(),
        ));
    }
    Ok(url)
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> Error {
    Error::Worker("fetcher lock poisoned".into())
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| Error::Worker(error.to_string()))?
}

fn retry_after(value: Option<&str>) -> Option<Duration> {
    let value = value?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    Some(
        httpdate::parse_http_date(value)
            .ok()?
            .duration_since(SystemTime::now())
            .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_retry_after_seconds_and_http_dates() {
        assert_eq!(retry_after(Some("12")), Some(Duration::from_secs(12)));
        assert_eq!(
            retry_after(Some("Wed, 21 Oct 2015 07:28:00 GMT")),
            Some(Duration::ZERO)
        );
        assert_eq!(retry_after(Some("invalid")), None);
        let future = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(60));
        let delay = retry_after(Some(&future));
        assert!(delay.is_some_and(
            |delay| delay > Duration::from_secs(58) && delay <= Duration::from_secs(60)
        ));
    }
}
