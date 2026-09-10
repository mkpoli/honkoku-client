use crate::{AppError, AppState};
use futures_util::{StreamExt, TryStreamExt, stream};
use honkoku_core::{
    auth::{Session, SessionStore, TokenManager, import_dev_session},
    home::{RankingSort, TimelineFilter as CoreTimelineFilter},
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
}
impl From<&Session> for SessionInfo {
    fn from(value: &Session) -> Self {
        Self {
            uid: value.uid.clone(),
            display_name: value.display_name.clone(),
            providers: value.providers.clone(),
        }
    }
}
#[tauri::command]
pub async fn session_import(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SessionInfo, AppError> {
    let path = app
        .path()
        .home_dir()
        .map_err(|_| AppError {
            kind: "io".into(),
            message: "ホームフォルダーを取得できません。".into(),
        })?
        .join(".local/share/honkoku-client/session.json");
    let mut connection = state.connection.write().await;
    let session = import_dev_session(path)?;
    let info = SessionInfo::from(&session);
    let client = state
        .anonymous()?
        .with_session(TokenManager::new(session.clone(), state.store.clone())?);
    state.store.save(&session)?;
    connection.client = client;
    connection.session = Some(info.clone());
    Ok(info)
}
#[tauri::command]
pub async fn session_current(state: State<'_, AppState>) -> Result<Option<SessionInfo>, AppError> {
    let mut connection = state.connection.write().await;
    if connection.session.is_some() {
        match connection.client.signed_in_uid().await {
            Ok(_) => {}
            Err(honkoku_core::Error::SignedOut) => {
                connection.client = state.anonymous()?;
                connection.session = None;
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(connection.session.clone())
}
#[tauri::command]
pub async fn session_clear(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut connection = state.connection.write().await;
    let client = state.anonymous()?;
    state.store.clear()?;
    connection.client = client;
    connection.session = None;
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
#[tauri::command]
pub async fn home_ranking(
    sort: Sort,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<User>, AppError> {
    let sort = match sort {
        Sort::Exp => RankingSort::Exp,
        Sort::CharCount => RankingSort::CharCount,
        Sort::LikeCount => RankingSort::LikeCount,
    };
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
