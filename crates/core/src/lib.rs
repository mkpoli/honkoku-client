pub mod api;
pub mod auth;
pub mod cache;
pub mod clips;
pub mod editing;
pub mod firestore;
pub mod glyphs;
pub mod history;
pub mod home;
pub mod model;
pub mod progress;

use reqwest::{Client, Method, Url};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

pub const API_BASE: &str = "https://app.honkoku.org/api";
pub const FIRESTORE_BASE: &str =
    "https://firestore.googleapis.com/v1/projects/honkoku3-c466c/databases/(default)/documents";
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cache: {0}")]
    Storage(#[from] honkoku_storage::Error),
    #[error("invalid response: {0}")]
    Invalid(String),
    #[error("request timed out after 30 seconds")]
    Timeout,
    #[error("worker: {0}")]
    Worker(String),
    #[error("session I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("credential store: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("signed out; import a new session to sign in")]
    SignedOut,
    #[error("document changed concurrently: {path}")]
    Conflict {
        path: String,
        current: Option<firestore::ReadDocument>,
    },
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone)]
pub struct HonkokuClient {
    http: Client,
    hosts: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    api_base: String,
    firestore_base: String,
    session: Option<Arc<auth::TokenManager>>,
    search_base: String,
    functions_base: String,
    home_storage: cache::SharedStorage,
    summary_gates: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}
impl HonkokuClient {
    pub fn new() -> Result<Self> {
        Self::with_endpoints(API_BASE, FIRESTORE_BASE)
    }
    /// Endpoint override for local contract tests.
    pub fn with_endpoints(api: &str, firestore: &str) -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .user_agent("honkoku-client/0.1 (+https://github.com/mkpoli/honkoku-client)")
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            hosts: Arc::new(Mutex::new(HashMap::new())),
            api_base: api.trim_end_matches('/').into(),
            firestore_base: firestore.trim_end_matches('/').into(),
            session: None,
            summary_gates: Arc::new(Mutex::new(HashMap::new())),
            search_base: "https://search-r7au5bknyq-uc.a.run.app".into(),
            functions_base: "https://us-central1-honkoku3-c466c.cloudfunctions.net".into(),
            home_storage: Arc::new(Mutex::new(honkoku_storage::Storage::in_memory()?)),
        })
    }
    pub fn with_session(mut self, session: auth::TokenManager) -> Self {
        self.session = Some(Arc::new(session));
        self
    }
    /// Share the application's persistent cache with home-screen document enrichment.
    pub fn with_storage(mut self, storage: cache::SharedStorage) -> Self {
        self.home_storage = storage;
        self
    }
    pub async fn signed_in_uid(&self) -> Result<String> {
        self.session.as_ref().ok_or(Error::SignedOut)?.uid().await
    }
    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        body: Option<&impl Serialize>,
        token: Option<&str>,
    ) -> Result<T> {
        Ok(self
            .request_response(method, url, body, token)
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    pub(crate) async fn request_response(
        &self,
        method: Method,
        url: Url,
        body: Option<&impl Serialize>,
        token: Option<&str>,
    ) -> Result<reqwest::Response> {
        let host = url
            .host_str()
            .ok_or_else(|| Error::Invalid("URL has no host".into()))?
            .to_owned();
        let semaphore = self
            .hosts
            .lock()
            .map_err(|e| Error::Worker(e.to_string()))?
            .entry(host)
            .or_insert_with(|| Arc::new(Semaphore::new(4)))
            .clone();
        tokio::time::timeout(Duration::from_secs(30), async {
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|e| Error::Worker(e.to_string()))?;
            let mut request = self.http.request(method, url);
            if let Some(body) = body {
                request = request.json(body);
            }
            let managed_token = match (&self.session, token) {
                (Some(session), None) => Some(session.id_token().await?),
                _ => None,
            };
            if let Some(token) = token.or(managed_token.as_deref()) {
                request = request.bearer_auth(token);
            }
            Ok(request.send().await?)
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
}
pub(crate) fn endpoint(base: &str, segments: &[&str]) -> Result<Url> {
    let mut url = Url::parse(base).map_err(|e| Error::Invalid(e.to_string()))?;
    if !segments.is_empty() {
        url.path_segments_mut()
            .map_err(|()| Error::Invalid("invalid base URL".into()))?
            .pop_if_empty()
            .extend(segments);
    }
    Ok(url)
}

#[cfg(test)]
mod tests;
