//! Measure cached corpus enrichment and verify a temporary account clip.
use honkoku_core::{
    HonkokuClient, Result,
    auth::{Session, SessionStore, TokenManager, import_dev_session},
    clips::ClipInput,
};
use honkoku_search::{Mode, Query, Searcher};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
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
    let storage = Arc::new(Mutex::new(honkoku_storage::Storage::open(
        ".local/glyph-verification.sqlite",
    )?));
    let client = HonkokuClient::new()?
        .with_session(TokenManager::new(session, Arc::new(MemorySession))?)
        .with_storage(storage);
    if std::env::args().any(|a| a == "--inspect-clips") {
        println!("Clips: {}", client.clips().await?.len());
        return Ok(());
    }
    let mut report: Value = std::fs::read(".local/logs/glyphs-live.json")
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}));
    if std::env::args().any(|a| a == "--live-clip") {
        let entry = "90aaa0afa9cf2f14fc579f138ed2fca9";
        let marker = format!("切り抜きの保存確認{}", honkoku_core::firestore::auto_id());
        let created = client
            .create_clip(ClipInput {
                entry_id: entry.into(),
                index: 23,
                reading: "候".into(),
                tags: vec!["保存確認".into()],
                comment: marker.clone(),
                is_private: true,
                xywh: [200, 200, 80, 120],
            })
            .await;
        let clip = match created {
            Ok(clip) => clip,
            Err(error) => {
                for clip in client
                    .clips()
                    .await?
                    .iter()
                    .filter(|c| c.input.comment == marker)
                {
                    client.delete_clip(&clip.id).await?;
                }
                return Err(error.into());
            }
        };
        report["clip"] = serde_json::to_value(&clip)?;
        std::fs::write(
            ".local/logs/glyphs-live.json",
            serde_json::to_vec_pretty(&report)?,
        )?;
        let readback = client.clips().await;
        let deletion = client.delete_clip(&clip.id).await;
        if let Err(error) = deletion {
            return Err(error.into());
        }
        report["deleted"] = json!(true);
        std::fs::write(
            ".local/logs/glyphs-live.json",
            serde_json::to_vec_pretty(&report)?,
        )?;
        let rows = readback?;
        let found = rows
            .iter()
            .find(|c| c.id == clip.id)
            .ok_or("created clip missing from account query")?;
        report["readback"] = serde_json::to_value(found)?;
        let remaining = client.clips().await?.iter().any(|c| c.id == clip.id);
        let missing = client
            .batch_get([format!("imageCollectionEntries/{}", clip.id)])
            .await?
            .into_iter()
            .all(|d| d.is_none());
        report["deletionVerified"] = json!(!remaining && missing);
        if remaining || !missing {
            return Err("temporary clip still exists".into());
        }
        std::fs::write(
            ".local/logs/glyphs-live.json",
            serde_json::to_vec_pretty(&report)?,
        )?;
    }
    if report["deleted"].as_bool() == Some(true)
        && let Some(id) = report["clip"]["id"].as_str()
    {
        let missing = client
            .batch_get([format!("imageCollectionEntries/{id}")])
            .await?
            .into_iter()
            .all(|d| d.is_none());
        report["deletionVerified"] = json!(missing);
        if !missing {
            return Err("temporary clip still exists".into());
        }
        std::fs::write(
            ".local/logs/glyphs-live.json",
            serde_json::to_vec_pretty(&report)?,
        )?;
    }
    let search = Searcher::open(home.join(".cache/honkoku-client/search"))?;
    let mut timings = Vec::<Value>::new();
    for character in ["候", "蝦夷"] {
        for run in ["cold", "warm"] {
            let start = Instant::now();
            let hits = search.search(Query {
                text: character.into(),
                mode: Mode::Strict,
                project: None,
                entry: None,
                limit: 100,
                cursor: None,
            })?;
            let search_ms = start.elapsed().as_secs_f64() * 1000.;
            let result = client.glyph_attestations(character, hits).await?;
            let timing = json!({"character":character,"run":run,"milliseconds":start.elapsed().as_secs_f64()*1000.,"searchMs":search_ms,"firestoreReads":result.firestore_reads,"pages":result.pages.len(),"pageErrors":result.pages.iter().filter(|p| p.error.is_some()).count()});
            println!("{timing}");
            timings.push(timing);
            report["timings"] = json!(timings);
            std::fs::write(
                ".local/logs/glyphs-live.json",
                serde_json::to_vec_pretty(&report)?,
            )?;
        }
    }
    Ok(())
}
