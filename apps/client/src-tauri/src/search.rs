use crate::{AppError, AppState};
use honkoku_core::cache::{SharedStorage, blocking};
use honkoku_search::{IndexBuilder, LivePage, Query, Results};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub struct SearchState {
    path: PathBuf,
    writer: Mutex<Option<IndexBuilder>>,
    reader: Mutex<Option<honkoku_search::Searcher>>,
}
fn error(e: impl std::fmt::Display) -> AppError {
    let mut message = e.to_string();
    if let Some(home) = std::env::var_os("HOME") {
        message = message.replace(&home.to_string_lossy().to_string(), "~");
    }
    AppError {
        kind: "search".into(),
        message,
    }
}
async fn task<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(error)?
}
impl SearchState {
    fn write<T>(
        &self,
        operation: impl FnOnce(&mut IndexBuilder) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut guard = self.writer.lock().map_err(error)?;
        if guard.is_none() {
            *guard = Some(IndexBuilder::open(&self.path).map_err(error)?);
        }
        let writer = guard
            .as_mut()
            .ok_or_else(|| error("index writer unavailable"))?;
        let result = operation(writer);
        if result.is_err() {
            *guard = None;
        }
        result
    }
}
pub fn initialize(app: &tauri::App, cache: PathBuf, storage: SharedStorage) {
    let state = Arc::new(SearchState {
        path: cache.join("search"),
        writer: Mutex::new(None),
        reader: Mutex::new(None),
    });
    app.manage(state.clone());
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            if let Err(e) = sync(state.clone(), storage.clone()).await {
                let _ = handle.emit("search-error", &e);
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    });
}
#[derive(Serialize)]
pub struct SearchStatus {
    #[serde(flatten)]
    index: honkoku_search::Status,
    git_available: bool,
    configured: bool,
}
#[tauri::command]
pub async fn search_status(
    search: State<'_, Arc<SearchState>>,
    state: State<'_, AppState>,
) -> Result<SearchStatus, AppError> {
    let path = search.path.clone();
    let configured = blocking(&state.storage, |db| db.search_setting("dump_path"))
        .await?
        .is_some();
    task(move || {
        Ok(SearchStatus {
            index: honkoku_search::status(path).map_err(error)?,
            git_available: std::process::Command::new("git")
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success()),
            configured,
        })
    })
    .await
}
#[tauri::command]
pub async fn search_choose_dump(app: tauri::AppHandle) -> Result<Option<String>, AppError> {
    task(move || {
        Ok(app
            .dialog()
            .file()
            .set_title("翻刻データのフォルダー")
            .blocking_pick_folder()
            .map(|path| path.to_string()))
    })
    .await
}
#[tauri::command]
pub async fn search_build(
    dump_path: Option<String>,
    clone_dump: Option<bool>,
    app: tauri::AppHandle,
    search: State<'_, Arc<SearchState>>,
    state: State<'_, AppState>,
) -> Result<honkoku_search::Status, AppError> {
    let path = match dump_path {
        Some(path) => Some(path),
        None => blocking(&state.storage, |db| db.search_setting("dump_path")).await?,
    };
    let search = search.inner().clone();
    let storage = state.storage.clone();
    let sync_state = search.clone();
    let search_path = search.path.clone();
    task(move || {
        let dump = if clone_dump.unwrap_or(false) {
            let dump = search.path.parent().ok_or_else(||error("cache directory unavailable"))?.join("honkoku-data");
            if !dump.exists() {
                let output = std::process::Command::new("git").args(["clone","--depth","1","https://github.com/yuta1984/honkoku-data"]).arg(&dump).output().map_err(error)?;
                if !output.status.success() { return Err(error("翻刻データをクローンできませんでした。通信状態と空き容量を確認してください。")); }
            }
            dump
        } else { PathBuf::from(path.ok_or_else(||error("翻刻データのフォルダーを選んでください。"))?) };
        let commit = honkoku_search::dump_commit(&dump).map_err(error)?;
        storage.lock().map_err(error)?.set_search_setting("dump_path",&dump.to_string_lossy()).map_err(error)?;
        search.write(|writer| {
            writer.from_dump(&dump,&commit,|progress| { let _ = app.emit("search-progress",progress); }).map_err(error)?;
            Ok(())
        })?;
        honkoku_search::status(&search.path).map_err(error)
    }).await?;
    sync(sync_state, state.storage.clone()).await?;
    let path = search_path;
    task(move || honkoku_search::status(path).map_err(error)).await
}
#[tauri::command]
pub async fn search_query(
    query: Query,
    search: State<'_, Arc<SearchState>>,
) -> Result<Results, AppError> {
    let search = search.inner().clone();
    task(move || {
        let mut reader = search.reader.lock().map_err(error)?;
        if reader.is_none() {
            *reader = Some(honkoku_search::Searcher::open(&search.path).map_err(error)?);
        }
        reader
            .as_ref()
            .ok_or_else(|| error("index reader unavailable"))?
            .search(query)
            .map_err(error)
    })
    .await
}
#[tauri::command]
pub async fn search_sync(
    search: State<'_, Arc<SearchState>>,
    state: State<'_, AppState>,
) -> Result<u64, AppError> {
    sync(search.inner().clone(), state.storage.clone()).await
}
async fn sync(search: Arc<SearchState>, storage: SharedStorage) -> Result<u64, AppError> {
    task(move || {
        if storage
            .lock()
            .map_err(error)?
            .search_changes(1)
            .map_err(error)?
            .is_empty()
        {
            return Ok(0);
        }
        search.write(|writer| {
            let mut count = 0;
            loop {
                let changes = storage
                    .lock()
                    .map_err(error)?
                    .search_changes(100)
                    .map_err(error)?;
                if changes.is_empty() {
                    break;
                }
                let sequences: Vec<_> = changes.iter().map(|c| c.sequence).collect();
                let mut pages = Vec::new();
                for change in changes {
                    if let Some(page) = change.page {
                        let entry = change.entry.unwrap_or_default();
                        let project = change.project.unwrap_or_default();
                        pages.push(LivePage {
                            page_id: change.page_id,
                            project_id: entry["projectId"].as_str().unwrap_or_default().into(),
                            entry_id: page["entryId"].as_str().unwrap_or_default().into(),
                            index: page["index"].as_u64().unwrap_or_default(),
                            text: page["text"].as_str().unwrap_or_default().into(),
                            updated_at: page["updatedAt"]
                                .as_str()
                                .and_then(|s| {
                                    time::OffsetDateTime::parse(
                                        s,
                                        &time::format_description::well_known::Rfc3339,
                                    )
                                    .ok()
                                })
                                .map_or(0, |t| t.unix_timestamp()),
                            entry_label: entry["label"]
                                .as_str()
                                .map(str::to_owned)
                                .or_else(|| {
                                    entry["label"].as_object().and_then(|m| {
                                        m.values().find_map(|v| {
                                            v.as_array()?.first()?.as_str().map(str::to_owned)
                                        })
                                    })
                                })
                                .unwrap_or_default(),
                            project_title: project["title"].as_str().unwrap_or_default().into(),
                        });
                    } else {
                        writer.remove(&change.page_id).map_err(error)?;
                    }
                }
                count += writer.apply_live(pages).map_err(error)?;
                storage
                    .lock()
                    .map_err(error)?
                    .acknowledge_search(&sequences)
                    .map_err(error)?;
            }
            Ok(count)
        })
    })
    .await
}
pub async fn cache_page(
    state: &AppState,
    page: &honkoku_core::model::Page,
) -> Result<(), AppError> {
    let page = page.clone();
    blocking(&state.storage, move |db| db.put_page(&page)).await?;
    Ok(())
}
