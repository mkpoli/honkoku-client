use super::*;
use crate::firestore::encode_value;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{method, path},
};

async fn mocked() -> (MockServer, HonkokuClient) {
    let server = MockServer::start().await;
    let client =
        HonkokuClient::with_endpoints(&server.uri(), &format!("{}/documents", server.uri()))
            .unwrap();
    Mock::given(method("POST")).and(path("/documents:runAggregationQuery"))
        .respond_with(|req: &Request| {
            let body: Value = req.body_json().unwrap();
            let filters = &body["structuredAggregationQuery"]["structuredQuery"]["where"]["compositeFilter"]["filters"];
            let ids = filters[0]["fieldFilter"]["value"]["arrayValue"]["values"].as_array().unwrap();
            assert!(!ids.is_empty() && ids.len() <= 30);
            assert_eq!(filters[0]["fieldFilter"]["op"], "IN");
            let status = filters[1]["fieldFilter"]["value"]["stringValue"].as_str().unwrap();
            let count = match status { "completed" => 3, "initiated" => 2, "editing" => 1, _ => panic!("unexpected status") } * ids.len();
            ResponseTemplate::new(200).set_body_json(json!([{"result":{"aggregateFields":{"count":{"integerValue":count.to_string()}}}}]))
        }).mount(&server).await;
    (server, client)
}
#[tokio::test]
async fn batches_thirty_ids_and_exact_entry_counts() {
    let (server, client) = mocked().await;
    let mut ids: Vec<_> = (0..61).map(|i| format!("e{i}")).collect();
    ids.push("e0".into());
    let counts = client.collection_status_counts(&ids).await.unwrap();
    assert_eq!(
        (counts.completed, counts.initiated, counts.editing),
        (183, 122, 61)
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 9);
    let exact = client.page_status_counts(&ids[..5]).await.unwrap();
    assert_eq!(exact.len(), 5);
    assert!(
        exact
            .values()
            .all(|c| c.completed == 3 && c.initiated == 2 && c.editing == 1)
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 24);
    assert_eq!(
        client.collection_status_counts(&[]).await.unwrap(),
        Default::default()
    );
    assert!(client.entry_progress(&[]).await.unwrap().is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 24);
}
fn query_response(entries: &[Value]) -> Value {
    json!(entries.iter().map(|e| json!({"document":{
        "name":format!("projects/test/databases/(default)/documents/entries/{}", e["id"].as_str().unwrap()),
        "fields":encode_value(e)["mapValue"]["fields"]
    }})).collect::<Vec<_>>())
}
#[tokio::test]
async fn cold_ainu_progress_request_count_and_warm_cache() {
    let (server, client) = mocked().await;
    let entries: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/api/entry-summaries-ainu.json"
    ))
    .unwrap();
    let collections: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/home/collection-progress-ainu.json"
    ))
    .unwrap();
    let source = entries.clone();
    Mock::given(method("POST"))
        .and(path("/documents:runQuery"))
        .respond_with(move |req: &Request| {
            let body: Value = req.body_json().unwrap();
            assert_eq!(
                body["structuredQuery"]["orderBy"][0]["field"]["fieldPath"],
                "index"
            );
            assert_eq!(
                body["structuredQuery"]["from"][0]["collectionId"],
                "entries"
            );
            let selected: Vec<_> = body["structuredQuery"]["select"]["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["fieldPath"].as_str().unwrap())
                .collect();
            assert_eq!(
                selected,
                [
                    "projectId",
                    "collectionId",
                    "index",
                    "label",
                    "manifestUrl",
                    "thumbnail",
                    "size",
                    "createdAt"
                ]
            );
            let id = &body["structuredQuery"]["where"]["fieldFilter"]["value"]["stringValue"];
            let mut rows: Vec<_> = source
                .iter()
                .filter(|e| &e["collectionId"] == id)
                .cloned()
                .collect();
            rows.sort_by_key(|e| e["index"].as_u64());
            ResponseTemplate::new(200).set_body_json(query_response(&rows))
        })
        .mount(&server)
        .await;
    let selected = collections[0]["collectionId"].as_str().unwrap();
    let ids: Vec<_> = entries
        .iter()
        .filter(|e| e["collectionId"] == selected)
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    let load = || async {
        let all = stream::iter(&collections)
            .map(|c| client.collection_progress(c["collectionId"].as_str().unwrap(), false))
            .buffered(4)
            .try_collect::<Vec<_>>();
        let selected_rows = async {
            let rows = client.list_entries(selected).await?;
            assert_eq!(rows.len(), ids.len());
            client.entry_progress(&ids).await
        };
        tokio::try_join!(all, selected_rows)
    };
    let (progress, exact) = load().await.unwrap();
    assert_eq!(progress.len(), 67);
    assert_eq!(exact.len(), 2);
    assert_eq!(progress.iter().map(|p| p.size).sum::<u64>(), 5573);
    let cold = server.received_requests().await.unwrap();
    assert_eq!(
        cold.len(),
        271,
        "67 listings + 198 collection aggregations + 6 selected-entry aggregations"
    );
    assert!(
        cold.iter()
            .all(|r| !r.headers.contains_key("authorization"))
    );
    load().await.unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 271);
    client.collection_progress(selected, true).await.unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 275);
    client.entry_progress_with_refresh(&[], true).await.unwrap();
}

#[tokio::test]
async fn entry_refresh_bypasses_summary_and_count_cache() {
    let (server, client) = mocked().await;
    let value = json!({"id":"e","projectId":"p","collectionId":"c","index":2,"label":{"ja":["資料"]},"manifestUrl":"https://example.test/manifest","size":10,"future":true});
    let response = query_response(&[value]);
    Mock::given(method("GET"))
        .and(path("/documents/entries/e"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response[0]["document"]))
        .mount(&server)
        .await;
    let ids = ["e".into(), "e".into()];
    let first = client.entry_progress(&ids).await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].size, 10);
    assert_eq!(first[0].counts.editing, 1);
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
    assert_eq!(client.entry_progress(&ids).await.unwrap(), first);
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
    client
        .entry_progress_with_refresh(&ids, true)
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 8);
}
