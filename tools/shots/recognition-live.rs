//! Read one real page crop and inspect the Metom response without saving a transcription.
use base64::{Engine, engine::general_purpose::STANDARD};
use honkoku_core::{
    HonkokuClient, Result,
    auth::{Session, SessionStore, TokenManager, import_dev_session},
};
use honkoku_iiif::{Fetcher, ImageCache};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
struct MemorySession;
impl SessionStore for MemorySession {
    fn load(&self) -> Result<Option<Session>> {
        Ok(None)
    }
    fn save(&self, _: &Session) -> Result<()> {
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        Ok(())
    }
}
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("home directory unavailable")?);
    let session = import_dev_session(home.join(".local/share/honkoku-client/session.json"))?;
    let client =
        HonkokuClient::new()?.with_session(TokenManager::new(session, Arc::new(MemorySession))?);
    let entry = client.entry("90aaa0afa9cf2f14fc579f138ed2fca9").await?;
    let canvas = entry
        .canvases
        .as_ref()
        .and_then(|c| c.get(23))
        .ok_or("page 23 canvas missing")?;
    let info_url = canvas
        .info_json_url
        .as_deref()
        .ok_or("image service missing")?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let fetcher = Fetcher::new(
        ImageCache::open(".local/recognition-live-cache", 8 * 1024 * 1024)?,
        http.clone(),
    );
    fetcher.allow_url(info_url)?;
    let cached = fetcher.get(info_url).await?;
    let info: Value = serde_json::from_slice(&cached.read().await?)?;
    let region = [200, 200, 80, 120];
    let base = info["id"]
        .as_str()
        .or_else(|| info["@id"].as_str())
        .ok_or("image id missing")?;
    let crop = format!("{base}/200,200,80,120/80,/0/default.jpg");
    fetcher.allow_url(&crop)?;
    let image = fetcher.get(&crop).await?;
    let bytes = image.read().await?;
    let response = http
        .post(honkoku_core::recognition::METOM_ENDPOINT)
        .json(&json!({"image_base64":STANDARD.encode(&bytes),"k":10,"return_probs":true}))
        .send()
        .await?;
    let status = response.status().as_u16();
    let body = response.text().await?;
    let parsed = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({"nonJson":body}));
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"entryId":entry.id,"index":23,"xywh":region,"cropUrl":crop,"imageBytes":bytes.len(),"endpoint":honkoku_core::recognition::METOM_ENDPOINT,"request":{"image_base64":"<crop bytes encoded as base64>","k":10,"return_probs":true},"httpStatus":status,"response":parsed})
        )?
    );
    Ok(())
}
