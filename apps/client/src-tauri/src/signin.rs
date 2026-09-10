use crate::{
    AppError, AppState,
    commands::{self, EditingState, SessionInfo},
};
use honkoku_core::auth::{
    CapturedSession, FIREBASE_API_KEY, FIREBASE_AUTH_DOMAIN, FIREBASE_SDK_VERSION, SIGN_IN_ORIGIN,
    SessionStore, SignInProvider, sign_in_page_url,
};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Default)]
pub struct SignInState {
    pub gate: tokio::sync::Mutex<()>,
    pending: Mutex<Option<Pending>>,
    completed: Mutex<Option<String>>,
}
impl SignInState {
    fn persist_attempt(
        &self,
        event_id: &str,
        time: u64,
        save: impl FnOnce() -> Result<(), AppError>,
    ) -> Result<(), AppError> {
        let mut pending = self.pending.lock().map_err(native_error)?;
        pending
            .as_ref()
            .ok_or_else(|| native_error("no pending sign-in"))?
            .validate(event_id, time)?;
        let mut completed = self.completed.lock().map_err(native_error)?;
        save()?;
        *pending = None;
        *completed = Some(event_id.to_owned());
        Ok(())
    }
}
#[derive(Clone)]
struct Pending {
    event_id: String,
    provider: SignInProvider,
    started_at: u64,
    deadline: u64,
}
impl Pending {
    fn new(provider: SignInProvider) -> Result<Self, AppError> {
        let started_at = now()?;
        Ok(Self {
            event_id: honkoku_core::firestore::auto_id(),
            provider,
            started_at,
            deadline: started_at + Duration::from_secs(10 * 60).as_secs(),
        })
    }
    fn validate(&self, event_id: &str, time: u64) -> Result<(), AppError> {
        if self.event_id != event_id || time < self.started_at || time >= self.deadline {
            return Err(native_error("invalid or expired attempt"));
        }
        Ok(())
    }
    fn url(&self) -> Result<reqwest::Url, AppError> {
        let mut url = sign_in_page_url(self.provider, &self.event_id)?;
        url.query_pairs_mut()
            .append_pair("started", &self.started_at.to_string())
            .append_pair("deadline", &self.deadline.to_string());
        Ok(url)
    }
}
fn now() -> Result<u64, AppError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(native_error)?
        .as_secs())
}
fn trusted_window(window: &WebviewWindow) -> Result<(), AppError> {
    let url = window.url().map_err(native_error)?;
    if window.label() != "signin"
        || url.origin().ascii_serialization() != SIGN_IN_ORIGIN
        || url.path() != "/__client_signin"
    {
        return Err(native_error("untrusted window"));
    }
    Ok(())
}
fn native_error(_: impl std::fmt::Display) -> AppError {
    AppError {
        kind: "signin".into(),
        message: "ログインウィンドウを操作できませんでした。再試行してください。".into(),
    }
}
fn profile_root(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(native_error)?
        .join("signin"))
}
fn profile_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let root = profile_root(app)?;
    std::fs::create_dir_all(&root).map_err(native_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .map_err(native_error)?;
    }
    let marker = root.join("profile");
    let id = match std::fs::read_to_string(&marker) {
        Ok(id) if id.len() == 20 && id.bytes().all(|b| b.is_ascii_alphanumeric()) => id,
        Ok(_) => return Err(native_error("invalid profile")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let id = honkoku_core::firestore::auto_id();
            std::fs::write(marker, &id).map_err(native_error)?;
            id
        }
        Err(error) => return Err(native_error(error)),
    };
    Ok(root.join(id))
}
fn window_builder<'a>(
    app: &'a tauri::AppHandle,
    label: &'a str,
    url: WebviewUrl,
    profile: PathBuf,
) -> WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle> {
    let builder = WebviewWindowBuilder::new(app, label, url);
    #[cfg(target_os = "macos")]
    let builder = {
        let mut identifier = [0; 16];
        if let Some(name) = profile.file_name().and_then(|v| v.to_str()) {
            identifier.copy_from_slice(&name.as_bytes()[..16]);
        }
        builder.data_store_identifier(identifier)
    };
    builder.data_directory(profile)
}
// X rejects WebView2/Edg user agents at its OAuth endpoint. Retain the Chrome
// string until both agents can be compared in the Windows sign-in window.
fn user_agent() -> &'static str {
    if cfg!(target_os = "macos") {
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36"
    } else if cfg!(target_os = "windows") {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36"
    } else {
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36"
    }
}
#[tauri::command]
pub async fn session_sign_in(
    app: tauri::AppHandle,
    signin: State<'_, SignInState>,
    provider: SignInProvider,
) -> Result<(), AppError> {
    let _gate = signin.gate.lock().await;
    if let Some(window) = app.get_webview_window("signin") {
        window.set_focus().map_err(native_error)?;
        return Ok(());
    }
    let attempt = Pending::new(provider)?;
    let script = include_str!("signin.js").replace("__SIGN_IN_CONFIG__", &serde_json::json!({
        "apiKey": FIREBASE_API_KEY, "authDomain": FIREBASE_AUTH_DOMAIN, "origin": SIGN_IN_ORIGIN,
        "sdkVersion": FIREBASE_SDK_VERSION, "provider": provider.id()
    }).to_string());
    let url = attempt.url()?;
    let builder = window_builder(
        &app,
        "signin",
        WebviewUrl::External(url),
        profile_path(&app)?,
    )
    .title("ログイン")
    .inner_size(520.0, 720.0)
    .center()
    .user_agent(user_agent())
    .initialization_script(script);
    #[cfg(debug_assertions)]
    let builder = builder.on_navigation(|url| {
        if url.scheme() == "honkoku-signin-probe" {
            println!("signin initialization script: {}", url.host_str().unwrap_or("unknown"));
            return false;
        }
        true
    }).on_page_load(|window, payload| {
        if std::env::var_os("HONKOKU_SIGNIN_PROBE").is_some()
            && payload.url().origin().ascii_serialization() == SIGN_IN_ORIGIN {
            let _ = window.eval(r#"(async () => {
                    if (!window.__HONKOKU_SIGNIN_READY__) return;
                    const invoke = window.__TAURI_INTERNALS__.invoke;
                    const denied = await invoke('session_current').then(() => false, error => String(error).includes('not allowed'));
                    const capture = await invoke('session_capture', {captured:{}}).then(() => false, error => String(error).includes('invalid args'));
                    location.href = 'honkoku-signin-probe://' + (denied && capture ? 'ready-restricted' : 'permission-check-failed');
                })()"#);
        }
    });
    *signin.pending.lock().map_err(native_error)? = Some(attempt);
    *signin.completed.lock().map_err(native_error)? = None;
    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            *signin.pending.lock().map_err(native_error)? = None;
            return Err(native_error(error));
        }
    };
    let handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let state = handle.state::<SignInState>();
            if let Ok(mut pending) = state.pending.lock() {
                *pending = None;
            }
            if let Ok(mut completed) = state.completed.lock() {
                *completed = None;
            }
            let _ = handle.emit_to("main", "signin-closed", ());
        }
    });
    #[cfg(debug_assertions)]
    if std::env::var_os("HONKOKU_SIGNIN_PROBE").is_some() {
        window
            .navigate(window.url().map_err(native_error)?)
            .map_err(native_error)?;
    }
    Ok(())
}
#[tauri::command]
pub async fn session_capture(
    window: WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
    signin: State<'_, SignInState>,
    captured: CapturedSession,
) -> Result<SessionInfo, AppError> {
    let _gate = signin.gate.lock().await;
    trusted_window(&window)?;
    let attempt = signin
        .pending
        .lock()
        .map_err(native_error)?
        .clone()
        .ok_or_else(|| native_error("no pending sign-in"))?;
    attempt.validate(&captured.attempt_id, now()?)?;
    let session = captured.verify(attempt.provider).await?;
    let info = commands::attach_session_with(&state, &editing, session, |session| {
        // Validate again at the write boundary, after network and connection-lock waits.
        signin.persist_attempt(&attempt.event_id, now()?, || {
            Ok(state.store.save(session)?)
        })
    })
    .await?;
    // A notification failure must still acknowledge persistence so the page can sign out.
    if app.emit_to("main", "session-changed", &info).is_err() {
        eprintln!("signin: session notification failed");
    }
    Ok(info)
}

#[tauri::command]
pub async fn session_sign_in_retry(
    window: WebviewWindow,
    signin: State<'_, SignInState>,
    attempt_id: String,
) -> Result<(), AppError> {
    let _gate = signin.gate.lock().await;
    trusted_window(&window)?;
    let mut pending = signin.pending.lock().map_err(native_error)?;
    let previous = pending
        .as_ref()
        .filter(|p| p.event_id == attempt_id)
        .ok_or_else(|| native_error("no pending sign-in"))?;
    let attempt = Pending::new(previous.provider)?;
    let url = attempt.url()?;
    *pending = Some(attempt);
    window.navigate(url).map_err(native_error)?;
    Ok(())
}

/// The page calls this only after Firebase signOut has removed its local credential.
#[tauri::command]
pub async fn session_sign_in_complete(
    window: WebviewWindow,
    signin: State<'_, SignInState>,
    attempt_id: String,
) -> Result<(), AppError> {
    let _gate = signin.gate.lock().await;
    trusted_window(&window)?;
    if signin.completed.lock().map_err(native_error)?.as_deref() != Some(&attempt_id) {
        return Err(native_error("no completed sign-in"));
    }
    window.close().map_err(native_error)?;
    Ok(())
}

pub fn cancel(app: &tauri::AppHandle, signin: &SignInState) -> Result<(), AppError> {
    *signin.completed.lock().map_err(native_error)? = None;
    *signin.pending.lock().map_err(native_error)? = None;
    if let Some(window) = app.get_webview_window("signin") {
        window.destroy().map_err(native_error)?;
    }
    Ok(())
}

pub async fn clear_profile(app: &tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("signin") {
        window.destroy().map_err(native_error)?;
    }
    let root = profile_root(app)?;
    if !root.exists() {
        return Ok(());
    }
    // A fresh profile name also avoids reusing an in-memory WebKit context.
    let window = window_builder(
        app,
        "signin-cleanup",
        WebviewUrl::External("about:blank".parse().map_err(native_error)?),
        profile_path(app)?,
    )
    .visible(false)
    .build()
    .map_err(native_error)?;
    window.clear_all_browsing_data().map_err(native_error)?;
    window.destroy().map_err(native_error)?;
    std::fs::remove_dir_all(root).map_err(native_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_consumes_attempt_before_fallible_notification() {
        let state = SignInState::default();
        let attempt = Pending::new(SignInProvider::Twitter).unwrap();
        *state.pending.lock().unwrap() = Some(attempt.clone());
        assert!(
            state
                .persist_attempt(&attempt.event_id, attempt.started_at, || Err(native_error(
                    "store unavailable"
                )))
                .is_err()
        );
        assert!(state.pending.lock().unwrap().is_some());
        assert!(state.completed.lock().unwrap().is_none());
        state
            .persist_attempt(&attempt.event_id, attempt.started_at, || Ok(()))
            .unwrap();
        let notify = || -> Result<(), AppError> {
            assert!(state.pending.lock().unwrap().is_none());
            Err(native_error("notification failed"))
        };
        assert!(notify().is_err());
        assert!(
            state
                .persist_attempt(&attempt.event_id, attempt.started_at, || panic!(
                    "duplicate persistence"
                ))
                .is_err()
        );
        assert_eq!(
            state.completed.lock().unwrap().as_deref(),
            Some(attempt.event_id.as_str())
        );
    }

    #[test]
    fn attempts_bind_identity_and_expire_at_ten_minutes() {
        let attempt = Pending::new(SignInProvider::Twitter).unwrap();
        assert_eq!(attempt.deadline - attempt.started_at, 600);
        assert!(
            attempt
                .validate(&attempt.event_id, attempt.started_at)
                .is_ok()
        );
        assert!(attempt.validate("old-attempt", attempt.started_at).is_err());
        assert!(
            attempt
                .validate(&attempt.event_id, attempt.deadline)
                .is_err()
        );
        assert!(
            attempt
                .validate(&attempt.event_id, attempt.started_at - 1)
                .is_err()
        );
        let retry = Pending::new(attempt.provider).unwrap();
        assert_ne!(retry.event_id, attempt.event_id);
        assert!(retry.validate(&attempt.event_id, retry.started_at).is_err());
    }
}
