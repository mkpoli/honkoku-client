use super::*;
use crate::{
    cache::SharedStorage,
    model::{Collection, Entry, Page, PageStatus, Project, Timestamp},
};
use honkoku_storage::Storage;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, header, method, path},
};

#[test]
fn every_captured_fixture() -> Result<()> {
    let projects: Vec<Project> =
        serde_json::from_str(include_str!("../../../fixtures/api/projects.json"))?;
    assert!(!projects.is_empty());
    let project: Project =
        serde_json::from_str(include_str!("../../../fixtures/api/project-ainu.json"))?;
    assert_eq!(project.id, "ainu");
    assert!(project.collections.is_some());
    assert!(project.extra.contains_key("isPrivate"));
    let collection: Collection = serde_json::from_str(include_str!(
        "../../../fixtures/api/collection-3R4VhlBfvOYeqPY13cJm.json"
    ))?;
    assert_eq!(collection.entries.as_ref().map(Vec::len), Some(3));
    let entry: Entry = serde_json::from_str(include_str!(
        "../../../fixtures/api/entry-0916dafb80cdc48ca7687afcad4a4f35.json"
    ))?;
    assert_eq!(entry.canvases.as_ref().map(Vec::len), Some(18));
    assert_eq!(entry.transcriptions.as_ref().map(Vec::len), Some(18));
    let encoded = serde_json::to_value(&entry)?;
    assert!(
        encoded["canvases"][0]["infoJsonUrl"]
            .as_str()
            .is_some_and(|url| url.contains("%2F"))
    );
    assert_eq!(serde_json::from_value::<Entry>(encoded)?, entry);
    let response: Value = serde_json::from_str(include_str!(
        "../../../fixtures/api/firestore-pages-0916dafb.json"
    ))?;
    let pages = firestore::decode_pages(&response)?;
    assert_eq!(pages.len(), 5);
    for page in pages {
        assert_eq!(page.id, format!("{}_{}", page.entry_id, page.index));
        assert!(page.extra.contains_key("_firestore"));
        assert_eq!(
            serde_json::from_value::<Page>(serde_json::to_value(&page)?)?,
            page
        );
    }
    Ok(())
}
#[test]
fn timestamps_unknown_status_and_nullable_notes() -> Result<()> {
    let a: Timestamp = serde_json::from_value(json!({"_seconds":1,"_nanoseconds":123456789}))?;
    let b: Timestamp = serde_json::from_value(json!("1970-01-01T00:00:01.123456789Z"))?;
    assert_eq!(a, b);
    assert_eq!(
        serde_json::to_value(a)?,
        json!("1970-01-01T00:00:01.123456789Z")
    );
    assert!(
        serde_json::from_value::<Timestamp>(json!({"_seconds":0,"_nanoseconds":1000000000}))
            .is_err()
    );
    for status in [
        "default",
        "initiated",
        "editing",
        "completed",
        "frozen",
        "future-state",
    ] {
        let decoded: PageStatus = serde_json::from_value(json!(status))?;
        assert_eq!(serde_json::to_value(decoded)?, json!(status));
    }
    assert_eq!(
        serde_json::from_value::<PageStatus>(json!("future-state"))?,
        PageStatus::Unknown("future-state".into())
    );
    let value = json!({"id":"e_0","entryId":"e","index":0,"status":"default","text":"","notes":[null,{"unknown":1}],"tempNotes":[null],"newField":{"nested":[1,null]}});
    let page: Page = serde_json::from_value(value.clone())?;
    assert_eq!(serde_json::to_value(page)?, value);
    assert_eq!(
        firestore::decode_value(
            &json!({"mapValue":{"fields":{"a":{"arrayValue":{"values":[{"nullValue":null},{"integerValue":"9223372036854775807"}]}}}}})
        )?,
        json!({"a":[null,i64::MAX]})
    );
    assert!(firestore::decode_value(&json!({"mysteryValue":1})).is_err());
    Ok(())
}
fn document(index: u32) -> Value {
    json!({"document":{"name":format!("projects/test/databases/(default)/documents/transcriptions/e_{index}"),"fields":{"entryId":{"stringValue":"e"},"index":{"integerValue":index.to_string()},"status":{"stringValue":"default"},"text":{"stringValue":"本文"},"notes":{"arrayValue":{}}}}})
}
#[tokio::test]
async fn firestore_pagination_and_bearer() -> Result<()> {
    let server = MockServer::start().await;
    let first: Vec<_> = (0..200).map(document).collect();
    Mock::given(method("POST")).and(path("/documents:runQuery")).and(header("authorization","Bearer test-token")).and(body_partial_json(json!({"structuredQuery":{"startAt":{"values":[{"integerValue":"199"},{"referenceValue":"projects/test/databases/(default)/documents/transcriptions/e_199"}],"before":false}}}))).respond_with(ResponseTemplate::new(200).set_body_json(vec![document(200)])).expect(1).with_priority(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/documents:runQuery")).and(body_partial_json(json!({"structuredQuery":{"limit":200,"where":{"fieldFilter":{"value":{"stringValue":"e"}}}}}))).respond_with(ResponseTemplate::new(200).set_body_json(first)).expect(1).with_priority(2).mount(&server).await;
    let client =
        HonkokuClient::with_endpoints(&server.uri(), &format!("{}/documents", server.uri()))?;
    let pages = client.pages("e", Some("test-token")).await?;
    assert_eq!(pages.len(), 201);
    assert_eq!(pages[200].index, 200);
    Ok(())
}
#[tokio::test]
async fn fresh_cache_refresh_and_summary_detail_separation() -> Result<()> {
    let server = MockServer::start().await;
    let client = HonkokuClient::with_endpoints(&server.uri(), &server.uri())?;
    let storage: SharedStorage = Arc::new(Mutex::new(Storage::in_memory()?));
    Mock::given(method("GET"))
        .and(path("/projects"))
        .and(header(
            "user-agent",
            "honkoku-client/0.1 (+https://github.com/mkpoli/honkoku-client)",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"id":"p","title":"summary"}])),
        )
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/projects/p"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id":"p","title":"detail","collections":[]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    client.cached_projects(&storage, false).await?;
    client.cached_projects(&storage, false).await?;
    client.cached_project(&storage, "p", false).await?;
    client.cached_projects(&storage, true).await?;
    assert_eq!(
        client
            .cached_project(&storage, "p", false)
            .await?
            .collections,
        Some(vec![])
    );
    assert_eq!(
        client.cached_collections(&storage, "p", false).await?,
        vec![]
    );
    Ok(())
}
#[tokio::test]
async fn failures_are_not_cached_as_empty_results() -> Result<()> {
    let server = MockServer::start().await;
    let client = HonkokuClient::with_endpoints(&server.uri(), &server.uri())?;
    let storage: SharedStorage = Arc::new(Mutex::new(Storage::in_memory()?));
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&server)
        .await;
    assert!(client.cached_projects(&storage, false).await.is_err());
    assert!(client.cached_projects(&storage, false).await.is_err());
    Ok(())
}

#[tokio::test]
async fn per_host_requests_are_bounded() -> Result<()> {
    use futures_util::future::try_join_all;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([]))
                .set_delay(Duration::from_millis(100)),
        )
        .expect(9)
        .mount(&server)
        .await;
    let client = HonkokuClient::with_endpoints(&server.uri(), &server.uri())?;
    let started = std::time::Instant::now();
    try_join_all((0..9).map(|_| client.projects())).await?;
    // Nine requests require three batches through a four-per-host semaphore.
    assert!(started.elapsed() >= Duration::from_millis(290));
    Ok(())
}
