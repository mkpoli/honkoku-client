use super::*;
use crate::{
    auth::{FileStore, Session, TokenManager},
    cache::SharedStorage,
};
use honkoku_storage::Storage;
use std::sync::{Arc, Mutex};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};

fn wire(value: Value) -> Value {
    match value {
        Value::Null => json!({"nullValue":null}),
        Value::Bool(v) => json!({"booleanValue":v}),
        Value::Number(v) => json!({"integerValue":v.to_string()}),
        Value::String(v) => json!({"stringValue":v}),
        Value::Array(v) => {
            json!({"arrayValue":{"values":v.into_iter().map(wire).collect::<Vec<_>>()}})
        }
        Value::Object(v) => {
            json!({"mapValue":{"fields":v.into_iter().map(|(k,v)| (k,wire(v))).collect::<serde_json::Map<_,_>>()}})
        }
    }
}
fn doc(path: &str, value: Value) -> Value {
    json!({"name":format!("projects/test/databases/(default)/documents/{path}"),"fields":wire(value)["mapValue"]["fields"], "updateTime":"2026-09-10T00:00:00Z"})
}
fn event(id: &str, day: u32) -> Value {
    doc(
        &format!("timelineEvents/{id}"),
        json!({"uid":"u", "projectId":"p", "entryId":"e", "transcriptionId":"e_0", "index":0, "eventType":"transcription", "count":2, "isReview":false,"share":true,"createdAt":format!("2026-09-{day:02}T00:00:00Z"), "data":{"text":"《圏点：日本｜・》【注釈】"},"future":true}),
    )
}
fn client(server: &MockServer) -> Result<HonkokuClient> {
    HonkokuClient::with_endpoints(&server.uri(), &format!("{}/documents", server.uri()))
}
#[tokio::test]
async fn timeline_enrichment_deduplicates_and_caches_documents() -> Result<()> {
    let server = MockServer::start().await;
    let db: SharedStorage = Arc::new(Mutex::new(Storage::in_memory()?));
    let client = client(&server)?.with_storage(db.clone());
    Mock::given(method("POST")).and(path("/documents:runQuery"))
        .and(body_json(json!({"structuredQuery":{"from":[{"collectionId":"timelineEvents"}],"orderBy":[{"field":{"fieldPath":"createdAt"},"direction":"DESCENDING"}],"limit":20,"where":{"compositeFilter":{"op":"AND","filters":[{"fieldFilter":{"field":{"fieldPath":"projectType"},"op":"IN","value":{"arrayValue":{"values":[{"stringValue":"official"},{"stringValue":"user"}]}}}},{"fieldFilter":{"field":{"fieldPath":"eventType"},"op":"EQUAL","value":{"stringValue":"transcription"}}},{"fieldFilter":{"field":{"fieldPath":"share"},"op":"EQUAL","value":{"booleanValue":true}}}]}}}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"document":event("a",9)},{"document":event("b",10)}])))
        .expect(3).mount(&server).await;
    for (path_name, value) in [
        (
            "users/u",
            json!({"uid":"u","displayName":"利用者","photoURL":"https://example.test/photo", "exp":9,"newUserField":true}),
        ),
        ("projects/p", json!({"title":"計画"})),
        (
            "entries/e",
            json!({"projectId":"p","collectionId":"c","index":0,"label":{"ja":["資料"]},"manifestUrl":"https://example.test/manifest"}),
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/documents/{path_name}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(doc(path_name, value)))
            .expect(2)
            .mount(&server)
            .await;
    }
    let items = client.timeline(TimelineFilter::All, None).await?;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].event.id, "b");
    assert_eq!(items[0].excerpt, "日本");
    assert_eq!(
        items[0].actor.as_ref().and_then(|u| u.photo_url.as_deref()),
        Some("https://example.test/photo")
    );
    assert_eq!(items[0].project_title.as_deref(), Some("計画"));
    assert_eq!(
        items[0].entry_label.as_ref().map(|l| l.preferred(&["ja"])),
        Some("資料".into())
    );
    client.timeline(TimelineFilter::All, None).await?;
    client
        .timeline_with_refresh(TimelineFilter::All, None, true)
        .await?;
    crate::cache::blocking(&db, |db| {
        assert!(db.get_user::<User>("u")?.is_some());
        assert_eq!(db.list_timeline_events::<TimelineEvent>("p")?.len(), 2);
        assert!(db.get_entry::<Entry>("e")?.is_some());
        assert!(db.get_project::<Project>("p")?.is_some());
        Ok(())
    })
    .await?;
    Ok(())
}
#[test]
fn joined_queries_chunk_thirty_ids_without_multiplying_disjunctions() {
    let mut ids: Vec<_> = (0..61).map(|i| format!("p{i:02}")).collect();
    ids.push("p00".into());
    let queries = timeline_queries(TimelineFilter::Joined(ids), 5);
    assert_eq!(queries.len(), 6);
    let sizes: Vec<_> = queries.iter().map(|q| q["where"]["compositeFilter"]["filters"][1]["fieldFilter"]["value"]["arrayValue"]["values"].as_array().map(Vec::len)).collect();
    assert_eq!(
        sizes,
        [Some(30), Some(30), Some(30), Some(30), Some(1), Some(1)]
    );
    for query in queries {
        assert_eq!(
            query["where"]["compositeFilter"]["filters"][0]["fieldFilter"]["op"],
            "EQUAL"
        );
        assert_eq!(query["limit"], 5);
    }
    assert!(timeline_queries(TimelineFilter::Joined(vec![]), 5).is_empty());
    let project = timeline_queries(TimelineFilter::Project("ainu".into()), 5);
    assert_eq!(
        project[0]["where"]["compositeFilter"]["filters"][1],
        equal("projectId", json!({"stringValue":"ainu"}))
    );
}
#[tokio::test]
async fn joined_feed_merges_global_limit_and_handles_deleted_joins() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let queries = timeline_queries(TimelineFilter::Joined(vec!["p".into()]), 2);
    for (query, events) in queries.into_iter().zip([
        vec![event("a", 8), event("b", 9)],
        vec![event("c", 10), event("b", 9)],
    ]) {
        Mock::given(method("POST"))
            .and(body_json(json!({"structuredQuery":query})))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    events
                        .into_iter()
                        .map(|d| json!({"document":d}))
                        .collect::<Vec<_>>(),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .expect(3)
        .mount(&server)
        .await;
    let items = client
        .timeline(TimelineFilter::Joined(vec!["p".into()]), Some(2))
        .await?;
    assert_eq!(
        items
            .iter()
            .map(|i| i.event.id.as_str())
            .collect::<Vec<_>>(),
        ["c", "b"]
    );
    assert!(
        items
            .iter()
            .all(|i| i.actor.is_none() && i.entry_label.is_none() && i.project_title.is_none())
    );
    assert!(
        client
            .timeline(TimelineFilter::Joined(vec![]), None)
            .await?
            .is_empty()
    );
    assert!(
        client
            .timeline(TimelineFilter::All, Some(0))
            .await?
            .is_empty()
    );
    Ok(())
}
#[tokio::test]
async fn home_default_queries_and_ranking_sorts() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let mut announcements = ordered("adminAnnouncements", "createdAt", 5);
    announcements["where"] = equal("display", json!({"booleanValue":true}));
    for query in [
        announcements,
        ordered("dailyProgress", "timestamp", 2),
        ordered("users", "exp", 100),
        ordered("users", "charCount", 100),
        ordered("users", "likeCount", 100),
    ] {
        Mock::given(method("POST"))
            .and(body_json(json!({"structuredQuery":query})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([{"readTime":"2026-09-10T00:00:00Z"}])),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    assert!(client.announcements(None).await?.is_empty());
    assert!(client.daily_progress(None).await?.is_empty());
    for sort in [
        RankingSort::Exp,
        RankingSort::CharCount,
        RankingSort::LikeCount,
    ] {
        assert!(client.ranking(sort, None).await?.is_empty());
    }
    Ok(())
}
#[tokio::test]
async fn session_bearer_reaches_firestore_search_and_callables() -> Result<()> {
    let server = MockServer::start().await;
    let mut session: Session =
        serde_json::from_str(include_str!("../auth/dev-session.fixture.json"))?;
    session.expires_at.0 = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    let dir = tempfile::tempdir()?;
    let manager = TokenManager::new(
        session,
        Arc::new(FileStore::new(dir.path().join("session.json"))),
    )?;
    let mut client = client(&server)?.with_session(manager);
    client.search_base = format!("{}/search", server.uri());
    client.functions_base = format!("{}/functions", server.uri());
    for (path_name, body) in [
        (
            "/documents/users/fixture-user",
            doc(
                "users/fixture-user",
                json!({"displayName":"利用者", "uid":"fixture-user", "level":1,"exp":2,"charCount":3,"likeCount":4,"stoneCount":5,"photoURL":null,"profile":"","updatedAt":"2026-09-10T00:00:00Z","future":true}),
            ),
        ),
        ("/search", json!({"results":[]})),
        ("/functions/test", json!({"result":{"ok":true}})),
    ] {
        Mock::given(path(path_name))
            .and(header("authorization", "Bearer fixture-id-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("POST")).and(path("/documents:runAggregationQuery")).and(header("authorization","Bearer fixture-id-token"))
        .and(body_json(json!({"structuredAggregationQuery":{"structuredQuery":{"from":[{"collectionId":"notifications"}],"where":{"compositeFilter":{"op":"AND","filters":[equal("uid",json!({"stringValue":"fixture-user"})),equal("state",json!({"stringValue":"unchecked"}))]}}},"aggregations":[{"alias":"count","count":{}}]}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"result":{"aggregateFields":{"count":{"integerValue":"7"}}}}]))).expect(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/documents:runQuery")).and(header("authorization","Bearer fixture-id-token"))
        .and(body_json(json!({"structuredQuery":{"from":[{"collectionId":"notifications"}],"where":equal("uid",json!({"stringValue":"fixture-user"})),"orderBy":[{"field":{"fieldPath":"createdAt"},"direction":"DESCENDING"}],"limit":10}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"document":doc("notifications/n",json!({"uid":"fixture-user","type":"future-kind","data":{"unknown":[null,1]},"state":"unchecked","createdAt":"2026-09-10T00:00:00Z","likerUid":"x"}))}]))).expect(1).mount(&server).await;
    let user = client.me().await?;
    assert_eq!(user.char_count, Some(3));
    assert_eq!(user.extra["future"], true);
    assert_eq!(client.unread_notification_count().await?, 7);
    let notices = client.notifications(10).await?;
    assert_eq!(notices[0].kind, "future-kind");
    assert_eq!(notices[0].extra["likerUid"], "x");
    assert_eq!(notices[0].data, json!({"unknown":[null,1]}));
    let _: Value = client.search(&[("keyword", "日本")]).await?;
    let callable: Value = client.callable("test", &json!({"a":1})).await?;
    assert_eq!(callable, json!({"ok":true}));
    Ok(())
}
#[tokio::test]
async fn private_reads_require_session_and_errors_are_not_empty_counts() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    assert!(matches!(client.me().await, Err(Error::SignedOut)));
    assert!(matches!(
        client.notifications(10).await,
        Err(Error::SignedOut)
    ));
    assert!(matches!(
        client.unread_notification_count().await,
        Err(Error::SignedOut)
    ));
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"error":{"message":"denied"}}])),
        )
        .expect(2)
        .mount(&server)
        .await;
    assert!(client.run_query::<Value>(json!({})).await.is_err());
    assert!(
        client
            .run_aggregation_count("notifications", vec![])
            .await
            .is_err()
    );
    Ok(())
}

#[test]
fn daily_progress_preserves_non_finite_project_counts() -> Result<()> {
    let value = json!({"id":"2026-09-09","timestamp":"2026-09-09T14:02:48.343Z","totalEntryCount":9646,"completedEntryCount":4602,"totalImageCount":268002,"completedImageCount":119264,"initiatedImageCount":15981,"defaultImageCount":135891,"charCount":60367944,"userCount":1988,"projects":{"kirishitan":{"totalEntryCount":1,"completedEntryCount":0,"totalImageCount":2,"completedImageCount":"NaN","charCount":1,"participants":{"u":true},"future":1}},"futureSiteField":true});
    let progress: DailyProgress = serde_json::from_value(value.clone())?;
    assert_eq!(
        progress.projects["kirishitan"].completed_image_count,
        crate::model::ProgressCount::NonFinite(crate::model::NonFiniteNumber::Nan)
    );
    assert_eq!(serde_json::to_value(progress)?, value);
    Ok(())
}

#[tokio::test]
#[ignore = "reads the live public Firestore service"]
async fn live_public_home_contract() -> Result<()> {
    let client = HonkokuClient::new()?;
    let progress = client.daily_progress(None).await?;
    assert_eq!(progress.len(), 2);
    println!(
        "dailyProgress: {} records; {} projects in latest ({})",
        progress.len(),
        progress[0].projects.len(),
        progress[0].id
    );
    let mut projects = vec!["ainu".into()];
    projects.extend((0..30).map(|i| format!("missing-project-{i}")));
    let items = client
        .timeline(TimelineFilter::Joined(projects), Some(5))
        .await?;
    assert_eq!(items.len(), 5);
    assert!(items.iter().all(|i| i.event.project_id == "ainu"));
    println!("joined timeline: {} rows from 31 project IDs", items.len());
    Ok(())
}

fn signed_in(client: HonkokuClient, uid: &str, dir: &std::path::Path) -> Result<HonkokuClient> {
    let mut session: Session =
        serde_json::from_str(include_str!("../auth/dev-session.fixture.json"))?;
    session.uid = uid.into();
    session.expires_at.0 = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    Ok(client.with_session(TokenManager::new(
        session,
        Arc::new(FileStore::new(dir.join("session.json"))),
    )?))
}

#[tokio::test]
async fn self_ranking_counts_strictly_greater_and_caches_per_account_and_sort() -> Result<()> {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir()?;
    let db: SharedStorage = Arc::new(Mutex::new(Storage::in_memory()?));
    for (uid, value, count) in [("first", 30, 186), ("second", 0, 0)] {
        let client = signed_in(client(&server)?.with_storage(db.clone()), uid, dir.path())?;
        Mock::given(method("GET"))
            .and(path(format!("/documents/users/{uid}")))
            .and(header("authorization", "Bearer fixture-id-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(doc(
                &format!("users/{uid}"),
                json!({"exp":value,"charCount":value,"likeCount":value}),
            )))
            .expect(3)
            .mount(&server)
            .await;
        for sort in [
            RankingSort::Exp,
            RankingSort::CharCount,
            RankingSort::LikeCount,
        ] {
            Mock::given(method("POST"))
                .and(path("/documents:runAggregationQuery"))
                .and(header("authorization", "Bearer fixture-id-token"))
                .and(body_json(json!({
                    "structuredAggregationQuery": {
                        "structuredQuery": {
                            "from": [{"collectionId":"users"}],
                            "where": {"compositeFilter": {"op":"AND","filters":[{"fieldFilter": {
                                "field": {"fieldPath":sort.field()},
                                "op":"GREATER_THAN",
                                "value":{"integerValue":value.to_string()}
                            }}]}}
                        },
                        "aggregations":[{"alias":"count","count":{}}]
                    }
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                    {"result":{"aggregateFields":{"count":{"integerValue":count.to_string()}}}}
                ])))
                .expect(1)
                .mount(&server)
                .await;
            let expected = RankingSelf {
                rank: Some(count + 1),
                value: Some(value),
            };
            assert_eq!(client.ranking_self(sort).await?, expected);
            assert_eq!(client.ranking_self(sort).await?, expected);
            let key = format!("ranking-self/{uid}/{}", sort.field());
            crate::cache::blocking(&db, move |db| {
                assert_eq!(
                    db.fresh_response::<RankingSelf>(&key, std::time::Duration::from_secs(600))?,
                    Some(expected)
                );
                Ok(())
            })
            .await?;
        }
    }
    assert!(matches!(
        client(&server)?
            .with_storage(db)
            .ranking_self(RankingSort::Exp)
            .await,
        Err(Error::SignedOut)
    ));
    Ok(())
}

#[tokio::test]
async fn self_ranking_missing_field_is_cached_without_aggregation() -> Result<()> {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir()?;
    let client = signed_in(client(&server)?, "new", dir.path())?;
    Mock::given(method("GET"))
        .and(path("/documents/users/new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(doc("users/new", json!({}))))
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    for sort in [
        RankingSort::Exp,
        RankingSort::CharCount,
        RankingSort::LikeCount,
    ] {
        for _ in 0..2 {
            let result = client.ranking_self(sort).await?;
            assert_eq!(
                serde_json::to_value(result)?,
                json!({"rank":null,"value":null})
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn self_ranking_retries_after_aggregation_failure() -> Result<()> {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir()?;
    let client = signed_in(client(&server)?, "u", dir.path())?;
    Mock::given(method("GET"))
        .and(path("/documents/users/u"))
        .respond_with(ResponseTemplate::new(200).set_body_json(doc("users/u", json!({"exp":5}))))
        .expect(2)
        .mount(&server)
        .await;
    let failure = Mock::given(method("POST"))
        .and(path("/documents:runAggregationQuery"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    assert!(client.ranking_self(RankingSort::Exp).await.is_err());
    drop(failure);
    Mock::given(method("POST"))
        .and(path("/documents:runAggregationQuery"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"result":{"aggregateFields":{"count":{"integerValue":"100"}}}}
        ])))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        client.ranking_self(RankingSort::Exp).await?,
        RankingSelf {
            rank: Some(101),
            value: Some(5)
        }
    );
    Ok(())
}
