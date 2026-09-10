//! The account's imageCollectionEntries, using the site's source-pixel regions.
use crate::{
    Error, HonkokuClient, Result, endpoint,
    firestore::{self, ServerValue, Write},
    model::Timestamp,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipInput {
    pub entry_id: String,
    pub index: u32,
    pub reading: String,
    pub tags: Vec<String>,
    pub comment: String,
    pub is_private: bool,
    pub xywh: [u32; 4],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    #[serde(flatten)]
    pub input: ClipInput,
    pub uid: String,
    pub uri: String,
    pub transcription_id: String,
    pub project_id: String,
    pub created_at: Timestamp,
}
impl HonkokuClient {
    pub async fn clips(&self) -> Result<Vec<Clip>> {
        let uid = self.signed_in_uid().await?;
        let query = json!({"from":[{"collectionId":"imageCollectionEntries"}],
            "where":firestore::equal("uid",json!({"stringValue":uid})),
            "orderBy":[{"field":{"fieldPath":"createdAt"},"direction":"DESCENDING"}]});
        let response = self
            .request_response(
                Method::POST,
                endpoint(&format!("{}:runQuery", self.firestore_base), &[])?,
                Some(&json!({"structuredQuery":query})),
                None,
            )
            .await?;
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            let failure = if value.is_array() { &value[0] } else { &value };
            let message = failure["error"]["message"]
                .as_str()
                .unwrap_or("クリップを取得できません。");
            return Err(Error::Invalid(if message.contains("requires an index") {
                "クリップの表示に必要な索引がサーバーにありません。管理者にお問い合わせください。"
                    .into()
            } else {
                message.into()
            }));
        }
        firestore::decode_query(&value)
    }
    pub async fn create_clip(&self, mut input: ClipInput) -> Result<Clip> {
        let uid = self.signed_in_uid().await?;
        self.document_name(&format!("entries/{}", input.entry_id))?;
        input.reading = input.reading.trim().into();
        input.comment = input.comment.trim().into();
        if input.reading.is_empty() {
            return Err(Error::Invalid("読みを入力してください。".into()));
        }
        input.tags = input
            .tags
            .into_iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let mut seen = std::collections::HashSet::new();
        input.tags.retain(|s| seen.insert(s.clone()));
        let entry = self.glyph_entry(&input.entry_id).await?;
        let canvas = crate::glyphs::canvas(&entry, input.index as usize)
            .ok_or_else(|| Error::Invalid("原本の情報がありません。".into()))?;
        let [x, y, w, h] = input.xywh;
        if w == 0
            || h == 0
            || u64::from(x) + u64::from(w) > u64::from(canvas.width)
            || u64::from(y) + u64::from(h) > u64::from(canvas.height)
        {
            return Err(Error::Invalid("切り抜き範囲が原本の外にあります。".into()));
        }
        let info = entry["tileSources"][input.index as usize]
            .as_str()
            .or(canvas.info_json_url.as_deref())
            .ok_or_else(|| Error::Invalid("IIIF画像がありません。".into()))?;
        let uri = clip_uri(
            info,
            input.xywh,
            entry["manifestVersion"].as_u64().unwrap_or(2),
        )?;
        let project_id = entry["projectId"]
            .as_str()
            .ok_or_else(|| Error::Invalid("projectId missing".into()))?;
        let id = firestore::auto_id();
        let mut fields = serde_json::to_value(&input)?;
        fields["uid"] = json!(uid);
        fields["uri"] = json!(uri);
        fields["transcriptionId"] = json!(format!("{}_{}", input.entry_id, input.index));
        fields["projectId"] = json!(project_id);
        let committed = self
            .commit(vec![Write::Set {
                name: self.document_name(&format!("imageCollectionEntries/{id}"))?,
                fields: fields.clone(),
                update_transforms: vec![("createdAt".into(), ServerValue::RequestTime)],
                precondition: None,
            }])
            .await?;
        fields["id"] = json!(id);
        let created_at = committed
            .write_results
            .first()
            .and_then(|r| r.transform_results.first())
            .and_then(|v| v["timestampValue"].as_str())
            .unwrap_or(&committed.commit_time);
        fields["createdAt"] = json!(created_at);
        Ok(serde_json::from_value(fields)?)
    }
    pub async fn delete_clip(&self, id: &str) -> Result<()> {
        let uid = self.signed_in_uid().await?;
        if id.is_empty() || id.contains('/') {
            return Err(Error::Invalid("invalid clip id".into()));
        }
        let path = format!("imageCollectionEntries/{id}");
        let document = self.batch_get([&path]).await?.into_iter().next().flatten();
        let Some(document) = document else {
            return Ok(());
        };
        if document.fields["uid"].as_str() != Some(&uid) {
            return Err(Error::Invalid("自分のクリップだけ削除できます。".into()));
        }
        let mut url = endpoint(&self.firestore_base, &["imageCollectionEntries", id])?;
        url.query_pairs_mut()
            .append_pair("currentDocument.updateTime", &document.update_time);
        self.request_response(Method::DELETE, url, None::<&Value>, None)
            .await?
            .error_for_status()?;
        Ok(())
    }
}
pub fn clip_uri(info: &str, xywh: [u32; 4], version: u64) -> Result<String> {
    let service = info
        .strip_suffix("/info.json")
        .ok_or_else(|| Error::Invalid("invalid IIIF info URL".into()))?;
    let [x, y, w, h] = xywh;
    let size = if version == 3 { "max" } else { "full" };
    Ok(format!("{service}/{x},{y},{w},{h}/{size}/0/default.jpg"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn site_uri_preserves_query_based_services() -> Result<()> {
        assert_eq!(
            clip_uri(
                "https://example.org/image/?IIIF=/a%2Fb.tif/info.json",
                [1, 2, 30, 40],
                2
            )?,
            "https://example.org/image/?IIIF=/a%2Fb.tif/1,2,30,40/full/0/default.jpg"
        );
        assert_eq!(
            clip_uri("https://example.org/id/info.json", [1, 2, 30, 40], 3)?,
            "https://example.org/id/1,2,30,40/max/0/default.jpg"
        );
        Ok(())
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::{
        auth::{Session, SessionStore, TokenManager},
        cache::blocking,
    };
    use std::sync::Arc;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, method, path},
    };
    struct Memory;
    impl SessionStore for Memory {
        fn load(&self) -> Result<Option<Session>> {
            Ok(None)
        }
        fn save(&self, _: &Session) -> Result<()> {
            Ok(())
        }
        fn clear(&self) -> Result<()> {
            Ok(())
        }
    }
    async fn client(server: &MockServer) -> Result<HonkokuClient> {
        let session: Session = serde_json::from_value(
            json!({"uid":"me","apiKey":"test","email":null,"displayName":null,"refreshToken":"test","idToken":"test","expiresAt":"2099-01-01T00:00:00Z"}),
        )?;
        let client =
            HonkokuClient::with_endpoints(&server.uri(), &format!("{}/documents", server.uri()))?
                .with_session(TokenManager::new(session, Arc::new(Memory))?);
        blocking(&client.home_storage,|db| db.put_entry(&json!({"id":"e","projectId":"p","collectionId":"c","index":0,"manifestVersion":3,"tileSources":["https://example.org/id/info.json"],"canvases":[{"id":"canvas","width":1000,"height":1000}]}))).await?;
        Ok(client)
    }
    #[tokio::test]
    async fn exact_write_fields_list_order_and_owned_delete() -> Result<()> {
        let server = MockServer::start().await;
        let client = client(&server).await?;
        Mock::given(method("POST")).and(path("/documents:commit")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"commitTime":"2026-09-10T00:00:00Z","writeResults":[{"updateTime":"2026-09-10T00:00:00Z"}]}))).expect(1).mount(&server).await;
        let clip = client
            .create_clip(ClipInput {
                entry_id: "e".into(),
                index: 0,
                reading: " 候 ".into(),
                tags: vec![" Tag ".into(), "tag".into()],
                comment: " 注 ".into(),
                is_private: true,
                xywh: [1, 2, 30, 40],
            })
            .await?;
        let requests = server.received_requests().await.unwrap();
        let body: Value = serde_json::from_slice(&requests[0].body)?;
        let fields = &body["writes"][0]["update"]["fields"];
        let mut keys: Vec<_> = fields
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "comment",
                "entryId",
                "index",
                "isPrivate",
                "projectId",
                "reading",
                "tags",
                "transcriptionId",
                "uid",
                "uri",
                "xywh"
            ]
        );
        assert_eq!(
            body["writes"][0]["updateTransforms"],
            json!([{"fieldPath":"createdAt","setToServerValue":"REQUEST_TIME"}])
        );
        assert_eq!(
            clip.uri,
            "https://example.org/id/1,2,30,40/max/0/default.jpg"
        );
        assert_eq!(clip.input.tags, vec!["tag"]);
        let document = json!({"name":client.document_name(&format!("imageCollectionEntries/{}",clip.id))?,"updateTime":"2026-09-10T00:00:00Z","fields":{
            "entryId":{"stringValue":"e"},"index":{"integerValue":"0"},"reading":{"stringValue":"候"},"tags":{"arrayValue":{"values":[]}},"comment":{"stringValue":"注"},"isPrivate":{"booleanValue":true},"xywh":{"arrayValue":{"values":[{"integerValue":"1"},{"integerValue":"2"},{"integerValue":"30"},{"integerValue":"40"}]}},"uid":{"stringValue":"me"},"uri":{"stringValue":clip.uri},"transcriptionId":{"stringValue":"e_0"},"projectId":{"stringValue":"p"},"createdAt":{"timestampValue":"2026-09-10T00:00:00Z"}}});
        Mock::given(method("POST")).and(path("/documents:runQuery")).and(body_partial_json(json!({"structuredQuery":{"where":{"fieldFilter":{"field":{"fieldPath":"uid"},"op":"EQUAL","value":{"stringValue":"me"}}},"orderBy":[{"field":{"fieldPath":"createdAt"},"direction":"DESCENDING"}]}}))).respond_with(ResponseTemplate::new(200).set_body_json(json!([{"document":document}]))).expect(1).mount(&server).await;
        assert_eq!(client.clips().await?[0].id, clip.id);
        Mock::given(method("POST"))
            .and(path("/documents:batchGet"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"found":document}])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!(
                "/documents/imageCollectionEntries/{}",
                clip.id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        client.delete_clip(&clip.id).await?;
        Ok(())
    }
    #[tokio::test]
    async fn refuses_other_accounts_and_explains_missing_index() -> Result<()> {
        let server = MockServer::start().await;
        let client = client(&server).await?;
        Mock::given(method("POST")).and(path("/documents:batchGet")).respond_with(ResponseTemplate::new(200).set_body_json(json!([{"found":{"name":client.document_name("imageCollectionEntries/other")?,"updateTime":"2026-09-10T00:00:00Z","fields":{"uid":{"stringValue":"someone-else"}}}}]))).mount(&server).await;
        assert!(client.delete_clip("other").await.is_err());
        Mock::given(method("POST"))
            .and(path("/documents:runQuery"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!([{"error":{"message":"The query requires an index."}}])),
            )
            .mount(&server)
            .await;
        assert!(
            client
                .clips()
                .await
                .err()
                .unwrap()
                .to_string()
                .contains("索引がサーバーにありません")
        );
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|r| r.method != Method::DELETE)
        );
        Ok(())
    }
}
