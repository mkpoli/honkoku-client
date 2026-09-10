#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
    client: HonkokuClient,
    storage: SharedStorage,
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
    Ok(state.client.cached_projects(&state.storage, false).await?)
}
#[tauri::command]
async fn get_project(id: String, state: State<'_, AppState>) -> Result<Project, AppError> {
    Ok(state
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
        .client
        .cached_collections(&state.storage, &project_id, false)
        .await?)
}
#[tauri::command]
async fn get_collection(id: String, state: State<'_, AppState>) -> Result<Collection, AppError> {
    Ok(state
        .client
        .cached_collection(&state.storage, &id, false)
        .await?)
}
#[tauri::command]
async fn get_entry(id: String, state: State<'_, AppState>) -> Result<Entry, AppError> {
    Ok(state
        .client
        .cached_entry(&state.storage, &id, false)
        .await?)
}
#[tauri::command]
async fn list_pages(entry_id: String, state: State<'_, AppState>) -> Result<Vec<Page>, AppError> {
    Ok(state
        .client
        .cached_pages(&state.storage, &entry_id, false)
        .await?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("honkoku-iiif", iiif_protocol::handle)
        .setup(|app| {
            iiif_protocol::initialize(app)?;
            let cache = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&cache)?;
            app.manage(AppState {
                client: HonkokuClient::new()?,
                storage: Arc::new(Mutex::new(Storage::open(cache.join("cache.sqlite"))?)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
