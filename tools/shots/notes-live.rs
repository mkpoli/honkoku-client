//! Verify a temporary note draft without publishing a transcription.
use honkoku_core::{
    HonkokuClient, Result,
    auth::{Session, SessionStore, TokenManager, import_dev_session},
    firestore::plain_value,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
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
    let identity = import_dev_session(home.join(".local/share/honkoku-client/session.json"))?;
    let uid = identity.uid.clone();
    let client =
        HonkokuClient::new()?.with_session(TokenManager::new(identity, Arc::new(MemorySession))?);
    let entry = "90aaa0afa9cf2f14fc579f138ed2fca9";
    let index = 23;
    let mut edit = client.lock_page(entry, index, false).await?;
    let original = edit.page().clone();
    let original_notes = original
        .temp_notes
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|n| n.unwrap_or(serde_json::Value::Null))
        .collect::<Vec<_>>();
    let outcome: std::result::Result<serde_json::Value, Box<dyn std::error::Error>> = async {
        let timestamp = serde_json::to_value(honkoku_core::model::Timestamp(
            time::OffsetDateTime::now_utc(),
        ))?;
        let note = json!({
            "id": "", "type": "memo", "content": "注釈の保存確認",
            "markdown": "注釈の保存確認", "createdBy": uid,
            "createdAt": timestamp, "updatedAt": timestamp
        });
        let precondition = edit.update_time().to_owned();
        let mut notes = original_notes.clone();
        notes.push(note.clone());
        edit.draft_notes(&notes).await?;
        let document = client
            .batch_get([format!("transcriptions/{entry}_{index}")])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or("readback missing")?;
        let fields = plain_value(&document.fields);
        let stored = document.fields["tempNotes"]
            .as_array()
            .and_then(|notes| notes.last())
            .ok_or("note missing")?;
        for key in ["id", "type", "content", "markdown", "createdBy"] {
            if stored[key] != note[key] {
                return Err(format!("note readback mismatch: {key}").into());
            }
        }
        for key in ["createdAt", "updatedAt"] {
            if stored[key].get("$firestoreTimestamp").is_none() {
                return Err(format!("timestamp type mismatch: {key}").into());
            }
        }
        Ok(json!({
            "entryId": entry, "index": index,
            "lock": {
                "status": original.status, "prevStatus": original.prev_status,
                "tempText": original.temp_text, "tempTextChanged": false,
                "tempNotes": original.temp_notes, "tempEditedBy": uid,
                "syncMode": false, "updatedAt": original.updated_at
            },
            "write": {
                "update": {"name": document.name, "fields": {
                    "tempNotes": honkoku_core::firestore::encode_value(&document.fields["tempNotes"])
                }},
                "updateMask": {"fieldPaths": ["tempNotes"]},
                "currentDocument": {"updateTime": precondition}
            },
            "readback": {
                "tempNotes": document.fields["tempNotes"], "status": fields["status"],
                "tempEditedBy": fields["tempEditedBy"], "updatedAt": fields["updatedAt"]
            },
            "publishedNotesUnchanged": fields["notes"] == json!(original.notes)
        }))
    }
    .await;
    // Remove the temporary probe from the draft as well as discarding the lock.
    let restore = edit.draft_notes(&original_notes).await;
    let discard = edit.discard().await;
    let path = ".local/logs/notes-live.json";
    let mut report = outcome
        .as_ref()
        .cloned()
        .unwrap_or_else(|error| json!({"error":error.to_string()}));
    report["draftRestored"] = json!(restore.is_ok());
    report["discarded"] = json!(discard.is_ok());
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    restore?;
    discard?;
    outcome?;
    let state = client.page_lock_state(entry, index).await?;
    report["finalStatus"] = json!(state.page.status);
    report["publishedNotesUnchanged"] = json!(state.page.notes == original.notes);
    report["temporaryNoteRemoved"] = json!(state.page.temp_notes == original.temp_notes);
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    if state.page.status != original.prev_status.ok_or("previous status missing")?
        || state.page.notes != original.notes
        || state.page.temp_notes != original.temp_notes
    {
        return Err("discard verification failed".into());
    }
    println!("Note readback, timestamp type, draft restoration, and discard verified.");
    Ok(())
}
