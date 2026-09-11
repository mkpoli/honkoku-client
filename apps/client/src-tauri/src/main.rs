#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod exports;
mod iiif_protocol;
mod ocr;
mod search;
mod signin;
use honkoku_core::{
    HonkokuClient,
    cache::SharedStorage,
    model::{Collection, Entry, Page, Project},
};
use honkoku_storage::Storage;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
struct AppState {
    connection: tokio::sync::RwLock<Connection>,
    storage: SharedStorage,
    store: Arc<honkoku_core::auth::DesktopStore>,
}
struct Connection {
    client: HonkokuClient,
    session: Option<commands::SessionInfo>,
}
impl AppState {
    fn anonymous(&self) -> honkoku_core::Result<HonkokuClient> {
        Ok(HonkokuClient::new()?.with_storage(self.storage.clone()))
    }
}
#[derive(Debug, Serialize)]
struct AppError {
    kind: String,
    message: String,
}
impl From<honkoku_core::Error> for AppError {
    fn from(error: honkoku_core::Error) -> Self {
        use honkoku_core::Error;
        let kind = match &error {
            Error::Http(_) => "network",
            Error::Timeout => "timeout",
            Error::Storage(_) => "storage",
            Error::Json(_) | Error::Invalid(_) => "data",
            Error::Worker(_) => "internal",
            Error::Io(_) => "io",
            Error::Keyring(_) => "credentials",
            Error::SignedOut => "signed_out",
            Error::Conflict { .. } => "conflict",
        };
        Self {
            kind: kind.into(),
            message: error.to_string(),
        }
    }
}
#[tauri::command]
async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .cached_projects(&state.storage, false)
        .await?)
}
#[tauri::command]
async fn get_project(id: String, state: State<'_, AppState>) -> Result<Project, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .cached_project(&state.storage, &id, false)
        .await?)
}
#[tauri::command]
async fn list_collections(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Collection>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .cached_collections(&state.storage, &project_id, false)
        .await?)
}
#[tauri::command]
async fn get_collection(id: String, state: State<'_, AppState>) -> Result<Collection, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .cached_collection(&state.storage, &id, false)
        .await?)
}
#[tauri::command]
async fn get_entry(id: String, state: State<'_, AppState>) -> Result<Entry, AppError> {
    let connection = state.connection.read().await;
    read_entry(&state, &connection.client, &id).await
}
#[tauri::command]
async fn list_pages(entry_id: String, state: State<'_, AppState>) -> Result<Vec<Page>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .cached_pages(&state.storage, &entry_id, false)
        .await?)
}
async fn read_entry(state: &AppState, client: &HonkokuClient, id: &str) -> Result<Entry, AppError> {
    match client.cached_entry(&state.storage, id, false).await {
        Ok(entry) => Ok(entry),
        Err(honkoku_core::Error::Json(_)) => {
            let mut value: serde_json::Value = client.document(&format!("entries/{id}")).await?;
            if let Some(canvases) = value["canvases"].as_array_mut() {
                for canvas in canvases {
                    normalize_canvas(canvas);
                }
            }
            let entry: Entry = serde_json::from_value(value).map_err(honkoku_core::Error::Json)?;
            let copy = entry.clone();
            honkoku_core::cache::blocking(&state.storage, move |db| db.put_entry(&copy)).await?;
            Ok(entry)
        }
        Err(error) => Err(error.into()),
    }
}
fn normalize_canvas(canvas: &mut serde_json::Value) {
    let original = canvas.clone();
    if original["id"].is_null() {
        canvas["id"] = original["@id"].clone();
    }
    let resource = &original["images"][0]["resource"];
    let service = if resource["service"].is_array() {
        &resource["service"][0]
    } else {
        &resource["service"]
    };
    let service_id = service["@id"].as_str().or_else(|| service["id"].as_str());
    if original["infoJsonUrl"].is_null()
        && let Some(id) = service_id
    {
        canvas["infoJsonUrl"] =
            serde_json::json!(format!("{}/info.json", id.trim_end_matches('/')));
    }
    if original["imageUrl"].is_null() {
        canvas["imageUrl"] = resource
            .get("@id")
            .or_else(|| resource.get("id"))
            .cloned()
            .unwrap_or_default();
    }
    if original["thumbnailUrl"].is_null() {
        canvas["thumbnailUrl"] = original["thumbnail"]
            .as_str()
            .map(serde_json::Value::from)
            .or_else(|| original["thumbnail"].get("@id").cloned())
            .or_else(|| original["thumbnail"].get("id").cloned())
            .unwrap_or_else(|| canvas["imageUrl"].clone());
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    install_early_diagnostics();
    let result = run();
    if let Err(error) = &result {
        early_diagnostic(&format!("exit with error: {error}"));
    }
    result
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    early_diagnostic(&format!("start {}", env!("CARGO_PKG_VERSION")));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("honkoku-iiif", iiif_protocol::handle)
        .setup(|app| {
            install_diagnostics(app);
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                iiif_protocol::initialize(app)?;
                app.manage(
                    honkoku_ocr::OcrSidecar::new(honkoku_ocr::OcrEnvironment::new(
                        app.path().data_dir()?.join("honkoku-client"),
                    ))
                    .with_log_path(app.path().app_log_dir()?.join("ocr.log")),
                );
                app.manage(commands::EditingState::default());
                app.manage(signin::SignInState::default());
                let cache = app.path().cache_dir()?.join("honkoku-client");
                std::fs::create_dir_all(&cache)?;
                use honkoku_core::auth::{DesktopStore, SessionStore, TokenManager};
                let storage = Arc::new(Mutex::new(Storage::open(cache.join("cache.sqlite"))?));
                let store = Arc::new(DesktopStore::new(
                    app.path().app_data_dir()?.join("session.json"),
                ));
                if let Some(reason) = store.fallback_reason() {
                    diagnostic(app, &format!("credential store: {reason}"));
                }
                // A credential that cannot be read means signed out, never a failed start.
                let stored = match store.load() {
                    Ok(stored) => stored,
                    Err(error) => {
                        diagnostic(app, &format!("stored session ignored: {error}"));
                        let _ = store.clear();
                        None
                    }
                };
                let session = stored
                    .as_ref()
                    .map(|session| commands::SessionInfo::new(session, store.kind()));
                let mut client = HonkokuClient::new()?.with_storage(storage.clone());
                if let Some(stored) = stored {
                    client = client.with_session(TokenManager::new(stored, store.clone())?);
                }
                search::initialize(app, cache.clone(), storage.clone());
                app.manage(AppState {
                    connection: tokio::sync::RwLock::new(Connection { client, session }),
                    storage,
                    store,
                });
                #[cfg(debug_assertions)]
                if std::env::var_os("HONKOKU_SIGNIN_PROBE").is_some() {
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let result = signin::session_sign_in(
                            handle.clone(),
                            handle.state::<signin::SignInState>(),
                            honkoku_core::auth::SignInProvider::Twitter,
                        )
                        .await;
                        println!(
                            "signin probe command: {}",
                            if result.is_ok() { "ok" } else { "failed" }
                        );
                    });
                }
                Ok(())
            })();
            if let Err(error) = &result {
                diagnostic(app, &format!("startup failed: {error}"));
            }
            result
        })
        .invoke_handler(tauri::generate_handler![
            commands::glyph_attestations,
            commands::glyph_image_url,
            commands::clips_list,
            commands::clip_create,
            commands::clip_delete,
            search::search_status,
            search::search_choose_dump,
            search::search_build,
            search::search_query,
            search::search_sync,
            ocr::ocr_status,
            ocr::ocr_diagnostics,
            ocr::ocr_doctor,
            ocr::ocr_repair_models,
            ocr::ocr_setup,
            ocr::ocr_run_page,
            ocr::ocr_cancel,
            ocr::ocr_result,
            ocr::ocr_publish_page,
            signin::session_sign_in,
            signin::session_capture,
            signin::session_sign_in_retry,
            signin::session_sign_in_complete,
            commands::session_import,
            commands::session_current,
            commands::session_clear,
            commands::home_timeline,
            commands::home_ranking,
            commands::home_ranking_self,
            commands::home_announcements,
            commands::home_daily_progress,
            commands::me,
            commands::unread_notification_count,
            commands::get_user,
            commands::page_lock,
            commands::page_draft,
            commands::page_draft_notes,
            commands::page_note_delete,
            commands::history_open,
            commands::history_recent,
            commands::page_history,
            exports::entry_bibliography,
            exports::save_transcription,
            exports::save_full_image,
            commands::history_clear,
            commands::page_save,
            commands::page_discard,
            commands::page_lock_state,
            commands::editing_pages,
            commands::project_page_activity,
            commands::region_cached,
            commands::region_refresh,
            list_projects,
            get_project,
            list_collections,
            commands::list_entry_summaries,
            commands::collection_progress,
            commands::entry_progress,
            get_collection,
            get_entry,
            list_pages,
            iiif_protocol::iiif_prepare_entry,
            iiif_protocol::iiif_local_url
        ])
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                // A sidecar that is busy installing or recognising must not keep
                // the process alive after the last window closes.
                tauri::async_runtime::block_on(async {
                    if let Some(engine) = app.try_state::<honkoku_ocr::OcrSidecar>() {
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            engine.shutdown(),
                        )
                        .await;
                    }
                });
                std::process::exit(0);
            }
        });
    Ok(())
}

/// Startup problems and panics are written to the platform log directory,
/// since a release build on Windows has no console to show them.
fn diagnostics_path(app: &tauri::App) -> Option<std::path::PathBuf> {
    let dir = app.path().app_log_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("startup.log"))
}
/// The same log file, located without Tauri so that failures before setup
/// are recorded as well: `%LOCALAPPDATA%\<identifier>\logs` on Windows,
/// `$XDG_DATA_HOME/<identifier>/logs` elsewhere.
fn early_diagnostics_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?
    } else if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        std::path::PathBuf::from(dir)
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join(".local/share")
    };
    let dir = base.join("li.mkpo.honkoku-client").join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("startup.log"))
}
fn early_diagnostic(message: &str) {
    if let Some(path) = early_diagnostics_path() {
        append_line(&path, message);
    }
}
fn install_early_diagnostics() {
    let Some(path) = early_diagnostics_path() else {
        return;
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        append_line(&path, &format!("panic: {info}"));
        previous(info);
    }));
}
fn diagnostic(app: &tauri::App, message: &str) {
    if let Some(path) = diagnostics_path(app) {
        append_line(&path, message);
    }
}
fn append_line(path: &std::path::Path, message: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let _ = writeln!(file, "{seconds} {message}");
    }
}
fn install_diagnostics(app: &tauri::App) {
    let Some(path) = diagnostics_path(app) else {
        return;
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        append_line(&path, &format!("panic: {info}"));
        previous(info);
    }));
}

#[cfg(test)]
mod entry_tests {
    use super::*;
    #[test]
    fn presentation_two_canvas_becomes_readable_without_losing_metadata() {
        let mut canvas = serde_json::json!({"@id":"https://library.example/canvas/1","width":1000,"height":800,"label":"一","images":[{"resource":{"@id":"https://library.example/image/full/full/0/default.jpg","service":{"@id":"https://library.example/image"}}}],"thumbnail":{"@id":"https://library.example/thumb.jpg"}});
        normalize_canvas(&mut canvas);
        let typed: honkoku_core::model::Canvas = serde_json::from_value(canvas).unwrap();
        assert_eq!(typed.id, "https://library.example/canvas/1");
        assert_eq!(
            typed.info_json_url.as_deref(),
            Some("https://library.example/image/info.json")
        );
        assert_eq!(
            typed.thumbnail_url.as_deref(),
            Some("https://library.example/thumb.jpg")
        );
        assert_eq!(typed.extra["label"], "一");
    }
}
