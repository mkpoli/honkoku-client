//! Firestore REST value decoding and cursor-paginated public reads.
use crate::{
    Error, HonkokuClient, Result, endpoint,
    model::{Notification, Page, User},
};
use reqwest::Method;
use serde_json::{Map, Value, json};

pub fn decode_value(value: &Value) -> Result<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::Invalid("Firestore value must be an object".into()))?;
    if object.len() != 1 {
        return Err(Error::Invalid("Firestore value must have one type".into()));
    }
    for key in [
        "stringValue",
        "timestampValue",
        "referenceValue",
        "bytesValue",
        "booleanValue",
        "nullValue",
        "geoPointValue",
    ] {
        if let Some(value) = object.get(key) {
            return Ok(value.clone());
        }
    }
    if let Some(value) = object.get("integerValue") {
        let integer = value
            .as_str()
            .ok_or_else(|| Error::Invalid("integerValue must be a string".into()))?
            .parse::<i64>()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        return Ok(json!(integer));
    }
    if let Some(value) = object.get("doubleValue") {
        return Ok(value.clone());
    }
    if let Some(value) = object.get("arrayValue") {
        let items = match value.get("values") {
            None => vec![],
            Some(values) => values
                .as_array()
                .ok_or_else(|| Error::Invalid("array values must be an array".into()))?
                .iter()
                .map(decode_value)
                .collect::<Result<_>>()?,
        };
        return Ok(Value::Array(items));
    }
    if let Some(value) = object.get("mapValue") {
        return Ok(Value::Object(decode_fields(value.get("fields"))?));
    }
    Err(Error::Invalid("unknown Firestore value type".into()))
}
fn decode_fields(fields: Option<&Value>) -> Result<Map<String, Value>> {
    match fields {
        None => Ok(Map::new()),
        Some(fields) => fields
            .as_object()
            .ok_or_else(|| Error::Invalid("fields must be an object".into()))?
            .iter()
            .map(|(key, value)| Ok((key.clone(), decode_value(value)?)))
            .collect(),
    }
}
pub fn decode_document(document: &Value) -> Result<Value> {
    let name = document["name"]
        .as_str()
        .ok_or_else(|| Error::Invalid("document name missing".into()))?;
    let id = name
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::Invalid("document ID missing".into()))?;
    let mut fields = decode_fields(document.get("fields"))?;
    fields.insert("id".into(), json!(id));
    // Keep the server revision distinct from application updatedAt.
    let mut metadata = document
        .as_object()
        .cloned()
        .ok_or_else(|| Error::Invalid("document must be an object".into()))?;
    metadata.remove("fields");
    fields.insert("_firestore".into(), Value::Object(metadata));
    Ok(Value::Object(fields))
}
pub fn decode_pages(response: &Value) -> Result<Vec<Page>> {
    decode_query(response)
}
pub fn decode_query<T: serde::de::DeserializeOwned>(response: &Value) -> Result<Vec<T>> {
    response
        .as_array()
        .ok_or_else(|| Error::Invalid("runQuery response must be an array".into()))?
        .iter()
        .filter_map(|row| {
            if let Some(error) = row.get("error") {
                Some(Err(Error::Invalid(error.to_string())))
            } else {
                row.get("document")
                    .map(|document| Ok(serde_json::from_value(decode_document(document)?)?))
            }
        })
        .collect()
}
fn query(entry_id: &str, cursor: Option<Value>) -> Value {
    let mut query = json!({
        "from":[{"collectionId":"transcriptions"}],
        "where":{"fieldFilter":{"field":{"fieldPath":"entryId"},"op":"EQUAL","value":{"stringValue":entry_id}}},
        "orderBy":[{"field":{"fieldPath":"index"},"direction":"ASCENDING"},{"field":{"fieldPath":"__name__"},"direction":"ASCENDING"}],
        "limit":200
    });
    if let Some(cursor) = cursor {
        query["startAt"] = cursor;
    }
    json!({"structuredQuery":query})
}
impl HonkokuClient {
    pub async fn document<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let segments: Vec<_> = path.split('/').collect();
        if !segments.len().is_multiple_of(2)
            || segments
                .iter()
                .any(|s| s.is_empty() || *s == "." || *s == "..")
        {
            return Err(Error::Invalid("expected a Firestore document path".into()));
        }
        let document: Value = self
            .request(
                Method::GET,
                endpoint(&self.firestore_base, &segments)?,
                None::<&()>,
                None,
            )
            .await?;
        Ok(serde_json::from_value(decode_document(&document)?)?)
    }
    pub async fn run_query<T: serde::de::DeserializeOwned>(
        &self,
        structured_query: Value,
    ) -> Result<Vec<T>> {
        decode_query(&self.query_response(structured_query, None).await?)
    }
    async fn query_response(&self, structured_query: Value, token: Option<&str>) -> Result<Value> {
        self.request(
            Method::POST,
            endpoint(&format!("{}:runQuery", self.firestore_base), &[])?,
            Some(&json!({"structuredQuery":structured_query})),
            token,
        )
        .await
    }
    pub async fn run_aggregation_count(
        &self,
        collection: &str,
        filters: Vec<Value>,
    ) -> Result<u64> {
        let mut query = json!({"from":[{"collectionId":collection}]});
        if !filters.is_empty() {
            query["where"] = and_filters(filters);
        }
        let response: Value = self.request(Method::POST, endpoint(&format!("{}:runAggregationQuery", self.firestore_base), &[])?, Some(&json!({"structuredAggregationQuery":{"structuredQuery":query,"aggregations":[{"alias":"count","count":{}}]}})), None).await?;
        let rows = response
            .as_array()
            .ok_or_else(|| Error::Invalid("aggregation response must be an array".into()))?;
        let mut count = None;
        for row in rows {
            if let Some(error) = row.get("error") {
                return Err(Error::Invalid(error.to_string()));
            }
            if let Some(value) = row.pointer("/result/aggregateFields/count") {
                if count.is_some() {
                    return Err(Error::Invalid("duplicate aggregation result".into()));
                }
                count = Some(
                    decode_value(value)?
                        .as_u64()
                        .ok_or_else(|| Error::Invalid("invalid aggregation count".into()))?,
                );
            }
        }
        count.ok_or_else(|| Error::Invalid("aggregation count missing".into()))
    }
    pub async fn pages(&self, entry_id: &str, token: Option<&str>) -> Result<Vec<Page>> {
        let mut pages = Vec::new();
        let mut cursor = None;
        loop {
            let body = query(entry_id, cursor.clone());
            let response = self
                .query_response(body["structuredQuery"].clone(), token)
                .await?;
            let batch = decode_pages(&response)?;
            if batch.iter().any(|page| page.entry_id != entry_id) {
                return Err(Error::Invalid("query returned a different entry".into()));
            }
            let count = batch.len();
            pages.extend(batch);
            if count < 200 {
                break;
            }
            let document = response
                .as_array()
                .and_then(|rows| rows.iter().rev().find_map(|row| row.get("document")))
                .ok_or_else(|| Error::Invalid("missing cursor document".into()))?;
            let next = json!({"values":[document["fields"]["index"].clone(),{"referenceValue":document["name"]}],"before":false});
            if cursor.as_ref() == Some(&next) {
                return Err(Error::Invalid("Firestore cursor did not advance".into()));
            }
            cursor = Some(next);
        }
        Ok(pages)
    }
    pub async fn user(&self, uid: &str) -> Result<User> {
        if uid.contains('/') {
            return Err(Error::Invalid("invalid uid".into()));
        }
        let mut value: Value = self.document(&format!("users/{uid}")).await?;
        value["uid"] = json!(uid);
        Ok(serde_json::from_value(value)?)
    }
    pub async fn me(&self) -> Result<User> {
        self.user(&self.signed_in_uid().await?).await
    }
    pub async fn unread_notification_count(&self) -> Result<u64> {
        let uid = self.signed_in_uid().await?;
        self.run_aggregation_count(
            "notifications",
            vec![
                equal("uid", json!({"stringValue":uid})),
                equal("state", json!({"stringValue":"unchecked"})),
            ],
        )
        .await
    }
    pub async fn notifications(&self, limit: u32) -> Result<Vec<Notification>> {
        let uid = self.signed_in_uid().await?;
        if limit == 0 {
            return Ok(vec![]);
        }
        self.run_query(json!({"from":[{"collectionId":"notifications"}],"where":equal("uid",json!({"stringValue":uid})),"orderBy":[{"field":{"fieldPath":"createdAt"},"direction":"DESCENDING"}],"limit":limit})).await
    }
}
pub(crate) fn equal(field: &str, value: Value) -> Value {
    json!({"fieldFilter":{"field":{"fieldPath":field},"op":"EQUAL","value":value}})
}
pub(crate) fn and_filters(filters: Vec<Value>) -> Value {
    json!({"compositeFilter":{"op":"AND","filters":filters}})
}
