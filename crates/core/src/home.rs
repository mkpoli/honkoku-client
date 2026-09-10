//! Public dashboard reads and bounded, cached document enrichment.
use crate::{
    Error, HonkokuClient, Result,
    firestore::{and_filters, equal},
    model::{Announcement, DailyProgress, Entry, Project, TimelineEvent, TimelineItem, User},
};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TimelineFilter {
    #[default]
    All,
    Project(String),
    Joined(Vec<String>),
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RankingSort {
    #[default]
    Exp,
    CharCount,
    LikeCount,
}
impl RankingSort {
    pub fn field(self) -> &'static str {
        match self {
            Self::Exp => "exp",
            Self::CharCount => "charCount",
            Self::LikeCount => "likeCount",
        }
    }
}
fn ordered(collection: &str, field: &str, limit: u32) -> Value {
    json!({"from":[{"collectionId":collection}],"orderBy":[{"field":{"fieldPath":field},"direction":"DESCENDING"}],"limit":limit})
}
fn in_filter(field: &str, values: &[String]) -> Value {
    json!({"fieldFilter":{"field":{"fieldPath":field},"op":"IN","value":{"arrayValue":{"values":values.iter().map(|s| json!({"stringValue":s})).collect::<Vec<_>>()}}}})
}
fn timeline_queries(filter: TimelineFilter, limit: u32) -> Vec<Value> {
    let public_types = vec!["official".to_owned(), "user".to_owned()];
    let constraints = match filter {
        TimelineFilter::All => vec![vec![in_filter("projectType", &public_types)]],
        TimelineFilter::Project(id) => vec![vec![
            in_filter("projectType", &public_types),
            equal("projectId", json!({"stringValue":id})),
        ]],
        TimelineFilter::Joined(ids) => {
            let ids: Vec<_> = ids
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            // Two IN predicates multiply their disjunction counts. Split projectType
            // into two queries so each 30-ID chunk stays within the 30-disjunction limit.
            ids.chunks(30)
                .flat_map(|chunk| {
                    public_types.iter().map(move |kind| {
                        vec![
                            equal("projectType", json!({"stringValue":kind})),
                            in_filter("projectId", chunk),
                        ]
                    })
                })
                .collect()
        }
    };
    constraints
        .into_iter()
        .map(|mut filters| {
            filters.push(equal("eventType", json!({"stringValue":"transcription"})));
            filters.push(equal("share", json!({"booleanValue":true})));
            let mut query = ordered("timelineEvents", "createdAt", limit);
            query["where"] = and_filters(filters);
            query
        })
        .collect()
}
impl HonkokuClient {
    pub async fn announcements(&self, limit: Option<u32>) -> Result<Vec<Announcement>> {
        let limit = limit.unwrap_or(5);
        if limit == 0 {
            return Ok(vec![]);
        }
        let mut query = ordered("adminAnnouncements", "createdAt", limit);
        query["where"] = equal("display", json!({"booleanValue":true}));
        self.run_query(query).await
    }
    pub async fn daily_progress(&self, limit: Option<u32>) -> Result<Vec<DailyProgress>> {
        let limit = limit.unwrap_or(2);
        if limit == 0 {
            return Ok(vec![]);
        }
        self.run_query(ordered("dailyProgress", "timestamp", limit))
            .await
    }
    pub async fn ranking(&self, sort: RankingSort, limit: Option<u32>) -> Result<Vec<User>> {
        let limit = limit.unwrap_or(100);
        if limit == 0 {
            return Ok(vec![]);
        }
        let values: Vec<Value> = self
            .run_query(ordered("users", sort.field(), limit))
            .await?;
        let users: Vec<User> = values
            .into_iter()
            .map(|mut value| {
                if value.get("uid").is_none() {
                    value["uid"] = value["id"].clone();
                }
                serde_json::from_value(value).map_err(Error::from)
            })
            .collect::<Result<_>>()?;
        let copy = users.clone();
        crate::cache::blocking(&self.home_storage, move |db| {
            db.transaction(|db| {
                for user in &copy {
                    db.put_user(user)?;
                }
                Ok(())
            })
        })
        .await?;
        Ok(users)
    }
    pub async fn timeline(
        &self,
        filter: TimelineFilter,
        limit: Option<u32>,
    ) -> Result<Vec<TimelineItem>> {
        self.timeline_with_refresh(filter, limit, false).await
    }
    /// Refresh bypasses enrichment freshness as well as fetching current events.
    pub async fn timeline_with_refresh(
        &self,
        filter: TimelineFilter,
        limit: Option<u32>,
        refresh: bool,
    ) -> Result<Vec<TimelineItem>> {
        let limit = limit.unwrap_or(20);
        if limit == 0 {
            return Ok(vec![]);
        }
        let batches: Vec<Vec<TimelineEvent>> = stream::iter(timeline_queries(filter, limit))
            .map(|query| self.run_query(query))
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
        let mut seen = BTreeSet::new();
        events.retain(|event| seen.insert(event.id.clone()));
        events.truncate(limit as usize);
        let copy = events.clone();
        crate::cache::blocking(&self.home_storage, move |db| {
            db.transaction(|db| {
                for event in &copy {
                    db.put_timeline_event(event)?;
                }
                Ok(())
            })
        })
        .await?;

        // A single queue bounds all join types together and deduplicates document IDs.
        let paths: BTreeSet<_> = events
            .iter()
            .flat_map(|e| {
                [
                    format!("users/{}", e.uid),
                    format!("entries/{}", e.entry_id),
                    format!("projects/{}", e.project_id),
                ]
            })
            .collect();
        let documents: BTreeMap<String, Option<Value>> = stream::iter(paths)
            .map(|path| async move {
                let value = self
                    .cached(
                        &self.home_storage,
                        format!("home/document/{path}"),
                        refresh,
                        async {
                            match self.document::<Value>(&path).await {
                                Ok(mut value) => {
                                    if let Some(uid) = path.strip_prefix("users/") {
                                        value["uid"] = json!(uid);
                                    }
                                    Ok(Some(value))
                                }
                                Err(Error::Http(error))
                                    if error.status() == Some(reqwest::StatusCode::NOT_FOUND) =>
                                {
                                    Ok(None)
                                }
                                Err(error) => Err(error),
                            }
                        },
                        |db, value| {
                            if let Some(value) = value {
                                let name = value["_firestore"]["name"].as_str().unwrap_or_default();
                                if name.contains("/documents/users/") {
                                    db.put_user(value)?;
                                } else if name.contains("/documents/entries/") {
                                    db.put_entry(value)?;
                                } else if name.contains("/documents/projects/") {
                                    db.put_project(value)?;
                                }
                            }
                            Ok(())
                        },
                    )
                    .await?;
                Ok::<_, Error>((path, value))
            })
            .buffered(4)
            .try_collect()
            .await?;
        events
            .into_iter()
            .map(|event| {
                let get = |path: String| documents.get(&path).and_then(Option::as_ref).cloned();
                let actor = get(format!("users/{}", event.uid))
                    .map(serde_json::from_value::<User>)
                    .transpose()?;
                let entry = get(format!("entries/{}", event.entry_id))
                    .map(serde_json::from_value::<Entry>)
                    .transpose()?;
                let project = get(format!("projects/{}", event.project_id))
                    .map(serde_json::from_value::<Project>)
                    .transpose()?;
                let excerpt =
                    honkoku_text::excerpt(event.data["text"].as_str().unwrap_or_default(), 60);
                Ok(TimelineItem {
                    event,
                    actor,
                    entry_label: entry.map(|e| e.label),
                    project_title: project.map(|p| p.title),
                    excerpt,
                })
            })
            .collect()
    }
}
