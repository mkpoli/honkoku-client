//! Optimistic page editing transactions and coalesced draft persistence.
use crate::{
    Error, HonkokuClient, Result,
    firestore::{CommitResponse, Precondition, ReadDocument, ServerValue, Write, auto_id},
    model::{Page, PageStatus},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant, sleep_until};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveOptions {
    pub status: Option<PageStatus>,
    pub share: bool,
    pub request_review: bool,
    pub comment: String,
    pub is_approval: Option<bool>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPage {
    pub page: Page,
    pub timeline_event_id: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLockState {
    pub page: Page,
    pub page_id: String,
    pub status: PageStatus,
    pub temp_edited_by: Option<String>,
    pub is_mine: bool,
    pub sync_mode: bool,
    pub update_time: String,
}
#[derive(Clone)]
pub struct ProjectSnapshot {
    pub name: String,
    pub fields: Value,
}
#[derive(Clone, PartialEq)]
struct Draft {
    text: String,
    notes: Option<Vec<Value>>,
}
#[derive(Default)]
struct PendingDraft {
    text: Option<Draft>,
    closed: bool,
}
/// Submit the latest text even while the session is waiting on an earlier write.
#[derive(Clone, Default)]
pub struct DraftQueue(Arc<Mutex<PendingDraft>>);
impl DraftQueue {
    pub fn request(&self, text: &str) -> Result<()> {
        self.request_with_notes(text, None)
    }
    pub fn request_with_notes(&self, text: &str, notes: Option<Vec<Value>>) -> Result<()> {
        let mut pending = self.0.lock().map_err(|e| Error::Worker(e.to_string()))?;
        if pending.closed {
            return Err(Error::Invalid("Transcription is not being edited".into()));
        }
        let notes = notes.or_else(|| pending.text.as_ref().and_then(|draft| draft.notes.clone()));
        pending.text = Some(Draft {
            text: text.into(),
            notes,
        });
        Ok(())
    }
    fn latest(&self) -> Result<Option<Draft>> {
        Ok(self
            .0
            .lock()
            .map_err(|e| Error::Worker(e.to_string()))?
            .text
            .clone())
    }
    fn has_text(&self) -> Result<bool> {
        Ok(self
            .0
            .lock()
            .map_err(|e| Error::Worker(e.to_string()))?
            .text
            .is_some())
    }
    fn acknowledge(&self, text: &Draft) -> Result<()> {
        let mut pending = self.0.lock().map_err(|e| Error::Worker(e.to_string()))?;
        if pending.text.as_ref() == Some(text) {
            pending.text = None;
        }
        Ok(())
    }
    fn freeze(&self) -> Result<DraftFreeze> {
        self.0
            .lock()
            .map_err(|e| Error::Worker(e.to_string()))?
            .closed = true;
        Ok(DraftFreeze {
            queue: self.clone(),
            complete: false,
        })
    }
    fn close(&self) -> Result<()> {
        let mut pending = self.0.lock().map_err(|e| Error::Worker(e.to_string()))?;
        pending.closed = true;
        pending.text = None;
        Ok(())
    }
}
struct DraftFreeze {
    queue: DraftQueue,
    complete: bool,
}
impl Drop for DraftFreeze {
    fn drop(&mut self) {
        if !self.complete
            && let Ok(mut pending) = self.queue.0.lock()
        {
            pending.closed = false;
        }
    }
}
enum NoteChange {
    Replace(Value),
    Delete(usize),
}
pub struct EditingSession {
    client: HonkokuClient,
    entry_id: String,
    index: u32,
    uid: String,
    page: Page,
    update_time: String,
    project: Option<ProjectSnapshot>,
    project_update_time: String,
    document: ReadDocument,
    drafts: DraftQueue,
    last_draft: Option<Instant>,
}
fn page_path(entry_id: &str, index: u32) -> Result<String> {
    if entry_id.is_empty() || entry_id.contains('/') || matches!(entry_id, "." | "..") {
        return Err(Error::Invalid("invalid entry ID".into()));
    }
    Ok(format!("transcriptions/{entry_id}_{index}"))
}
impl HonkokuClient {
    pub async fn write_ocr(
        &self,
        entry_id: &str,
        index: u32,
        engine: &str,
        mut result: Value,
    ) -> Result<()> {
        self.signed_in_uid().await?;
        if !matches!(engine, "minna" | "ndl") || !result.is_object() {
            return Err(Error::Invalid("invalid OCR engine or result".into()));
        }
        if let Some(object) = result.as_object_mut() {
            object.remove("createdAt");
        }
        let document = self.read_edit_page(entry_id, index).await?;
        self.commit(vec![Write::Update {
            name: document.name,
            fields: json!({"ocr":{engine:result}}),
            update_mask: vec![format!("ocr.{engine}")],
            update_transforms: vec![
                ("updatedAt".into(), ServerValue::RequestTime),
                (format!("ocr.{engine}.createdAt"), ServerValue::RequestTime),
            ],
            precondition: Some(Precondition {
                update_time: document.update_time,
            }),
        }])
        .await?;
        Ok(())
    }

    async fn read_edit_page(&self, entry_id: &str, index: u32) -> Result<ReadDocument> {
        self.batch_get([page_path(entry_id, index)?])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| Error::Invalid("Transcription not found".into()))
    }
    async fn read_edit_project(&self, page: &ReadDocument) -> Result<Option<ReadDocument>> {
        let Some(id) = page.fields.get("projectId").filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        let id = id
            .as_str()
            .filter(|id| !id.is_empty() && !id.contains('/'))
            .ok_or_else(|| Error::Invalid("invalid project ID".into()))?;
        self.batch_get([format!("projects/{id}")])
            .await?
            .into_iter()
            .next()
            .flatten()
            .map(Some)
            .ok_or_else(|| Error::Invalid("Project not found".into()))
    }
    pub async fn page_lock_state(&self, entry_id: &str, index: u32) -> Result<PageLockState> {
        let uid = if self.session.is_some() {
            Some(self.signed_in_uid().await?)
        } else {
            None
        };
        let document = self.read_edit_page(entry_id, index).await?;
        let page = document.page()?;
        Ok(PageLockState {
            page: page.clone(),
            page_id: page.id,
            is_mine: page.status == PageStatus::Editing
                && uid.is_some()
                && page.temp_edited_by.as_deref() == uid.as_deref(),
            status: page.status,
            temp_edited_by: page.temp_edited_by,
            sync_mode: page.sync_mode.unwrap_or(false),
            update_time: document.update_time,
        })
    }
    pub async fn lock_page(
        &self,
        entry_id: &str,
        index: u32,
        sync_mode: bool,
    ) -> Result<EditingSession> {
        let uid = self.signed_in_uid().await?;
        let document = self.read_edit_page(entry_id, index).await?;
        let project = self.read_edit_project(&document).await?;
        if project.as_ref().is_some_and(|p| {
            p.fields["blockedUsers"]
                .as_array()
                .is_some_and(|users| users.contains(&json!(uid)))
        }) {
            return Err(Error::Invalid(
                "User is blocked from editing this project".into(),
            ));
        }
        let mut session =
            EditingSession::new(self.clone(), entry_id, index, uid, document, project)?;
        if session.page.status == PageStatus::Editing {
            return Err(Error::Invalid(
                "Transcription is already being edited".into(),
            ));
        }
        let fields = json!({
            "status":"editing", "prevStatus":session.page.status, "tempText":session.page.text,
            "tempTextChanged":false, "tempNotes":session.document.fields.get("notes").filter(|v|!v.is_null()).cloned().unwrap_or(json!([])), "tempEditedBy":session.uid,
            "syncMode":sync_mode
        });
        let mut writes = vec![session.update(fields.clone(), true)];
        session.verify_project(&mut writes);
        let response = self.commit(writes).await?;
        session.apply(fields, &response)?;
        Ok(session)
    }
    /// Resume an existing lock without taking it again or overwriting its draft.
    pub async fn resume_editing(&self, entry_id: &str, index: u32) -> Result<EditingSession> {
        let uid = self.signed_in_uid().await?;
        let document = self.read_edit_page(entry_id, index).await?;
        let session = EditingSession::new(self.clone(), entry_id, index, uid, document, None)?;
        session.check_lock()?;
        Ok(session)
    }
}
impl EditingSession {
    fn new(
        client: HonkokuClient,
        entry_id: &str,
        index: u32,
        uid: String,
        document: ReadDocument,
        project: Option<ReadDocument>,
    ) -> Result<Self> {
        let page = document.page()?;
        if page.entry_id != entry_id || page.index != index {
            return Err(Error::Invalid("page identity mismatch".into()));
        }
        let project_update_time = project
            .as_ref()
            .map(|p| p.update_time.clone())
            .unwrap_or_default();
        Ok(Self {
            client,
            entry_id: entry_id.into(),
            index,
            uid,
            page,
            update_time: document.update_time.clone(),
            project: project.map(|p| ProjectSnapshot {
                name: p.name,
                fields: p.fields,
            }),
            project_update_time,
            document,
            drafts: DraftQueue::default(),
            last_draft: None,
        })
    }
    pub fn page(&self) -> &Page {
        &self.page
    }
    pub fn uid(&self) -> &str {
        &self.uid
    }
    pub fn update_time(&self) -> &str {
        &self.update_time
    }
    pub fn draft_queue(&self) -> DraftQueue {
        self.drafts.clone()
    }
    fn check_lock(&self) -> Result<()> {
        if self.page.status != PageStatus::Editing {
            return Err(Error::Invalid("Transcription is not being edited".into()));
        }
        if self.page.temp_edited_by.as_deref() != Some(&self.uid) {
            return Err(Error::Invalid(
                "Transcription is being edited by another user".into(),
            ));
        }
        Ok(())
    }
    async fn refresh(&mut self) -> Result<()> {
        if self.client.signed_in_uid().await? != self.uid {
            return Err(Error::SignedOut);
        }
        self.document = self
            .client
            .read_edit_page(&self.entry_id, self.index)
            .await?;
        self.page = self.document.page()?;
        self.update_time = self.document.update_time.clone();
        self.check_lock()
    }
    fn update(&self, fields: Value, timestamp: bool) -> Write {
        Write::Update {
            name: self.document.name.clone(),
            update_mask: fields
                .as_object()
                .map(|f| f.keys().cloned().collect())
                .unwrap_or_default(),
            fields,
            update_transforms: if timestamp {
                vec![("updatedAt".into(), ServerValue::RequestTime)]
            } else {
                vec![]
            },
            precondition: Some(Precondition {
                update_time: self.update_time.clone(),
            }),
        }
    }
    fn verify_project(&self, writes: &mut Vec<Write>) {
        if let Some(project) = &self.project {
            writes.push(Write::Verify {
                name: project.name.clone(),
                update_time: self.project_update_time.clone(),
            });
        }
    }
    fn apply(&mut self, fields: Value, response: &CommitResponse) -> Result<()> {
        let result = response
            .write_results
            .first()
            .ok_or_else(|| Error::Invalid("commit omitted page result".into()))?;
        let target = self
            .document
            .fields
            .as_object_mut()
            .ok_or_else(|| Error::Invalid("page fields must be an object".into()))?;
        if let Some(fields) = fields.as_object() {
            target.extend(fields.clone());
        }
        if let Some(timestamp) = result.transform_results.first() {
            target.insert(
                "updatedAt".into(),
                crate::firestore::decode_value(timestamp)?,
            );
        }
        self.document.update_time = result.update_time.clone();
        self.update_time = result.update_time.clone();
        self.page = self.document.page()?;
        Ok(())
    }
    pub async fn draft_notes(&mut self, notes: &[Value]) -> Result<()> {
        self.change_notes(NoteChange::Replace(stored_notes(notes)?))
            .await
    }
    pub async fn delete_note(&mut self, index: usize) -> Result<()> {
        self.change_notes(NoteChange::Delete(index)).await
    }
    async fn change_notes(&mut self, change: NoteChange) -> Result<()> {
        for attempt in 0..5 {
            self.refresh().await?;
            let notes = match &change {
                NoteChange::Replace(notes) => notes.clone(),
                NoteChange::Delete(index) => {
                    let mut notes = self.document.fields["tempNotes"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    if let Some(note) = notes.get_mut(*index) {
                        *note = Value::Null;
                    }
                    json!(notes)
                }
            };
            let fields = json!({"tempNotes":notes});
            match self
                .client
                .commit(vec![self.update(fields.clone(), false)])
                .await
            {
                Ok(response) => return self.apply(fields, &response),
                Err(Error::Conflict { .. }) if attempt < 4 => continue,
                Err(error) => return Err(error),
            }
        }
        Err(Error::Invalid("note transaction retries exhausted".into()))
    }
    pub async fn draft(&mut self, text: &str) -> Result<()> {
        self.drafts.request(text)?;
        self.flush_drafts(false).await
    }
    pub async fn draft_now(&mut self, text: &str) -> Result<()> {
        self.drafts.request(text)?;
        self.flush_drafts(true).await
    }
    /// Drain the newest pending text, with a single request in flight per session.
    pub async fn flush_drafts(&mut self, immediate: bool) -> Result<()> {
        self.check_lock()?;
        while self.drafts.has_text()? {
            if !immediate && let Some(last) = self.last_draft {
                sleep_until(last + Duration::from_secs(3)).await;
            }
            self.refresh().await?;
            let Some(text) = self.drafts.latest()? else {
                break;
            };
            let notes = match &text.notes {
                Some(notes) => stored_notes(notes)?,
                None => self
                    .document
                    .fields
                    .get("tempNotes")
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or(json!([])),
            };
            let fields = json!({"tempText":text.text,"tempTextChanged":true,"tempNotes":notes});
            self.last_draft = Some(Instant::now());
            match self
                .client
                .commit(vec![self.update(fields.clone(), true)])
                .await
            {
                Ok(response) => {
                    self.apply(fields, &response)?;
                    self.drafts.acknowledge(&text)?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    pub async fn save(&mut self, options: SaveOptions) -> Result<SavedPage> {
        let status = options.status.unwrap_or(PageStatus::Initiated);
        if !matches!(status, PageStatus::Initiated | PageStatus::Completed) {
            return Err(Error::Invalid(
                "saved status must be initiated or completed".into(),
            ));
        }
        let mut freeze = self.drafts.freeze()?;
        self.flush_drafts(true).await?;
        self.refresh().await?;
        let project = self
            .client
            .read_edit_project(&self.document)
            .await?
            .ok_or_else(|| Error::Invalid("Project not found".into()))?;
        self.project_update_time = project.update_time;
        self.project = Some(ProjectSnapshot {
            name: project.name,
            fields: project.fields.clone(),
        });
        let text = self.page.temp_text.as_deref().unwrap_or_default();
        let mut fields = json!({"status":status,"text":text,"notes":self.document.fields.get("tempNotes").filter(|v|!v.is_null()).cloned().unwrap_or(json!([])),"editedBy":self.uid,"share":options.share,"requestReview":options.request_review,"syncMode":false});
        if let Some(approval) = options.is_approval {
            let mut approvers = self.page.approved_by.clone().unwrap_or_default();
            if approval && !approvers.contains(&self.uid) {
                if approvers.len() >= 2 {
                    return Err(Error::Invalid(
                        "Transcription cannot have more than two approvers".into(),
                    ));
                }
                approvers.push(self.uid.clone());
            } else if !approval {
                approvers.retain(|uid| uid != &self.uid);
            }
            fields["approvedBy"] = json!(approvers);
        }
        let mut data = self.document.fields.clone();
        data["text"] = json!(text);
        data["editedBy"] = json!(self.uid);
        data["status"] = json!(status);
        let event_id = auto_id();
        let event = json!({
            "uid":self.uid,"projectId":self.document.fields["projectId"],"entryId":self.entry_id,
            "transcriptionId":self.page.id,"index":self.index,"eventType":"transcription","data":data,
            "members":project.fields["members"],"projectType":project.fields["projectType"],
            "count":added_character_count(&self.page.text,text),
            "isReview":self.page.request_review == Some(true) && self.page.edited_by.as_deref() != Some(&self.uid),
            "status":status,"share":options.share,"requestReview":options.request_review,
            "comment":options.comment,"isApproval":options.is_approval.unwrap_or(false)
        });
        let mut writes = vec![
            self.update(fields.clone(), false),
            Write::Set {
                name: self
                    .client
                    .document_name(&format!("timelineEvents/{event_id}"))?,
                fields: event,
                update_transforms: vec![("createdAt".into(), ServerValue::RequestTime)],
                precondition: None,
            },
        ];
        self.verify_project(&mut writes);
        let response = self.client.commit(writes).await?;
        self.apply(fields, &response)?;
        self.drafts.close()?;
        freeze.complete = true;
        Ok(SavedPage {
            page: self.page.clone(),
            timeline_event_id: event_id,
        })
    }
    pub async fn discard(mut self) -> Result<()> {
        let mut freeze = self.drafts.freeze()?;
        self.refresh().await?;
        let previous = self
            .page
            .prev_status
            .clone()
            .ok_or_else(|| Error::Invalid("previous page status missing".into()))?;
        let fields = json!({"status":previous});
        let response = self
            .client
            .commit(vec![self.update(fields.clone(), false)])
            .await?;
        self.apply(fields, &response)?;
        self.drafts.close()?;
        freeze.complete = true;
        Ok(())
    }
}
/// Restore Firestore timestamps after the plain JSON IPC round trip.
fn stored_notes(notes: &[Value]) -> Result<Value> {
    let mut notes = notes.to_vec();
    for note in &mut notes {
        if let Some(fields) = note.as_object_mut() {
            for key in ["createdAt", "updatedAt"] {
                if let Some(Value::String(date)) = fields.get(key) {
                    let timestamp: crate::model::Timestamp = serde_json::from_value(json!(date))?;
                    fields.insert(key.into(), json!({"$firestoreTimestamp": timestamp}));
                }
            }
        }
    }
    Ok(json!(notes))
}
/// jsdiff operates on UTF-16 units; supplementary-plane characters may count differently.
pub fn added_character_count(old: &str, new: &str) -> usize {
    let old: Vec<_> = old.chars().collect();
    let new: Vec<_> = new.chars().collect();
    similar::capture_diff_slices(similar::Algorithm::Myers, &old, &new)
        .iter()
        .map(|op| match op {
            similar::DiffOp::Insert {
                new_index, new_len, ..
            }
            | similar::DiffOp::Replace {
                new_index, new_len, ..
            } => new[*new_index..new_index + new_len]
                .iter()
                .filter(|ch| !js_whitespace(**ch))
                .count(),
            _ => 0,
        })
        .sum()
}
fn js_whitespace(ch: char) -> bool {
    matches!(ch,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
#[cfg(test)]
mod tests;
