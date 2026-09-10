use crate::{AppError, AppState};
use honkoku_core::cache::blocking;
use honkoku_iiif::Fetcher;
use honkoku_ocr::{OcrEngine, OcrPage, OcrSidecar, OcrStatus, ProgressHandler};
use std::sync::Arc;
use tauri::{Emitter, State};
impl From<honkoku_ocr::Error> for AppError {
    fn from(error: honkoku_ocr::Error) -> Self {
        Self {
            kind: "ocr".into(),
            message: honkoku_ocr::redact(&error.to_string()),
        }
    }
}
fn progress(app: tauri::AppHandle) -> ProgressHandler {
    Arc::new(move |event| {
        let _ = app.emit("ocr-progress", event);
    })
}
#[tauri::command]
pub async fn ocr_status(engine: State<'_, OcrSidecar>) -> Result<OcrStatus, AppError> {
    Ok(engine.status().await?)
}
#[tauri::command]
pub async fn ocr_setup(
    use_gpu: bool,
    app: tauri::AppHandle,
    engine: State<'_, OcrSidecar>,
) -> Result<OcrStatus, AppError> {
    Ok(engine.setup(use_gpu, progress(app)).await?)
}
#[tauri::command]
pub async fn ocr_run_page(
    entry_id: String,
    index: u32,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    engine: State<'_, OcrSidecar>,
    fetcher: State<'_, Fetcher>,
) -> Result<OcrPage, AppError> {
    let client = state.connection.read().await.client.clone();
    Ok(honkoku_ocr::run_page(
        engine.inner(),
        &client,
        &state.storage,
        &fetcher,
        &entry_id,
        index,
        progress(app),
    )
    .await?)
}
#[tauri::command]
pub async fn ocr_cancel(engine: State<'_, OcrSidecar>) -> Result<(), AppError> {
    Ok(engine.cancel().await?)
}
#[tauri::command]
pub async fn ocr_result(
    entry_id: String,
    index: u32,
    state: State<'_, AppState>,
) -> Result<Option<OcrPage>, AppError> {
    let id = format!("{entry_id}_{index}");
    Ok(blocking(&state.storage, move |db| db.ocr_result(&id)).await?)
}
#[tauri::command]
pub async fn ocr_publish_page(
    entry_id: String,
    index: u32,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let id = format!("{entry_id}_{index}");
    let result: OcrPage = blocking(&state.storage, move |db| db.ocr_result(&id))
        .await?
        .ok_or_else(|| AppError {
            kind: "ocr".into(),
            message: "ローカルOCRの結果がありません。".into(),
        })?;
    state
        .connection
        .read()
        .await
        .client
        .write_ocr(&entry_id, index, "minna", result.site_result()?)
        .await?;
    Ok(())
}
