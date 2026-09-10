#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod iiif_protocol;
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
    store: Arc<honkoku_core::auth::FileStore>,
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
            serde_json::from_value(value).map_err(|e| honkoku_core::Error::Json(e).into())
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("honkoku-iiif", iiif_protocol::handle)
        .setup(|app| {
            iiif_protocol::initialize(app)?;
            let cache = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&cache)?;
            use honkoku_core::auth::{FileStore, SessionStore, TokenManager};
            let storage = Arc::new(Mutex::new(Storage::open(cache.join("cache.sqlite"))?));
            let store = Arc::new(FileStore::new(
                app.path().app_data_dir()?.join("session.json"),
            ));
            let stored = store.load()?;
            let session = stored.as_ref().map(commands::SessionInfo::from);
            let mut client = HonkokuClient::new()?.with_storage(storage.clone());
            if let Some(stored) = stored {
                client = client.with_session(TokenManager::new(stored, store.clone())?);
            }
            app.manage(AppState {
                connection: tokio::sync::RwLock::new(Connection { client, session }),
                storage,
                store,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::session_import,
            commands::session_current,
            commands::session_clear,
            commands::home_timeline,
            commands::home_ranking,
            commands::home_announcements,
            commands::home_daily_progress,
            commands::me,
            commands::unread_notification_count,
            commands::get_user,
            list_projects,
            get_project,
            list_collections,
            get_collection,
            get_entry,
            list_pages,
            iiif_protocol::iiif_prepare_entry,
            iiif_protocol::iiif_local_url
        ])
        .run(tauri::generate_context!())?;
    Ok(())
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
