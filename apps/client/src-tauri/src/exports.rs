use crate::{AppError, AppState};
use honkoku_iiif::Fetcher;
use serde_json::Value;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

fn failure(message: impl Into<String>) -> AppError {
    AppError {
        kind: "export".into(),
        message: message.into(),
    }
}
fn filename(name: &str) -> String {
    let value: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(120)
        .collect();
    let value = value.trim_matches([' ', '.']);
    if value.is_empty() {
        "翻刻文".into()
    } else {
        value.into()
    }
}
async fn save(
    app: tauri::AppHandle,
    name: String,
    extension: &str,
    bytes: Vec<u8>,
) -> Result<bool, AppError> {
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(name)
        .add_filter("保存形式", &[extension])
        .save_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive
        .await
        .map_err(|_| failure("保存先を取得できませんでした。"))?
    else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|_| failure("保存先を取得できませんでした。"))?;
    tokio::fs::write(path, bytes)
        .await
        .map_err(|_| failure("ファイルを保存できませんでした。"))?;
    Ok(true)
}
#[tauri::command]
pub async fn save_transcription(
    app: tauri::AppHandle,
    name: String,
    format: String,
    content: String,
) -> Result<bool, AppError> {
    if !["txt", "xml", "tex"].contains(&format.as_str()) {
        return Err(failure("保存形式を選んでください。"));
    }
    save(
        app,
        format!("{}.{}", filename(&name), format),
        &format,
        content.into_bytes(),
    )
    .await
}
#[tauri::command]
pub async fn entry_bibliography(
    entry_id: String,
    state: State<'_, AppState>,
    fetcher: State<'_, Fetcher>,
) -> Result<Value, AppError> {
    let connection = state.connection.read().await;
    let entry = crate::read_entry(&state, &connection.client, &entry_id).await?;
    fetcher
        .allow_url(&entry.manifest_url)
        .map_err(|_| failure("書誌情報のURLを確認できません。"))?;
    let cached = fetcher
        .get(&entry.manifest_url)
        .await
        .map_err(|_| failure("書誌情報を取得できませんでした。"))?;
    let bytes = cached
        .read()
        .await
        .map_err(|_| failure("書誌情報を読み込めませんでした。"))?;
    serde_json::from_slice(&bytes).map_err(|_| failure("書誌情報の形式を確認できません。"))
}
#[tauri::command]
pub async fn save_full_image(
    app: tauri::AppHandle,
    entry_id: String,
    index: u32,
    state: State<'_, AppState>,
    fetcher: State<'_, Fetcher>,
) -> Result<bool, AppError> {
    let connection = state.connection.read().await;
    let entry = crate::read_entry(&state, &connection.client, &entry_id).await?;
    fetcher
        .allow_url(&entry.manifest_url)
        .map_err(|_| failure("画像のURLを確認できません。"))?;
    let manifest = fetcher
        .get_manifest(&entry.manifest_url)
        .await
        .map_err(|_| failure("画像一覧を取得できませんでした。"))?;
    drop(connection);
    let canvas = manifest
        .canvases
        .get(index as usize)
        .ok_or_else(|| failure("指定されたコマがありません。"))?;
    let cached = if let Some(service) = &canvas.image_service {
        fetcher
            .allow_url(&service.id)
            .map_err(|_| failure("画像のURLを確認できません。"))?;
        fetcher.full_image(service).await
    } else if let Some(url) = &canvas.image_url {
        fetcher
            .allow_url(url)
            .map_err(|_| failure("画像のURLを確認できません。"))?;
        fetcher.get(url).await
    } else {
        return Err(failure("画像がありません。"));
    }
    .map_err(|_| failure("フルサイズ画像を取得できませんでした。"))?;
    let extension = match cached.content_type.split(';').next().unwrap_or_default() {
        "image/png" => "png",
        "image/tiff" => "tif",
        "image/webp" => "webp",
        "image/jpeg" => "jpg",
        _ => return Err(failure("取得したデータが対応する画像形式ではありません。")),
    };
    let bytes = cached
        .read()
        .await
        .map_err(|_| failure("画像を読み込めませんでした。"))?;
    let title = entry.label.preferred(&["ja", "en", "none"]);
    let title = if title.is_empty() {
        entry.id.clone()
    } else {
        title
    };
    save(
        app,
        format!("{}-{}.{}", filename(&title), index + 1, extension),
        extension,
        bytes.to_vec(),
    )
    .await
}
