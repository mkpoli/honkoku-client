use crate::{AppError, AppState};
use futures_util::{StreamExt, TryStreamExt, stream};
use honkoku_core::{
    auth::{Session, SessionStore, TokenManager, import_dev_session},
    home::{RankingSelf, RankingSort, TimelineFilter as CoreTimelineFilter},
    model::{Announcement, DailyProgress, Label, TimelineEvent, TimelineItem, User},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{Manager, State};

#[derive(Clone, Serialize)]
pub struct SessionInfo {
    uid: String,
    display_name: Option<String>,
    providers: Vec<String>,
    credential_store: honkoku_core::auth::CredentialStore,
}
impl SessionInfo {
    pub(crate) fn new(
        value: &Session,
        credential_store: honkoku_core::auth::CredentialStore,
    ) -> Self {
        Self {
            uid: value.uid.clone(),
            display_name: value.display_name.clone(),
            providers: value.providers.clone(),
            credential_store,
        }
    }
}
#[tauri::command]
pub async fn session_import(
    signin: State<'_, crate::signin::SignInState>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<SessionInfo, AppError> {
    let path = app
        .path()
        .home_dir()
        .map_err(|_| AppError {
            kind: "io".into(),
            message: "ホームフォルダーを取得できません。".into(),
        })?
        .join(".local/share/honkoku-client/session.json");
    let _gate = signin.gate.lock().await;
    let session = import_dev_session(path)?;
    attach_session(&state, &editing, session).await
}
pub(crate) async fn attach_session(
    state: &AppState,
    editing: &EditingState,
    session: Session,
) -> Result<SessionInfo, AppError> {
    attach_session_with(state, editing, session, |session| {
        Ok(state.store.save(session)?)
    })
    .await
}
pub(crate) async fn attach_session_with(
    state: &AppState,
    editing: &EditingState,
    session: Session,
    persist: impl FnOnce(&Session) -> Result<(), AppError>,
) -> Result<SessionInfo, AppError> {
    let mut connection = state.connection.write().await;
    let info = SessionInfo::new(&session, state.store.kind());
    let client = state
        .anonymous()?
        .with_session(TokenManager::new(session.clone(), state.store.clone())?);
    persist(&session)?;
    connection.client = client;
    connection.session = Some(info.clone());
    editing.pages.lock().await.clear();
    Ok(info)
}
#[tauri::command]
pub async fn session_current(
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<Option<SessionInfo>, AppError> {
    let mut connection = state.connection.write().await;
    if connection.session.is_some() {
        match connection.client.signed_in_uid().await {
            Ok(_) => {}
            Err(honkoku_core::Error::SignedOut) => {
                connection.client = state.anonymous()?;
                connection.session = None;
                editing.pages.lock().await.clear();
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(connection.session.clone())
}
#[tauri::command]
pub async fn session_clear(
    app: tauri::AppHandle,
    signin: State<'_, crate::signin::SignInState>,
    clear_site_data: Option<bool>,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<(), AppError> {
    let _gate = signin.gate.lock().await;
    let mut connection = state.connection.write().await;
    let client = state.anonymous()?;
    state.store.clear()?;
    connection.client = client;
    connection.session = None;
    editing.pages.lock().await.clear();
    crate::signin::cancel(&app, &signin)?;
    if clear_site_data.unwrap_or(false) {
        crate::signin::clear_profile(&app).await?;
    }
    Ok(())
}
#[derive(Default, Deserialize)]
pub struct TimelineFilter {
    project_id: Option<String>,
    joined: Option<bool>,
    before: Option<String>,
    before_id: Option<String>,
}
fn equal(field: &str, value: &str) -> Value {
    json!({"fieldFilter":{"field":{"fieldPath":field},"op":"EQUAL","value":{"stringValue":value}}})
}
fn timeline_query(
    projects: Option<&[String]>,
    kind: &str,
    filter: &TimelineFilter,
    limit: u32,
) -> Value {
    let mut filters = vec![
        equal("eventType", "transcription"),
        equal("projectType", kind),
        json!({"fieldFilter":{"field":{"fieldPath":"share"},"op":"EQUAL","value":{"booleanValue":true}}}),
    ];
    if let Some(ids) = projects {
        filters.push(json!({"fieldFilter":{"field":{"fieldPath":"projectId"},"op":"IN","value":{"arrayValue":{"values":ids.iter().map(|id|json!({"stringValue":id})).collect::<Vec<_>>()}}}}));
    }
    let mut query = json!({"from":[{"collectionId":"timelineEvents"}],"where":{"compositeFilter":{"op":"AND","filters":filters}},"orderBy":[{"field":{"fieldPath":"createdAt"},"direction":"DESCENDING"},{"field":{"fieldPath":"__name__"},"direction":"DESCENDING"}],"limit":limit});
    if let Some(before) = &filter.before {
        let mut values = vec![json!({"timestampValue":before})];
        if let Some(id) = &filter.before_id {
            values.push(json!({"referenceValue":format!("projects/honkoku3-c466c/databases/(default)/documents/timelineEvents/{id}")}));
        }
        query["startAt"] = json!({"values":values,"before":false});
    }
    query
}
#[tauri::command]
pub async fn home_timeline(
    filter: TimelineFilter,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<TimelineItem>, AppError> {
    let connection = state.connection.read().await;
    let client = &connection.client;
    let limit = limit.unwrap_or(20).min(100);
    if limit == 0 {
        return Ok(vec![]);
    }
    let ids = if filter.joined.unwrap_or(false) {
        let uid = client.signed_in_uid().await?;
        let projects = client.cached_projects(&state.storage, false).await?;
        Some(
            projects
                .into_iter()
                .filter(|p| {
                    p.members.as_ref().is_some_and(|ids| ids.contains(&uid))
                        || p.admins.as_ref().is_some_and(|ids| ids.contains(&uid))
                })
                .map(|p| p.id)
                .collect::<Vec<_>>(),
        )
    } else {
        filter.project_id.clone().map(|id| vec![id])
    };
    if filter.before.is_none() {
        let core_filter = match (&ids, filter.joined.unwrap_or(false)) {
            (Some(ids), true) => CoreTimelineFilter::Joined(ids.clone()),
            (Some(ids), false) => CoreTimelineFilter::Project(ids[0].clone()),
            (None, _) => CoreTimelineFilter::All,
        };
        match client.timeline(core_filter, Some(limit)).await {
            Ok(items) => return Ok(items),
            // Older canvases use @id; the fallback only reads enrichment labels.
            Err(honkoku_core::Error::Json(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }
    let chunks: Vec<_> = match &ids {
        Some(ids) => ids.chunks(30).map(Some).collect(),
        None => vec![None],
    };
    let queries: Vec<_> = chunks
        .into_iter()
        .flat_map(|chunk| {
            ["official", "user"].map(|kind| timeline_query(chunk, kind, &filter, limit))
        })
        .collect();
    let batches: Vec<Vec<TimelineEvent>> = stream::iter(queries)
        .map(|q| client.run_query(q))
        .buffered(4)
        .try_collect()
        .await?;
    let mut events: Vec<_> = batches.into_iter().flatten().collect();
    events.sort_by(|a, b| {
        b.created_at
            .0
            .cmp(&a.created_at.0)
            .then_with(|| b.id.cmp(&a.id))
    });
    events.dedup_by(|a, b| a.id == b.id);
    events.truncate(limit as usize);
    Ok(stream::iter(events)
        .map(|event| async move {
            async fn optional<T: serde::de::DeserializeOwned>(
                client: &honkoku_core::HonkokuClient,
                path: &str,
            ) -> honkoku_core::Result<Option<T>> {
                match client.document(path).await {
                    Ok(value) => Ok(Some(value)),
                    Err(honkoku_core::Error::Http(e))
                        if e.status() == Some(reqwest::StatusCode::NOT_FOUND) =>
                    {
                        Ok(None)
                    }
                    Err(e) => Err(e),
                }
            }
            let actor = optional::<Value>(client, &format!("users/{}", event.uid))
                .await?
                .map(|mut v| {
                    v["uid"] = json!(event.uid);
                    serde_json::from_value::<User>(v)
                })
                .transpose()?;
            let entry = optional::<Value>(client, &format!("entries/{}", event.entry_id)).await?;
            let project =
                optional::<Value>(client, &format!("projects/{}", event.project_id)).await?;
            let entry_label = entry
                .and_then(|v| v.get("label").cloned())
                .map(serde_json::from_value::<Label>)
                .transpose()?;
            let project_title = project.and_then(|v| v["title"].as_str().map(str::to_owned));
            let excerpt =
                honkoku_text::excerpt(event.data["text"].as_str().unwrap_or_default(), 120);
            Ok::<_, honkoku_core::Error>(TimelineItem {
                event,
                actor,
                entry_label,
                project_title,
                excerpt,
            })
        })
        .buffered(4)
        .try_collect()
        .await?)
}
#[derive(Deserialize)]
pub enum Sort {
    #[serde(rename = "exp")]
    Exp,
    #[serde(rename = "charCount")]
    CharCount,
    #[serde(rename = "likeCount")]
    LikeCount,
}
impl From<Sort> for RankingSort {
    fn from(sort: Sort) -> Self {
        match sort {
            Sort::Exp => Self::Exp,
            Sort::CharCount => Self::CharCount,
            Sort::LikeCount => Self::LikeCount,
        }
    }
}
#[tauri::command]
pub async fn home_ranking_self(
    sort: Sort,
    state: State<'_, AppState>,
) -> Result<RankingSelf, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .ranking_self(sort.into())
        .await?)
}
#[tauri::command]
pub async fn home_ranking(
    sort: Sort,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<User>, AppError> {
    let sort = sort.into();
    Ok(state
        .connection
        .read()
        .await
        .client
        .ranking(sort, Some(limit.unwrap_or(100).min(100)))
        .await?)
}
#[tauri::command]
pub async fn home_announcements(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<Announcement>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .announcements(Some(limit.unwrap_or(5).min(100)))
        .await?)
}
#[tauri::command]
pub async fn home_daily_progress(
    state: State<'_, AppState>,
) -> Result<Vec<DailyProgress>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .daily_progress(None)
        .await?)
}
#[tauri::command]
pub async fn me(state: State<'_, AppState>) -> Result<Option<User>, AppError> {
    match state.connection.read().await.client.me().await {
        Ok(user) => Ok(Some(user)),
        Err(honkoku_core::Error::SignedOut) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
#[tauri::command]
pub async fn unread_notification_count(state: State<'_, AppState>) -> Result<u64, AppError> {
    match state
        .connection
        .read()
        .await
        .client
        .unread_notification_count()
        .await
    {
        Ok(count) => Ok(count),
        Err(honkoku_core::Error::SignedOut) => Ok(0),
        Err(error) => Err(error.into()),
    }
}
#[tauri::command]
pub async fn get_user(uid: String, state: State<'_, AppState>) -> Result<User, AppError> {
    Ok(state.connection.read().await.client.user(&uid).await?)
}

#[tauri::command]
pub async fn page_history(
    entry_id: String,
    index: u32,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<TimelineItem>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .page_history(&entry_id, index, limit)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_keeps_timestamp_and_document_tie_breaker() {
        let filter = TimelineFilter {
            before: Some("2026-09-10T01:00:00Z".into()),
            before_id: Some("event-2".into()),
            ..Default::default()
        };
        let q = timeline_query(Some(&["ainu".into()]), "user", &filter, 20);
        assert_eq!(q["startAt"]["before"], false);
        assert_eq!(
            q["startAt"]["values"][0]["timestampValue"],
            "2026-09-10T01:00:00Z"
        );
        assert!(
            q["startAt"]["values"][1]["referenceValue"]
                .as_str()
                .unwrap()
                .ends_with("/event-2")
        );
        assert_eq!(
            q["where"]["compositeFilter"]["filters"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(q["limit"], 20);
    }
}

use honkoku_core::{
    editing::{DraftQueue, EditingSession, PageLockState, SaveOptions, SavedPage as CoreSavedPage},
    model::Page,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPage {
    page: Page,
    timeline_event_id: String,
    count: u64,
}
struct LiveEditingSession {
    session: Mutex<Option<EditingSession>>,
    saved: Mutex<Option<CoreSavedPage>>,
    drafts: DraftQueue,
    uid: String,
}
#[derive(Default)]
pub struct EditingState {
    pages: Mutex<HashMap<String, Arc<LiveEditingSession>>>,
}
impl EditingState {
    async fn remove(&self, key: &str, live: &Arc<LiveEditingSession>) {
        let mut pages = self.pages.lock().await;
        if pages
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, live))
        {
            pages.remove(key);
        }
    }
    async fn session(
        &self,
        client: &honkoku_core::HonkokuClient,
        entry_id: &str,
        index: u32,
    ) -> Result<Arc<LiveEditingSession>, AppError> {
        let uid = client.signed_in_uid().await?;
        let key = format!("{entry_id}_{index}");
        let mut pages = self.pages.lock().await;
        if let Some(live) = pages.get(&key)
            && live.uid == uid
        {
            return Ok(live.clone());
        }
        let session = client.resume_editing(entry_id, index).await?;
        let live = Arc::new(LiveEditingSession {
            drafts: session.draft_queue(),
            uid,
            session: Mutex::new(Some(session)),
            saved: Mutex::new(None),
        });
        pages.insert(key, live.clone());
        Ok(live)
    }
}
fn no_editing_session() -> AppError {
    honkoku_core::Error::Invalid("Transcription is not being edited".into()).into()
}
#[tauri::command]
pub async fn page_lock(
    entry_id: String,
    index: u32,
    sync_mode: Option<bool>,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<Page, AppError> {
    let connection = state.connection.read().await;
    let mut pages = editing.pages.lock().await;
    let session = connection
        .client
        .lock_page(&entry_id, index, sync_mode.unwrap_or(false))
        .await?;
    let page = session.page().clone();
    pages.insert(
        page.id.clone(),
        Arc::new(LiveEditingSession {
            drafts: session.draft_queue(),
            uid: session.uid().into(),
            session: Mutex::new(Some(session)),
            saved: Mutex::new(None),
        }),
    );
    crate::search::cache_page(&state, &page).await?;
    Ok(page)
}
#[tauri::command]
pub async fn page_draft(
    entry_id: String,
    index: u32,
    text: String,
    notes: Option<Vec<Value>>,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<Page, AppError> {
    let connection = state.connection.read().await;
    let live = editing
        .session(&connection.client, &entry_id, index)
        .await?;
    live.drafts.request_with_notes(&text, notes)?;
    let mut guard = live.session.lock().await;
    let session = guard.as_mut().ok_or_else(no_editing_session)?;
    session.flush_drafts(false).await?;
    Ok(session.page().clone())
}
#[tauri::command]
pub async fn page_save(
    entry_id: String,
    index: u32,
    options: SaveOptions,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<SavedPage, AppError> {
    let connection = state.connection.read().await;
    let live = editing
        .session(&connection.client, &entry_id, index)
        .await?;
    let result = live.save(&connection.client, options).await?;
    crate::search::cache_page(&state, &result.page).await?;
    editing.remove(&result.page.id, &live).await;
    Ok(result)
}
impl LiveEditingSession {
    async fn save(
        &self,
        client: &honkoku_core::HonkokuClient,
        options: SaveOptions,
    ) -> Result<SavedPage, AppError> {
        let mut guard = self.session.lock().await;
        let mut receipt = self.saved.lock().await;
        if receipt.is_none() {
            let saved = guard
                .as_mut()
                .ok_or_else(no_editing_session)?
                .save(options)
                .await?;
            *receipt = Some(saved);
            *guard = None;
        }
        // Keep the committed receipt until its event can be read. Retrying must not write again.
        let saved = receipt.as_ref().ok_or_else(no_editing_session)?;
        let project_id = saved
            .page
            .extra
            .get("projectId")
            .and_then(Value::as_str)
            .ok_or_else(|| honkoku_core::Error::Invalid("saved page has no projectId".into()))?;
        client.history_saved(&saved.page, project_id).await?;
        saved_with_count(client, saved).await
    }
}

async fn saved_with_count(
    client: &honkoku_core::HonkokuClient,
    saved: &CoreSavedPage,
) -> Result<SavedPage, AppError> {
    #[derive(Deserialize)]
    struct SavedEvent {
        count: u64,
    }
    let event: SavedEvent = client
        .document(&format!("timelineEvents/{}", saved.timeline_event_id))
        .await
        .map_err(|_| AppError {
            kind: "saved_count".into(),
            message: "翻刻は保存されましたが、文字数を取得できませんでした。もう一度保存を押すと、保存済みの結果を取得します。".into(),
        })?;
    Ok(SavedPage {
        page: saved.page.clone(),
        timeline_event_id: saved.timeline_event_id.clone(),
        count: event.count,
    })
}
#[tauri::command]
pub async fn page_discard(
    entry_id: String,
    index: u32,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<(), AppError> {
    let connection = state.connection.read().await;
    let live = editing
        .session(&connection.client, &entry_id, index)
        .await?;
    let mut guard = live.session.lock().await;
    let session = guard.take().ok_or_else(no_editing_session)?;
    let result = session.discard().await;
    editing.remove(&format!("{entry_id}_{index}"), &live).await;
    Ok(result?)
}
#[tauri::command]
pub async fn page_lock_state(
    entry_id: String,
    index: u32,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<PageLockState, AppError> {
    let connection = state.connection.read().await;
    let lock = connection.client.page_lock_state(&entry_id, index).await?;
    crate::search::cache_page(&state, &lock.page).await?;
    if lock.is_mine {
        editing
            .session(&connection.client, &entry_id, index)
            .await?;
    } else {
        editing.pages.lock().await.remove(&lock.page_id);
    }
    Ok(lock)
}

#[cfg(test)]
mod saved_count_tests {
    use super::*;
    use std::io::{Read, Write};

    #[tokio::test]
    async fn retry_after_event_read_failure_uses_the_committed_receipt() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (status, body) in [
                ("503 Service Unavailable", "{}"),
                (
                    "200 OK",
                    r#"{"name":"projects/test/databases/(default)/documents/timelineEvents/event","fields":{"count":{"integerValue":"123"}}}"#,
                ),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0; 4096];
                let bytes = socket.read(&mut request).unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..bytes])
                        .starts_with("GET /timelineEvents/event ")
                );
                write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let client = honkoku_core::HonkokuClient::with_endpoints(&base, &base).unwrap();
        let saved = CoreSavedPage {
            page: serde_json::from_value(json!({"id":"entry_0", "entryId":"entry", "projectId":"project", "index":0, "status":"completed", "text":"字", "notes":[]})).unwrap(),
            timeline_event_id: "event".into(),
        };
        let live = LiveEditingSession {
            session: Mutex::new(None),
            saved: Mutex::new(Some(saved)),
            drafts: DraftQueue::default(),
            uid: "user".into(),
        };
        assert_eq!(
            live.save(&client, SaveOptions::default())
                .await
                .err()
                .unwrap()
                .kind,
            "saved_count"
        );
        assert!(live.saved.lock().await.is_some());
        let history = client.history_recent(8).await.unwrap();
        assert_eq!(history[0].record.status_after, "completed");
        assert!(history[0].record.saved_at.is_some());
        let result = live.save(&client, SaveOptions::default()).await.unwrap();
        assert_eq!(result.count, 123);
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["timelineEventId"], "event");
        assert_eq!(value["count"], 123);
        assert_eq!(value["page"]["text"], "字");
        server.join().unwrap();
    }
}

#[tauri::command]
pub async fn list_entry_summaries(
    collection_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<honkoku_core::model::EntrySummary>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .list_entries(&collection_id)
        .await?)
}
#[tauri::command]
pub async fn collection_progress(
    collection_id: String,
    refresh: Option<bool>,
    state: State<'_, AppState>,
) -> Result<honkoku_core::model::CollectionProgress, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .collection_progress(&collection_id, refresh.unwrap_or(false))
        .await?)
}
#[tauri::command]
pub async fn entry_progress(
    entry_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<honkoku_core::model::EntryProgress>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .entry_progress(&entry_ids)
        .await?)
}

#[tauri::command]
pub async fn page_draft_notes(
    entry_id: String,
    index: u32,
    notes: Vec<Value>,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<Page, AppError> {
    let connection = state.connection.read().await;
    let live = editing
        .session(&connection.client, &entry_id, index)
        .await?;
    let mut guard = live.session.lock().await;
    let session = guard.as_mut().ok_or_else(no_editing_session)?;
    session.draft_notes(&notes).await?;
    Ok(session.page().clone())
}
#[tauri::command]
pub async fn history_open(
    entry_id: String,
    index: u32,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .history_open(&entry_id, index)
        .await?)
}
#[tauri::command]
pub async fn history_recent(
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<honkoku_core::history::RecentWork>, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .history_recent(limit)
        .await?)
}
#[tauri::command]
pub async fn history_clear(state: State<'_, AppState>) -> Result<(), AppError> {
    Ok(state.connection.read().await.client.history_clear().await?)
}

#[tauri::command]
pub async fn editing_pages(
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<Vec<Page>, AppError> {
    let connection = state.connection.read().await;
    let uid = connection.client.signed_in_uid().await?;
    let sessions: Vec<_> = editing
        .pages
        .lock()
        .await
        .values()
        .filter(|live| live.uid == uid)
        .cloned()
        .collect();
    let mut pages = Vec::new();
    for live in sessions {
        if let Some(session) = live.session.lock().await.as_ref() {
            pages.push(session.page().clone());
        }
    }
    pages.sort_by(|a, b| a.entry_id.cmp(&b.entry_id).then(a.index.cmp(&b.index)));
    Ok(pages)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageActivity {
    entry_id: String,
    updated_at: Option<honkoku_core::model::Timestamp>,
}
#[tauri::command]
pub async fn project_page_activity(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<HashMap<String, honkoku_core::model::Timestamp>, AppError> {
    let connection = state.connection.read().await;
    let rows: Vec<PageActivity> = connection
        .client
        .run_query(json!({
            "from": [{"collectionId": "transcriptions"}],
            "select": {"fields": [{"fieldPath": "entryId"}, {"fieldPath": "updatedAt"}]},
            "where": equal("projectId", &project_id)
        }))
        .await?;
    let mut latest = HashMap::<String, honkoku_core::model::Timestamp>::new();
    for row in rows {
        if let Some(updated_at) = row.updated_at {
            latest
                .entry(row.entry_id)
                .and_modify(|current| {
                    if updated_at.0 > current.0 {
                        *current = updated_at.clone();
                    }
                })
                .or_insert(updated_at);
        }
    }
    Ok(latest)
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReadRegion {
    Projects,
    Project { id: String },
    Collections { id: String },
    Collection { id: String },
    Entry { id: String },
    Pages { id: String },
    Entries { id: String },
    CollectionProgress { id: String },
    EntryProgress { ids: Vec<String> },
}
#[tauri::command]
pub async fn region_cached(
    resource: ReadRegion,
    state: State<'_, AppState>,
) -> Result<Option<Value>, AppError> {
    Ok(honkoku_core::cache::blocking(&state.storage, move |db| {
        let read = |key: &str| db.fresh_response::<Value>(key, std::time::Duration::MAX);
        let key = match resource {
            ReadRegion::Projects => "projects".into(),
            ReadRegion::Project { id } => format!("project/{id}"),
            ReadRegion::Collection { id } => format!("collection/{id}"),
            ReadRegion::Entry { id } => format!("entry/{id}"),
            ReadRegion::Pages { id } => format!("pages/{id}"),
            ReadRegion::Entries { id } => format!("entry-summaries/{id}"),
            ReadRegion::CollectionProgress { id } => format!("collection-progress/{id}"),
            ReadRegion::Collections { id } => {
                let Some(project) = read(&format!("project/{id}"))? else {
                    return Ok(None);
                };
                let Some(ids) = project["collections"].as_array() else {
                    return Ok(None);
                };
                let mut rows = Vec::new();
                for id in ids.iter().filter_map(Value::as_str) {
                    if let Some(row) = read(&format!("collection/{id}"))? {
                        rows.push(row);
                    }
                }
                return Ok((!rows.is_empty() || ids.is_empty()).then_some(Value::Array(rows)));
            }
            ReadRegion::EntryProgress { ids } => {
                let mut rows = Vec::new();
                for id in &ids {
                    if let Some(row) = read(&format!("entry-progress/{id}"))? {
                        rows.push(row);
                    }
                }
                return Ok((!rows.is_empty() || ids.is_empty()).then_some(Value::Array(rows)));
            }
        };
        read(&key)
    })
    .await?)
}
#[tauri::command]
pub async fn region_refresh(
    resource: ReadRegion,
    state: State<'_, AppState>,
) -> Result<Value, AppError> {
    let connection = state.connection.read().await;
    let client = &connection.client;
    let value = match resource {
        ReadRegion::Projects => {
            serde_json::to_value(client.cached_projects(&state.storage, true).await?)
        }
        ReadRegion::Project { id } => {
            serde_json::to_value(client.cached_project(&state.storage, &id, true).await?)
        }
        ReadRegion::Collections { id } => {
            serde_json::to_value(client.cached_collections(&state.storage, &id, true).await?)
        }
        ReadRegion::Collection { id } => {
            serde_json::to_value(client.cached_collection(&state.storage, &id, true).await?)
        }
        ReadRegion::Entry { id } => match client.cached_entry(&state.storage, &id, true).await {
            Ok(entry) => serde_json::to_value(entry),
            Err(honkoku_core::Error::Json(_)) => {
                let mut value: Value = client.document(&format!("entries/{id}")).await?;
                if let Some(canvases) = value["canvases"].as_array_mut() {
                    canvases.iter_mut().for_each(crate::normalize_canvas);
                }
                let copy = value.clone();
                honkoku_core::cache::blocking(&state.storage, move |db| db.put_entry(&copy))
                    .await?;
                Ok(value)
            }
            Err(error) => return Err(error.into()),
        },
        ReadRegion::Pages { id } => {
            serde_json::to_value(client.cached_pages(&state.storage, &id, true).await?)
        }
        ReadRegion::Entries { id } => {
            serde_json::to_value(client.list_entries_with_refresh(&id, true).await?)
        }
        ReadRegion::CollectionProgress { id } => {
            serde_json::to_value(client.collection_progress(&id, true).await?)
        }
        ReadRegion::EntryProgress { ids } => {
            serde_json::to_value(client.entry_progress_with_refresh(&ids, true).await?)
        }
    };
    Ok(value.map_err(honkoku_core::Error::from)?)
}

#[tauri::command]
pub async fn glyph_attestations(
    character: String,
    project: Option<String>,
    limit: usize,
    state: State<'_, AppState>,
    search: State<'_, Arc<crate::search::SearchState>>,
) -> Result<honkoku_core::glyphs::Attestations, AppError> {
    if !(1..=200).contains(&limit) || character.is_empty() {
        return Err(honkoku_core::Error::Invalid("invalid glyph query".into()).into());
    }
    let hits = crate::search::search_query(
        honkoku_search::Query {
            text: character.clone(),
            mode: honkoku_search::Mode::Strict,
            project,
            entry: None,
            limit,
            cursor: None,
        },
        search,
    )
    .await?;
    Ok(state
        .connection
        .read()
        .await
        .client
        .glyph_attestations(&character, hits)
        .await?)
}
#[tauri::command]
pub async fn clips_list(
    state: State<'_, AppState>,
) -> Result<Vec<honkoku_core::clips::Clip>, AppError> {
    Ok(state.connection.read().await.client.clips().await?)
}
#[tauri::command]
pub async fn clip_create(
    input: honkoku_core::clips::ClipInput,
    state: State<'_, AppState>,
) -> Result<honkoku_core::clips::Clip, AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .create_clip(input)
        .await?)
}
#[tauri::command]
pub async fn clip_delete(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    Ok(state
        .connection
        .read()
        .await
        .client
        .delete_clip(&id)
        .await?)
}

fn glyph_service_url(info: &Value, info_url: &str, requested: &str) -> Result<String, AppError> {
    use honkoku_iiif::{ImageService, ImageVersion};
    let invalid = || AppError {
        kind: "iiif".into(),
        message: "切り抜き画像のURLを確認できません。".into(),
    };
    let base = info_url.strip_suffix("/info.json").ok_or_else(invalid)?;
    let tail = requested
        .strip_prefix(&format!("{base}/"))
        .ok_or_else(invalid)?;
    let parts: Vec<_> = tail.split('/').collect();
    if parts.len() != 4 || parts[2] != "0" || parts[3] != "default.jpg" {
        return Err(invalid());
    }
    let region: Vec<u32> = parts[0]
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| invalid())?;
    if region.len() != 4 || region[2] == 0 || region[3] == 0 {
        return Err(invalid());
    }
    let requested_side: u32 = parts[1].trim_matches(',').parse().map_err(|_| invalid())?;
    let hint = format!("{} {} {}", info["@context"], info["profile"], info["type"]);
    let version = if hint.contains("/image/3/") || hint.contains("ImageService3") {
        ImageVersion::V3
    } else if hint.contains("/image/1/")
        || hint.contains("/image-api/1.")
        || hint.contains("ImageService1")
    {
        ImageVersion::V1
    } else {
        ImageVersion::V2
    };
    let size = if version == ImageVersion::V3
        && requested_side
            > if parts[1].starts_with(',') {
                region[3]
            } else {
                region[2]
            } {
        format!("^{}", parts[1])
    } else {
        parts[1].into()
    };
    let service = ImageService {
        id: info["id"]
            .as_str()
            .or_else(|| info["@id"].as_str())
            .unwrap_or(base)
            .into(),
        version,
        profile: info["profile"].clone(),
    };
    Ok(service.url(parts[0], &size, "0", "default", "jpg"))
}
#[tauri::command]
pub async fn glyph_image_url(
    info_url: String,
    url: String,
    fetcher: State<'_, honkoku_iiif::Fetcher>,
) -> Result<String, AppError> {
    let error = |_| AppError {
        kind: "iiif".into(),
        message: "切り抜き画像の情報を取得できません。".into(),
    };
    fetcher.allow_url(&info_url).map_err(error)?;
    let cached = fetcher.get(&info_url).await.map_err(error)?;
    let bytes = cached.read().await.map_err(error)?;
    let info: Value = serde_json::from_slice(&bytes).map_err(|_| AppError {
        kind: "iiif".into(),
        message: "原本の画像情報を読み取れません。".into(),
    })?;
    let upstream = glyph_service_url(&info, &info_url, &url)?;
    fetcher.allow_url(&upstream).map_err(error)?;
    Ok(honkoku_iiif::local_url(
        &upstream,
        cfg!(target_os = "windows"),
    ))
}
#[cfg(test)]
mod glyph_image_tests {
    use super::*;
    #[test]
    fn image_versions_keep_regions_quality_and_upscaling() {
        let info = "https://example.org/image/?IIIF=/a%2Fb.tif/info.json";
        let crop = "https://example.org/image/?IIIF=/a%2Fb.tif/1,2,30,40/,192/0/default.jpg";
        assert!(
            glyph_service_url(
                &json!({"@context":"http://library.stanford.edu/iiif/image-api/1.1/context.json"}),
                info,
                crop
            )
            .unwrap()
            .ends_with("/,192/0/native.jpg")
        );
        assert!(
            glyph_service_url(
                &json!({"@context":"http://iiif.io/api/image/3/context.json"}),
                info,
                crop
            )
            .unwrap()
            .ends_with("/^,192/0/default.jpg")
        );
        assert_eq!(
            glyph_service_url(
                &json!({"@context":"http://iiif.io/api/image/2/context.json"}),
                info,
                crop
            )
            .unwrap(),
            crop
        );
    }
}

#[tauri::command]
pub async fn page_note_delete(
    entry_id: String,
    index: u32,
    note_index: usize,
    state: State<'_, AppState>,
    editing: State<'_, EditingState>,
) -> Result<Page, AppError> {
    let connection = state.connection.read().await;
    let live = editing
        .session(&connection.client, &entry_id, index)
        .await?;
    let mut guard = live.session.lock().await;
    let session = guard.as_mut().ok_or_else(no_editing_session)?;
    session.delete_note(note_index).await?;
    Ok(session.page().clone())
}

#[tauri::command]
pub async fn recognize_region(
    info_url: String,
    xywh: [u32; 4],
    fetcher: State<'_, honkoku_iiif::Fetcher>,
) -> Result<Vec<honkoku_core::recognition::Prediction>, AppError> {
    let image_error = |_| AppError {
        kind: "iiif".into(),
        message: "原本の切り抜きを取得できません。".into(),
    };
    fetcher.allow_url(&info_url).map_err(image_error)?;
    let cached = fetcher.get(&info_url).await.map_err(image_error)?;
    let bytes = cached.read().await.map_err(image_error)?;
    let info: Value = serde_json::from_slice(&bytes).map_err(|_| AppError {
        kind: "iiif".into(),
        message: "原本の画像情報を読み取れません。".into(),
    })?;
    let upstream = recognition_crop_url(&info, &info_url, xywh)?;
    fetcher.allow_url(&upstream).map_err(image_error)?;
    let crop = fetcher.get(&upstream).await.map_err(image_error)?;
    let bytes = crop.read().await.map_err(image_error)?;
    honkoku_core::recognition::predict(&bytes)
        .await
        .map_err(|error| {
            let timeout = matches!(&error, honkoku_core::Error::Http(e) if e.is_timeout());
            AppError {
                kind: if timeout { "timeout" } else { "recognition" }.into(),
                message: if timeout {
                    "文字認識が時間内に完了しませんでした。"
                } else {
                    "文字認識の結果を取得できません。"
                }
                .into(),
            }
        })
}

fn recognition_crop_url(info: &Value, info_url: &str, xywh: [u32; 4]) -> Result<String, AppError> {
    let [x, y, w, h] = xywh;
    if w == 0 || h == 0 || !info_url.ends_with("/info.json") {
        return Err(AppError {
            kind: "invalid".into(),
            message: "認識する範囲を選んでください。".into(),
        });
    }
    if x as u64 + w as u64 > info["width"].as_u64().unwrap_or(0)
        || y as u64 + h as u64 > info["height"].as_u64().unwrap_or(0)
    {
        return Err(AppError {
            kind: "invalid".into(),
            message: "認識範囲が原本の外にあります。".into(),
        });
    }
    let width = (u64::from(w) * 64)
        .div_ceil(u64::from(w.min(h)))
        .max(u64::from(w));
    let url = format!(
        "{}/{x},{y},{w},{h}/{width},/0/default.jpg",
        info_url.trim_end_matches("/info.json")
    );
    glyph_service_url(info, info_url, &url)
}

#[cfg(test)]
mod recognition_region_tests {
    use super::*;
    #[test]
    fn crops_use_full_image_pixels_and_upscale_both_orientations() {
        let url = "https://example.org/image/info.json";
        let info = json!({"width":1000,"height":2000,"@context":"http://iiif.io/api/image/3/context.json"});
        assert_eq!(
            recognition_crop_url(&info, url, [10, 20, 20, 100]).unwrap(),
            "https://example.org/image/10,20,20,100/^64,/0/default.jpg"
        );
        assert_eq!(
            recognition_crop_url(&info, url, [10, 20, 100, 20]).unwrap(),
            "https://example.org/image/10,20,100,20/^320,/0/default.jpg"
        );
        assert_eq!(
            recognition_crop_url(&info, url, [10, 20, 80, 120]).unwrap(),
            "https://example.org/image/10,20,80,120/80,/0/default.jpg"
        );
        assert!(recognition_crop_url(&info, url, [990, 20, 20, 100]).is_err());
        assert!(recognition_crop_url(&info, url, [10, 20, 0, 100]).is_err());
    }
}
