//! Firebase sessions. Store operations are synchronous; token refresh schedules them off-runtime.
use crate::{Error, Result, model::Timestamp};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use time::OffsetDateTime;
use tokio::sync::Mutex;

const TOKEN_ENDPOINT: &str = "https://securetoken.googleapis.com/v1/token";
const SERVICE: &str = "li.mkpo.honkoku-client";

#[cfg(test)]
mod tests;

// Deliberately no Debug implementation: credentials must never appear in diagnostics.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub api_key: String,
    pub uid: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    #[serde(default)]
    pub providers: Vec<String>,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: Timestamp,
}

pub trait SessionStore: Send + Sync {
    fn load(&self) -> Result<Option<Session>>;
    fn save(&self, session: &Session) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

pub struct KeyringStore {
    entry: keyring::Entry,
    uid: String,
}
impl KeyringStore {
    /// One credential per uid. Probes the platform store so callers can choose a fallback.
    pub fn new(uid: &str) -> Result<Self> {
        let entry = keyring::Entry::new(SERVICE, uid)?;
        match entry.get_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self {
            entry,
            uid: uid.into(),
        })
    }
}
impl SessionStore for KeyringStore {
    fn load(&self) -> Result<Option<Session>> {
        match self.entry.get_password() {
            Ok(json) => {
                let session: Session = serde_json::from_str(&json)?;
                if session.uid != self.uid {
                    return Err(Error::Invalid("credential uid mismatch".into()));
                }
                Ok(Some(session))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn save(&self, session: &Session) -> Result<()> {
        if session.uid != self.uid {
            return Err(Error::Invalid("credential uid mismatch".into()));
        }
        Ok(self.entry.set_password(&serde_json::to_string(session)?)?)
    }
    fn clear(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

pub struct FileStore(pub PathBuf);
impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}
impl SessionStore for FileStore {
    fn load(&self) -> Result<Option<Session>> {
        match std::fs::read(&self.0) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn save(&self, session: &Session) -> Result<()> {
        let parent = self
            .0
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        // Same-directory atomic replacement preserves the old session if serialization or writing fails.
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        serde_json::to_writer(&mut file, session)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(&self.0)
            .map_err(|error| Error::Io(error.error))?;
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        match std::fs::remove_file(&self.0) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

pub fn import_dev_session(path: impl AsRef<Path>) -> Result<Session> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

struct State {
    session: Option<Session>,
    needs_save: bool,
}
pub struct TokenManager {
    state: Mutex<State>,
    store: Arc<dyn SessionStore>,
    http: reqwest::Client,
    endpoint: String,
}
impl TokenManager {
    pub fn new(session: Session, store: Arc<dyn SessionStore>) -> Result<Self> {
        Self::with_endpoint(session, store, TOKEN_ENDPOINT)
    }
    /// Endpoint override for refresh contract tests.
    pub fn with_endpoint(
        session: Session,
        store: Arc<dyn SessionStore>,
        endpoint: &str,
    ) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(State {
                session: Some(session),
                needs_save: false,
            }),
            store,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            endpoint: endpoint.into(),
        })
    }
    pub async fn uid(&self) -> Result<String> {
        // Validate/refresh before exposing identity for authenticated reads.
        self.id_token().await?;
        self.state
            .lock()
            .await
            .session
            .as_ref()
            .map(|s| s.uid.clone())
            .ok_or(Error::SignedOut)
    }
    pub async fn id_token(&self) -> Result<String> {
        // Keep the lock through refresh and persistence: concurrent callers share one refresh.
        let mut state = self.state.lock().await;
        let Some(session) = state.session.as_ref() else {
            self.persist(None).await?;
            return Err(Error::SignedOut);
        };
        if session.expires_at.0 - OffsetDateTime::now_utc() < time::Duration::minutes(5) {
            let mut url = reqwest::Url::parse(&self.endpoint)
                .map_err(|_| Error::Invalid("invalid token endpoint".into()))?;
            url.query_pairs_mut().append_pair("key", &session.api_key);
            let form = reqwest::Url::parse_with_params(
                "https://localhost/",
                [
                    ("grant_type", "refresh_token"),
                    ("refresh_token", session.refresh_token.as_str()),
                ],
            )
            .map_err(|_| Error::Invalid("invalid refresh form".into()))?;
            // Strip the URL from errors: its query contains the API key.
            let response = self
                .http
                .post(url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(form.query().unwrap_or_default().to_owned())
                .send()
                .await
                .map_err(|e| Error::Http(e.without_url()))?;
            let status = response.status();
            let value: serde_json::Value = response
                .json()
                .await
                .map_err(|e| Error::Http(e.without_url()))?;
            if !status.is_success() {
                let code = value["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .split(" : ")
                    .next()
                    .unwrap_or_default();
                if status == reqwest::StatusCode::BAD_REQUEST
                    && matches!(
                        code,
                        "TOKEN_EXPIRED"
                            | "USER_DISABLED"
                            | "USER_NOT_FOUND"
                            | "INVALID_REFRESH_TOKEN"
                    )
                {
                    state.session = None;
                    self.persist(None).await?;
                    return Err(Error::SignedOut);
                }
                return Err(Error::Invalid(format!("token refresh HTTP {status}")));
            }
            #[derive(Deserialize)]
            struct Refreshed {
                id_token: String,
                refresh_token: Option<String>,
                expires_in: String,
                user_id: String,
            }
            let refreshed: Refreshed = serde_json::from_value(value)?;
            let seconds: i64 = refreshed
                .expires_in
                .parse()
                .map_err(|_| Error::Invalid("invalid token lifetime".into()))?;
            if seconds <= 0 || refreshed.id_token.is_empty() || refreshed.user_id != session.uid {
                return Err(Error::Invalid(
                    "invalid refreshed identity or lifetime".into(),
                ));
            }
            let expires_at = OffsetDateTime::now_utc()
                .checked_add(time::Duration::seconds(seconds))
                .ok_or_else(|| Error::Invalid("token lifetime overflow".into()))?;
            let mut updated = session.clone();
            updated.id_token = refreshed.id_token;
            if let Some(token) = refreshed.refresh_token.filter(|t| !t.is_empty()) {
                updated.refresh_token = token;
            }
            updated.expires_at = Timestamp(expires_at);
            // Retain a rotated token in memory even if the store fails; retry persistence next call.
            state.session = Some(updated);
            state.needs_save = true;
        }
        if state.needs_save {
            self.persist(state.session.clone()).await?;
            state.needs_save = false;
        }
        state
            .session
            .as_ref()
            .map(|s| s.id_token.clone())
            .ok_or(Error::SignedOut)
    }
    async fn persist(&self, session: Option<Session>) -> Result<()> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || match session {
            Some(session) => store.save(&session),
            None => store.clear(),
        })
        .await
        .map_err(|e| Error::Worker(e.to_string()))?
    }
}

pub const FIREBASE_API_KEY: &str = "AIzaSyB-n5klhtxCtVmJqcsnhIc7-bWj5Ou--GY";
pub const FIREBASE_SDK_VERSION: &str = "10.14.1";
pub const SIGN_IN_ORIGIN: &str = "https://app.honkoku.org";

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum SignInProvider {
    #[serde(rename = "google.com")]
    Google,
    #[serde(rename = "twitter.com")]
    Twitter,
}
impl SignInProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::Google => "google.com",
            Self::Twitter => "twitter.com",
        }
    }
}

pub fn sign_in_url(provider: SignInProvider, event_id: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse("https://honkoku3-c466c.firebaseapp.com/__/auth/handler")
        .map_err(|_| Error::Invalid("invalid sign-in URL".into()))?;
    url.query_pairs_mut().extend_pairs([
        ("apiKey", FIREBASE_API_KEY),
        ("appName", "[DEFAULT]"),
        ("authType", "signInViaRedirect"),
        ("redirectUrl", "https://app.honkoku.org/"),
        ("v", FIREBASE_SDK_VERSION),
        ("providerId", provider.id()),
        ("eventId", event_id),
    ]);
    if matches!(provider, SignInProvider::Google) {
        url.query_pairs_mut().append_pair("scopes", "profile");
    }
    Ok(url)
}

// Deliberately excludes Debug, like Session.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturedSession {
    uid: String,
    email: Option<String>,
    display_name: Option<String>,
    providers: Vec<String>,
    refresh_token: String,
    id_token: String,
    expires_at: Timestamp,
}
impl CapturedSession {
    pub fn into_session(self, provider: SignInProvider) -> Result<Session> {
        if self.uid.is_empty()
            || self.refresh_token.is_empty()
            || self.id_token.is_empty()
            || self.expires_at.0 <= OffsetDateTime::now_utc()
            || !self.providers.iter().any(|id| id == provider.id())
        {
            return Err(Error::Invalid("invalid captured session".into()));
        }
        Ok(Session {
            api_key: FIREBASE_API_KEY.into(),
            uid: self.uid,
            email: self.email,
            display_name: self.display_name,
            providers: self.providers,
            refresh_token: self.refresh_token,
            id_token: self.id_token,
            expires_at: self.expires_at,
        })
    }
}

#[cfg(test)]
mod sign_in_tests {
    use super::*;

    #[test]
    fn redirect_parameters_match_the_site_sdk() {
        for provider in [SignInProvider::Google, SignInProvider::Twitter] {
            let url = sign_in_url(provider, "random-event").unwrap();
            assert_eq!(url.host_str(), Some("honkoku3-c466c.firebaseapp.com"));
            let params = url
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            assert_eq!(params["apiKey"], FIREBASE_API_KEY);
            assert_eq!(params["appName"], "[DEFAULT]");
            assert_eq!(params["authType"], "signInViaRedirect");
            assert_eq!(params["redirectUrl"], "https://app.honkoku.org/");
            assert_eq!(params["v"], "10.14.1");
            assert_eq!(params["providerId"], provider.id());
            assert_eq!(params["eventId"], "random-event");
            assert_eq!(
                params.get("scopes").map(|v| v.as_ref()),
                matches!(provider, SignInProvider::Google).then_some("profile")
            );
        }
        assert!(serde_json::from_str::<SignInProvider>("\"github.com\"").is_err());
    }

    #[test]
    fn captured_credentials_are_validated() {
        let valid = serde_json::json!({"uid":"user", "email":null, "displayName":null,
            "providers":["google.com"], "refreshToken":"refresh", "idToken":"id",
            "expiresAt":"2099-01-01T00:00:00Z"});
        let capture: CapturedSession = serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(
            capture
                .into_session(SignInProvider::Google)
                .unwrap()
                .api_key,
            FIREBASE_API_KEY
        );
        for (key, value) in [
            ("uid", serde_json::json!("")),
            ("refreshToken", serde_json::json!("")),
            ("idToken", serde_json::json!("")),
            ("expiresAt", serde_json::json!("2000-01-01T00:00:00Z")),
            ("providers", serde_json::json!(["twitter.com"])),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            let capture: CapturedSession = serde_json::from_value(invalid).unwrap();
            assert!(capture.into_session(SignInProvider::Google).is_err());
        }
    }
}
