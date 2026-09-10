use crate::{
    AppError, AppState,
    commands::{self, EditingState, SessionInfo},
};
use honkoku_core::auth::{
    CapturedSession, FIREBASE_API_KEY, SIGN_IN_ORIGIN, SignInProvider, sign_in_url,
};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Default)]
pub struct SignInState {
    pub gate: tokio::sync::Mutex<()>,
    pending: Mutex<Option<Pending>>,
}
#[derive(Clone)]
struct Pending {
    event_id: String,
    provider: SignInProvider,
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
    let event_id = honkoku_core::firestore::auto_id();
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(native_error)?
        .as_secs();
    let script = include_str!("signin.js").replace("__SIGN_IN_CONFIG__", &serde_json::json!({
        "apiKey": FIREBASE_API_KEY, "provider": provider.id(), "eventId": event_id, "startedAt": started_at
    }).to_string());
    let url = sign_in_url(provider, &event_id)?;
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
    *signin.pending.lock().map_err(native_error)? = Some(Pending {
        event_id: event_id.clone(),
        provider,
    });
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
            if let Ok(mut pending) = state.pending.lock()
                && pending.as_ref().is_some_and(|p| p.event_id == event_id)
            {
                *pending = None;
            }
            let _ = handle.emit_to("main", "signin-closed", ());
        }
    });
    #[cfg(debug_assertions)]
    if std::env::var_os("HONKOKU_SIGNIN_PROBE").is_some() {
        window
            .navigate("https://app.honkoku.org/".parse().map_err(native_error)?)
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
    if window.label() != "signin"
        || window
            .url()
            .map_err(native_error)?
            .origin()
            .ascii_serialization()
            != SIGN_IN_ORIGIN
    {
        return Err(native_error("untrusted window"));
    }
    let provider = signin
        .pending
        .lock()
        .map_err(native_error)?
        .as_ref()
        .map(|p| p.provider)
        .ok_or_else(|| native_error("no pending sign-in"))?;
    let info = commands::attach_session(&state, &editing, captured.into_session(provider)?).await?;
    app.emit_to("main", "session-changed", &info)
        .map_err(native_error)?;
    // Persistence has succeeded; a failed close must not allow another capture.
    *signin.pending.lock().map_err(native_error)? = None;
    window.close().map_err(native_error)?;
    Ok(info)
}

pub fn cancel(app: &tauri::AppHandle, signin: &SignInState) -> Result<(), AppError> {
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
