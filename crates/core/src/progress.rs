//! Lightweight entry listings and ten-minute progress snapshots.
use crate::{
    HonkokuClient, Result,
    model::{CollectionProgress, EntryProgress, EntrySummary, Timestamp},
};
use futures_util::{StreamExt, TryStreamExt, stream};
use time::OffsetDateTime;

impl HonkokuClient {
    pub async fn list_entries(&self, collection_id: &str) -> Result<Vec<EntrySummary>> {
        self.list_entries_with_refresh(collection_id, false).await
    }
    pub async fn list_entries_with_refresh(
        &self,
        collection_id: &str,
        refresh: bool,
    ) -> Result<Vec<EntrySummary>> {
        let gate = self
            .summary_gates
            .lock()
            .map_err(|e| crate::Error::Worker(e.to_string()))?
            .entry(collection_id.into())
            .or_default()
            .clone();
        let _guard = gate.lock().await;
        self.cached(
            &self.home_storage,
            format!("entry-summaries/{collection_id}"),
            refresh,
            self.entries_in_collection(collection_id),
            |db, entries| {
                for entry in entries {
                    db.put_response(&format!("entry-summary/{}", entry.id), entry)?;
                }
                Ok(())
            },
        )
        .await
    }
    pub async fn collection_progress(
        &self,
        collection_id: &str,
        refresh: bool,
    ) -> Result<CollectionProgress> {
        self.cached(
            &self.home_storage,
            format!("collection-progress/{collection_id}"),
            refresh,
            async {
                let entries = self
                    .list_entries_with_refresh(collection_id, refresh)
                    .await?;
                let ids: Vec<_> = entries.iter().map(|entry| entry.id.clone()).collect();
                let counts = self.collection_status_counts(&ids).await?;
                Ok(CollectionProgress {
                    collection_id: collection_id.into(),
                    entries: entries.len(),
                    size: entries
                        .iter()
                        .map(|entry| u64::from(entry.size.unwrap_or(0)))
                        .sum(),
                    counts,
                    fetched_at: Timestamp(OffsetDateTime::now_utc()),
                })
            },
            |_, _| Ok(()),
        )
        .await
    }
    pub async fn entry_progress(&self, entry_ids: &[String]) -> Result<Vec<EntryProgress>> {
        self.entry_progress_with_refresh(entry_ids, false).await
    }
    pub async fn entry_progress_with_refresh(
        &self,
        entry_ids: &[String],
        refresh: bool,
    ) -> Result<Vec<EntryProgress>> {
        let mut seen = std::collections::HashSet::new();
        let ids: Vec<_> = entry_ids
            .iter()
            .filter(|id| seen.insert(*id))
            .cloned()
            .collect();
        stream::iter(ids)
            .map(|id| async move {
                self.cached(
                    &self.home_storage,
                    format!("entry-progress/{id}"),
                    refresh,
                    async {
                        let summary: EntrySummary = self
                            .cached(
                                &self.home_storage,
                                format!("entry-summary/{id}"),
                                refresh,
                                self.document(&format!("entries/{id}")),
                                |_, _| Ok(()),
                            )
                            .await?;
                        let mut counts = self.page_status_counts(std::slice::from_ref(&id)).await?;
                        Ok(EntryProgress {
                            entry_id: id.clone(),
                            size: u64::from(summary.size.unwrap_or(0)),
                            counts: counts.remove(&id).ok_or_else(|| {
                                crate::Error::Invalid("entry count missing".into())
                            })?,
                            fetched_at: Timestamp(OffsetDateTime::now_utc()),
                        })
                    },
                    |_, _| Ok(()),
                )
                .await
            })
            .buffered(4)
            .try_collect()
            .await
    }
}

#[cfg(test)]
mod tests;
