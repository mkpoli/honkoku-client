//! Shared public cache policy for the CLI and desktop shell.
use crate::{
    Error, HonkokuClient, Result,
    model::{Collection, Entry, Page, Project},
};
use futures_util::{StreamExt, TryStreamExt, stream};
use honkoku_storage::Storage;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
pub type SharedStorage = Arc<Mutex<Storage>>;
const MAX_AGE: Duration = Duration::from_secs(600);

pub async fn blocking<T: Send + 'static>(
    storage: &SharedStorage,
    operation: impl FnOnce(&mut Storage) -> honkoku_storage::Result<T> + Send + 'static,
) -> Result<T> {
    let storage = storage.clone();
    tokio::task::spawn_blocking(move || {
        let mut storage = storage.lock().map_err(|e| Error::Worker(e.to_string()))?;
        Ok(operation(&mut storage)?)
    })
    .await
    .map_err(|e| Error::Worker(e.to_string()))?
}
impl HonkokuClient {
    pub(crate) async fn cached<T, F, P>(
        &self,
        storage: &SharedStorage,
        key: String,
        refresh: bool,
        fetch: F,
        persist: P,
    ) -> Result<T>
    where
        T: Serialize + DeserializeOwned + Clone + Send + 'static,
        F: Future<Output = Result<T>>,
        P: FnOnce(&Storage, &T) -> honkoku_storage::Result<()> + Send + 'static,
    {
        if !refresh {
            let lookup = key.clone();
            if let Some(value) =
                blocking(storage, move |db| db.fresh_response(&lookup, MAX_AGE)).await?
            {
                return Ok(value);
            }
        }
        let value = fetch.await?;
        let copy = value.clone();
        blocking(storage, move |db| {
            db.transaction(|db| {
                persist(db, &copy)?;
                db.put_response(&key, &copy)
            })
        })
        .await?;
        Ok(value)
    }
    pub async fn cached_projects(
        &self,
        storage: &SharedStorage,
        refresh: bool,
    ) -> Result<Vec<Project>> {
        self.cached(
            storage,
            "projects".into(),
            refresh,
            self.projects(),
            |db, projects| {
                for project in projects {
                    db.put_project(project)?;
                }
                Ok(())
            },
        )
        .await
    }
    pub async fn cached_project(
        &self,
        storage: &SharedStorage,
        id: &str,
        refresh: bool,
    ) -> Result<Project> {
        self.cached(
            storage,
            format!("project/{id}"),
            refresh,
            self.project(id),
            |db, value| db.put_project(value),
        )
        .await
    }
    pub async fn cached_collection(
        &self,
        storage: &SharedStorage,
        id: &str,
        refresh: bool,
    ) -> Result<Collection> {
        self.cached(
            storage,
            format!("collection/{id}"),
            refresh,
            self.collection(id),
            |db, value| db.put_collection(value),
        )
        .await
    }
    pub async fn cached_entry(
        &self,
        storage: &SharedStorage,
        id: &str,
        refresh: bool,
    ) -> Result<Entry> {
        self.cached(
            storage,
            format!("entry/{id}"),
            refresh,
            self.entry(id),
            |db, value| {
                db.put_entry(value)?;
                // Express transcription snapshots can lag Firestore; list_pages fetches its own source.
                Ok(())
            },
        )
        .await
    }
    pub async fn cached_pages(
        &self,
        storage: &SharedStorage,
        entry_id: &str,
        refresh: bool,
    ) -> Result<Vec<Page>> {
        let id = entry_id.to_owned();
        self.cached(
            storage,
            format!("pages/{entry_id}"),
            refresh,
            self.pages(entry_id, None),
            move |db, pages| db.replace_pages(&id, pages),
        )
        .await
    }
    pub async fn cached_collections(
        &self,
        storage: &SharedStorage,
        project_id: &str,
        refresh: bool,
    ) -> Result<Vec<Collection>> {
        let project = self.cached_project(storage, project_id, refresh).await?;
        let ids = project
            .collections
            .ok_or_else(|| Error::Invalid("project detail has no collections field".into()))?;
        stream::iter(ids)
            .map(|id| async move { self.cached_collection(storage, &id, refresh).await })
            .buffered(4)
            .try_collect()
            .await
    }
}
