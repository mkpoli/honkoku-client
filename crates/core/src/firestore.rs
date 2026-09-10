//! Firestore REST value decoding and cursor-paginated public reads.
use crate::{
    Error, HonkokuClient, Result, endpoint,
    model::{Notification, Page, User},
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
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
    if let Some(timestamp) = object.get("timestampValue") {
        return Ok(json!({"$firestoreTimestamp": timestamp}));
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
    Ok(plain_value(&Value::Object(fields)))
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

/// Timestamp tags retain the Firestore type without mistaking date-like text for a timestamp.
/// Empty arrays and maps use the explicit REST write representation.
pub fn encode_value(value: &Value) -> Value {
    match value {
        Value::Null => json!({"nullValue": null}),
        Value::Bool(value) => json!({"booleanValue": value}),
        Value::Number(value) if value.is_i64() || value.is_u64() => {
            json!({"integerValue": value.to_string()})
        }
        Value::Number(value) => json!({"doubleValue": value}),
        Value::String(value) => json!({"stringValue": value}),
        Value::Array(values) => {
            json!({"arrayValue": {"values": values.iter().map(encode_value).collect::<Vec<_>>()}})
        }
        Value::Object(fields) => {
            if fields.len() == 1
                && let Some(value) = fields.get("$firestoreTimestamp")
            {
                return json!({"timestampValue": value});
            }
            json!({"mapValue": {"fields": fields.iter().map(|(k,v)| (k.clone(), encode_value(v))).collect::<Map<_,_>>()}})
        }
    }
}
impl crate::model::Timestamp {
    pub fn firestore_value(&self) -> Result<Value> {
        Ok(encode_value(
            &json!({"$firestoreTimestamp": serde_json::to_value(self)?}),
        ))
    }
}
/// Convert typed Firestore JSON to the plain JSON used by public models and IPC.
pub fn plain_value(value: &Value) -> Value {
    match value {
        Value::Object(fields)
            if fields.len() == 1 && fields.contains_key("$firestoreTimestamp") =>
        {
            fields["$firestoreTimestamp"].clone()
        }
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), plain_value(v)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(plain_value).collect()),
        value => value.clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadDocument {
    pub name: String,
    pub fields: Value,
    pub update_time: String,
}
impl ReadDocument {
    pub fn page(&self) -> Result<Page> {
        let mut fields = plain_value(&self.fields);
        if fields["notes"].is_null() {
            fields["notes"] = json!([]);
        }
        fields["id"] = json!(self.name.rsplit('/').next().unwrap_or_default());
        fields["_firestore"] = json!({"name":self.name,"updateTime":self.update_time});
        Ok(serde_json::from_value(fields)?)
    }
}
#[derive(Debug, Clone, Serialize)]
pub enum ServerValue {
    #[serde(rename = "REQUEST_TIME")]
    RequestTime,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Precondition {
    pub update_time: String,
}
#[derive(Debug, Clone)]
pub enum Write {
    Update {
        name: String,
        fields: Value,
        update_mask: Vec<String>,
        update_transforms: Vec<(String, ServerValue)>,
        precondition: Option<Precondition>,
    },
    Set {
        name: String,
        fields: Value,
        update_transforms: Vec<(String, ServerValue)>,
        precondition: Option<Precondition>,
    },
    Verify {
        name: String,
        update_time: String,
    },
}
impl Serialize for Write {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let (name, fields, mask, transforms, precondition) = match self {
            Self::Verify { name, update_time } => {
                return json!({"verify":name,"currentDocument":{"updateTime":update_time}})
                    .serialize(serializer);
            }
            Self::Update {
                name,
                fields,
                update_mask,
                update_transforms,
                precondition,
            } => (
                name,
                fields,
                Some(update_mask),
                update_transforms,
                precondition,
            ),
            Self::Set {
                name,
                fields,
                update_transforms,
                precondition,
            } => (name, fields, None, update_transforms, precondition),
        };
        if !fields.is_object() {
            return Err(serde::ser::Error::custom("write fields must be an object"));
        }
        let mut value =
            json!({"update":{"name":name,"fields":encode_value(fields)["mapValue"]["fields"]}});
        if let Some(mask) = mask {
            let mut mask = mask.clone();
            mask.sort();
            mask.dedup();
            value["updateMask"] = json!({"fieldPaths":mask});
        }
        if !transforms.is_empty() {
            value["updateTransforms"] = json!(
                transforms
                    .iter()
                    .map(|(field, server)| json!({"fieldPath":field,"setToServerValue":server}))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(precondition) = precondition {
            value["currentDocument"] =
                serde_json::to_value(precondition).map_err(serde::ser::Error::custom)?;
        }
        value.serialize(serializer)
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResponse {
    pub write_results: Vec<WriteResult>,
    pub commit_time: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    pub update_time: String,
    #[serde(default)]
    pub transform_results: Vec<Value>,
}
pub fn auto_id() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..20)
        .map(|_| char::from(ALPHABET[rng.random_range(0..ALPHABET.len())]))
        .collect()
}
impl HonkokuClient {
    pub fn document_name(&self, path: &str) -> Result<String> {
        let prefix = crate::FIRESTORE_BASE
            .strip_prefix("https://firestore.googleapis.com/v1/")
            .ok_or_else(|| Error::Invalid("invalid Firestore base".into()))?;
        let path = path.strip_prefix(&format!("{prefix}/")).unwrap_or(path);
        let segments: Vec<_> = path.split('/').collect();
        if !segments.len().is_multiple_of(2)
            || segments
                .iter()
                .any(|s| s.is_empty() || *s == "." || *s == "..")
        {
            return Err(Error::Invalid("expected a Firestore document path".into()));
        }
        Ok(format!("{prefix}/{path}"))
    }
    /// Results follow the requested order, regardless of the server's streaming order.
    pub async fn batch_get(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<Option<ReadDocument>>> {
        let names = paths
            .into_iter()
            .map(|p| self.document_name(p.as_ref()))
            .collect::<Result<Vec<_>>>()?;
        if names.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<Value> = self
            .request(
                Method::POST,
                endpoint(&format!("{}:batchGet", self.firestore_base), &[])?,
                Some(&json!({"documents":names})),
                None,
            )
            .await?;
        let mut documents = std::collections::HashMap::new();
        for row in rows {
            let (name, document) = if let Some(found) = row.get("found") {
                let name = found["name"]
                    .as_str()
                    .ok_or_else(|| Error::Invalid("document name missing".into()))?
                    .to_owned();
                let update_time = found["updateTime"]
                    .as_str()
                    .ok_or_else(|| Error::Invalid("document updateTime missing".into()))?
                    .to_owned();
                let fields = Value::Object(decode_fields(found.get("fields"))?);
                (
                    name.clone(),
                    Some(ReadDocument {
                        name,
                        fields,
                        update_time,
                    }),
                )
            } else if let Some(missing) = row["missing"].as_str() {
                (missing.to_owned(), None)
            } else {
                return Err(Error::Invalid("invalid batchGet row".into()));
            };
            if !names.contains(&name) || documents.insert(name, document).is_some() {
                return Err(Error::Invalid(
                    "unexpected or duplicate batchGet document".into(),
                ));
            }
        }
        names
            .iter()
            .map(|name| {
                documents
                    .get(name)
                    .cloned()
                    .ok_or_else(|| Error::Invalid("batchGet omitted a document".into()))
            })
            .collect()
    }
    pub async fn commit(&self, writes: Vec<Write>) -> Result<CommitResponse> {
        let body = json!({"writes":serde_json::to_value(&writes)?});
        let response = self
            .request_response(
                Method::POST,
                endpoint(&format!("{}:commit", self.firestore_base), &[])?,
                Some(&body),
                None,
            )
            .await?;
        if response.status() == reqwest::StatusCode::BAD_REQUEST {
            let http_error = response.error_for_status_ref().err();
            let body: Value = response.json().await?;
            if body["error"]["status"] == "FAILED_PRECONDITION" {
                let path = writes
                    .iter()
                    .find_map(|write| match write {
                        Write::Update { name, .. } | Write::Set { name, .. }
                            if name.contains("/transcriptions/") =>
                        {
                            Some(name.clone())
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        Error::Invalid("precondition failed without a page write".into())
                    })?;
                let current = self.batch_get([&path]).await?.into_iter().next().flatten();
                return Err(Error::Conflict { path, current });
            }
            return Err(http_error
                .map(Error::Http)
                .unwrap_or_else(|| Error::Invalid("commit HTTP 400".into())));
        }
        Ok(response.error_for_status()?.json().await?)
    }
}
