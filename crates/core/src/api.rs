//! Public Express API. Child collections and entries are returned as IDs.
use crate::{
    HonkokuClient, Result, endpoint,
    model::{Collection, Entry, Project},
};
use reqwest::Method;
impl HonkokuClient {
    /// Search-service query parameters use the site's wire names (e.g. projectIds).
    pub async fn search<T: serde::de::DeserializeOwned>(
        &self,
        parameters: &[(&str, &str)],
    ) -> Result<T> {
        let mut url = endpoint(&self.search_base, &[])?;
        url.query_pairs_mut()
            .extend_pairs(parameters.iter().copied());
        self.request(Method::GET, url, None::<&()>, None).await
    }
    pub async fn callable<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
        data: &impl serde::Serialize,
    ) -> Result<T> {
        let response: serde_json::Value = self
            .request(
                Method::POST,
                endpoint(&self.functions_base, &[name])?,
                Some(&serde_json::json!({"data":data})),
                None,
            )
            .await?;
        if let Some(error) = response.get("error") {
            return Err(crate::Error::Invalid(error.to_string()));
        }
        let data = response
            .get("data")
            .or_else(|| response.get("result"))
            .ok_or_else(|| crate::Error::Invalid("callable result missing".into()))?;
        Ok(serde_json::from_value(data.clone())?)
    }
    pub async fn projects(&self) -> Result<Vec<Project>> {
        self.get_api(&["projects"]).await
    }
    pub async fn project(&self, id: &str) -> Result<Project> {
        self.get_api(&["projects", id]).await
    }
    pub async fn collection(&self, id: &str) -> Result<Collection> {
        self.get_api(&["collections", id]).await
    }
    pub async fn entry(&self, id: &str) -> Result<Entry> {
        self.get_api(&["entries", id]).await
    }
    async fn get_api<T: serde::de::DeserializeOwned>(&self, segments: &[&str]) -> Result<T> {
        self.request(
            Method::GET,
            endpoint(&self.api_base, segments)?,
            None::<&()>,
            None,
        )
        .await
    }
}
