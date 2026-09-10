//! Local reading history enriched only from cached records.
use crate::{
    HonkokuClient, Result,
    cache::blocking,
    model::{EntrySummary, Label, Page, Project},
};
use honkoku_storage::HistoryRecord;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWork {
    #[serde(flatten)]
    pub record: HistoryRecord,
    pub entry_label: Option<Label>,
    pub project_title: Option<String>,
    pub thumbnail: Option<String>,
    pub next_unfinished_index: Option<u32>,
}
impl HonkokuClient {
    pub async fn history_open(&self, entry_id: &str, index: u32) -> Result<()> {
        let id = entry_id.to_owned();
        let cached: Option<EntrySummary> =
            blocking(&self.home_storage, move |db| db.get_entry(&id)).await?;
        let entry = match cached {
            Some(entry) => entry,
            None => {
                let entry: EntrySummary = self.document(&format!("entries/{entry_id}")).await?;
                let copy = entry.clone();
                blocking(&self.home_storage, move |db| db.put_entry(&copy)).await?;
                entry
            }
        };
        let id = entry_id.to_owned();
        let pages: Vec<Page> = blocking(&self.home_storage, move |db| db.list_pages(&id)).await?;
        let page = pages.iter().find(|p| p.index == index);
        if index >= entry.size.unwrap_or_default() && page.is_none() {
            return Err(crate::Error::Invalid("page index outside entry".into()));
        }
        let status = page
            .map(|p| p.status.as_str())
            .unwrap_or("default")
            .to_owned();
        blocking(&self.home_storage, move |db| {
            db.history_record(&entry.id, index, &entry.project_id, &status, false)
        })
        .await
    }
    pub async fn history_saved(&self, page: &Page, project_id: &str) -> Result<()> {
        let page = page.clone();
        let project_id = project_id.to_owned();
        blocking(&self.home_storage, move |db| {
            db.transaction(|db| {
                db.put_page(&page)?;
                db.remove_response(&format!("pages/{}", page.entry_id))?;
                db.history_record(
                    &page.entry_id,
                    page.index,
                    &project_id,
                    page.status.as_str(),
                    true,
                )
            })
        })
        .await
    }
    pub async fn history_recent(&self, limit: u32) -> Result<Vec<RecentWork>> {
        blocking(&self.home_storage, move |db| {
            db.history_recent(limit)?
                .into_iter()
                .map(|record| {
                    let entry: Option<EntrySummary> = db.get_entry(&record.entry_id)?;
                    let project: Option<Project> = db.get_project(&record.project_id)?;
                    let pages: Vec<Page> = db.list_pages(&record.entry_id)?;
                    let next_unfinished_index = entry.as_ref().and_then(|e| {
                        (0..e.size.unwrap_or_default()).find(|index| {
                            pages.iter().find(|p| p.index == *index).is_none_or(|p| {
                                matches!(p.status.as_str(), "default" | "initiated")
                            })
                        })
                    });
                    Ok(RecentWork {
                        entry_label: entry.as_ref().map(|e| e.label.clone()),
                        thumbnail: entry.and_then(|e| e.thumbnail),
                        project_title: project.map(|p| p.title),
                        record,
                        next_unfinished_index,
                    })
                })
                .collect()
        })
        .await
    }
    pub async fn history_clear(&self) -> Result<()> {
        blocking(&self.home_storage, |db| db.history_clear()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::SharedStorage;
    use honkoku_storage::Storage;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn opening_legacy_canvases_uses_cached_statuses() -> Result<()> {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        let storage: SharedStorage = Arc::new(Mutex::new(Storage::in_memory()?));
        let client = HonkokuClient::with_endpoints(&server.uri(), &server.uri())?
            .with_storage(storage.clone());
        let metadata = json!({"id":"entry","projectId":"project","collectionId":"collection","index":0,"label":"資料","manifestUrl":"https://example.org/manifest","size":3,"canvases":[{"@id":"legacy-canvas"}]});
        Mock::given(method("GET")).and(path("/entries/entry"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name":"projects/test/databases/(default)/documents/entries/entry",
                "fields": metadata.as_object().ok_or_else(|| crate::Error::Invalid("metadata object".into()))?
                    .iter().map(|(key,value)| (key.clone(), crate::firestore::encode_value(value))).collect::<serde_json::Map<_,_>>()
            }))).expect(1).mount(&server).await;
        blocking(&storage, |db| {
            db.put_page(&json!({"id":"entry_0","entryId":"entry","index":0,"status":"completed","text":"字","notes":[]}))
        }).await?;
        client.history_open("entry", 0).await?;
        let rows = client.history_recent(8).await?;
        assert_eq!(rows[0].record.status_after, "completed");
        assert_eq!(rows[0].next_unfinished_index, Some(1));
        client.history_open("entry", 1).await?;
        assert_eq!(client.history_recent(8).await?[0].record.index, 1);
        assert!(client.history_open("entry", 3).await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn recent_work_uses_cached_statuses_and_keeps_one_row_per_entry() -> Result<()> {
        let storage: SharedStorage = Arc::new(Mutex::new(Storage::in_memory()?));
        let client = HonkokuClient::new()?.with_storage(storage.clone());
        blocking(&storage, |db| {
            db.put_entry(&json!({"id":"entry","projectId":"project","collectionId":"collection","index":0,"label":"資料","manifestUrl":"https://example.org/manifest","size":3}))?;
            db.put_project(&json!({"id":"project","title":"研究"}))?;
            db.put_page(&json!({"id":"entry_0","entryId":"entry","index":0,"status":"completed","text":"字","notes":[]}))?;
            db.put_page(&json!({"id":"entry_1","entryId":"entry","index":1,"status":"editing","text":"","notes":[]}))?;
            db.history_record("entry",0,"project","completed",false)?;
            db.history_record("entry",1,"project","editing",false)?;
            db.history_record("entry",0,"project","completed",true)
        }).await?;
        let rows = client.history_recent(8).await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].record.index, 1);
        assert!(rows[0].record.saved_at.is_none());
        assert_eq!(rows[0].next_unfinished_index, Some(2));
        assert_eq!(rows[0].project_title.as_deref(), Some("研究"));
        assert!(client.history_recent(0).await?.is_empty());
        client.history_clear().await?;
        assert!(client.history_recent(8).await?.is_empty());
        Ok(())
    }
}
