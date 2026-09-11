use super::*;

#[test]
fn added_characters() {
    for (old, new, count) in [
        ("", "汐干（しほひ）　", 7),
        ("同じ", "同じ", 0),
        ("消す", "", 0),
        ("前後", "前挿入後", 2),
        ("前後", "前　 \n\t\u{feff}後", 0),
    ] {
        assert_eq!(added_character_count(old, new), count);
    }
}

use crate::{
    auth::{FileStore, Session, TokenManager},
    firestore::{decode_value, encode_value},
    model::Timestamp,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};
const ENTRY: &str = "90aaa0afa9cf2f14fc579f138ed2fca9";
const UID: &str = "B5KzM2KmjxXlvUVHiQdEO6Xth9J2";
fn fixture() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../fixtures/api/edit-cycle-commits.json"
    ))
    .unwrap()
}
fn page_reads() -> Vec<Value> {
    fixture()
        .iter()
        .filter(|r| r["kind"] == "response" && r["url"].as_str().unwrap().ends_with(":batchGet"))
        .filter_map(|r| r["body"][0].get("found"))
        .filter(|d| d["name"].as_str().unwrap().contains("/transcriptions/"))
        .cloned()
        .collect()
}
fn project_read() -> Value {
    fixture()
        .iter()
        .filter_map(|r| r["body"][0].get("found"))
        .find(|d| d["name"].as_str().unwrap().ends_with("/projects/ainu"))
        .unwrap()
        .clone()
}
fn commits(kind: &str) -> Vec<Value> {
    fixture()
        .into_iter()
        .filter(|r| r["kind"] == kind && r["url"].as_str().unwrap().ends_with(":commit"))
        .map(|r| r["body"].clone())
        .collect()
}
fn client(server: &MockServer) -> Result<HonkokuClient> {
    let session = Session {
        api_key: "test-key".into(),
        uid: UID.into(),
        email: None,
        display_name: None,
        providers: vec![],
        refresh_token: "test-refresh".into(),
        id_token: "test-token".into(),
        expires_at: Timestamp(time::OffsetDateTime::now_utc() + time::Duration::hours(1)),
    };
    Ok(
        HonkokuClient::with_endpoints(&server.uri(), &format!("{}/documents", server.uri()))?
            .with_session(TokenManager::new(
                session,
                Arc::new(FileStore::new("/tmp/unused-editing-test-session")),
            )?),
    )
}
async fn read(server: &MockServer, document: Value, times: u64) {
    Mock::given(method("POST"))
        .and(path("/documents:batchGet"))
        .and(header("authorization", "Bearer test-token"))
        .and(body_json(json!({"documents":[document["name"]]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"found":document}])))
        .expect(times)
        .mount(server)
        .await;
}
async fn commit_response(server: &MockServer, body: Value) {
    Mock::given(method("POST"))
        .and(path("/documents:commit"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(server)
        .await;
}
fn normalized(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(key.as_str(), "timestampValue" | "updateTime") {
                    let t: Timestamp = serde_json::from_value(value.clone()).unwrap();
                    *value = serde_json::to_value(t).unwrap();
                } else if key == "name"
                    && value
                        .as_str()
                        .is_some_and(|s| s.contains("/timelineEvents/"))
                {
                    let name = value.as_str().unwrap();
                    let (prefix, id) = name.rsplit_once('/').unwrap();
                    assert_eq!(id.len(), 20);
                    assert!(id.chars().all(|ch| ch.is_ascii_alphanumeric()));
                    *value = json!(format!("{prefix}/AUTO_ID"));
                } else {
                    *value = normalized(value.clone());
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                *value = normalized(value.clone());
            }
        }
        _ => {}
    }
    value
}
async fn assert_commit(server: &MockServer, expected: Value) {
    let requests = server.received_requests().await.unwrap();
    let commits: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().ends_with(":commit"))
        .collect();
    assert_eq!(commits.len(), 1);
    let actual: Value = serde_json::from_slice(&commits[0].body).unwrap();
    assert_eq!(normalized(actual), normalized(expected));
    server.verify().await;
}
#[tokio::test]
async fn captured_lock_drafts_save_requests_and_local_state() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let pages = page_reads();
    let requests = commits("request");
    let responses = commits("response");
    read(&server, pages[0].clone(), 1).await;
    read(&server, project_read(), 1).await;
    commit_response(&server, responses[0].clone()).await;
    let mut session = client.lock_page(ENTRY, 20, false).await?;
    assert_eq!(session.page().status, PageStatus::Editing);
    assert_eq!(
        session.update_time(),
        responses[0]["writeResults"][0]["updateTime"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        session.page().updated_at.as_ref().unwrap().0,
        serde_json::from_value::<Timestamp>(
            responses[0]["writeResults"][0]["transformResults"][0]["timestampValue"].clone()
        )?
        .0
    );
    assert_commit(&server, requests[0].clone()).await;
    for index in 1..=2 {
        server.reset().await;
        read(&server, pages[index].clone(), 1).await;
        commit_response(&server, responses[index].clone()).await;
        let text = requests[index]["writes"][0]["update"]["fields"]["tempText"]["stringValue"]
            .as_str()
            .unwrap();
        session.draft_now(text).await?;
        assert_eq!(session.page().temp_text.as_deref(), Some(text));
        assert_eq!(session.page().temp_text_changed, Some(true));
        assert_eq!(
            session.update_time(),
            responses[index]["writeResults"][0]["updateTime"]
                .as_str()
                .unwrap()
        );
        let mut expected = requests[index].clone();
        expected["writes"][0]["update"]["fields"]["tempNotes"] =
            encode_value(&decode_value(&pages[index]["fields"]["tempNotes"])?);
        expected["writes"][0]["updateMask"]["fieldPaths"] =
            json!(["tempNotes", "tempText", "tempTextChanged"]);
        assert_commit(&server, expected).await;
    }
    server.reset().await;
    read(&server, pages[3].clone(), 1).await;
    read(&server, project_read(), 1).await;
    commit_response(&server, responses[3].clone()).await;
    let saved = session.save(SaveOptions::default()).await?;
    assert_eq!(saved.page.text, "汐干（しほひ）　");
    assert_eq!(saved.page.status, PageStatus::Initiated);
    assert_eq!(
        session.update_time(),
        responses[3]["writeResults"][0]["updateTime"]
            .as_str()
            .unwrap()
    );
    assert_commit(&server, requests[3].clone()).await;
    assert!(session.draft("late").await.is_err());
    Ok(())
}
#[tokio::test]
async fn discard_only_restores_previous_status() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let page = page_reads()[3].clone();
    read(&server, page.clone(), 2).await;
    commit_response(&server, commits("response")[3].clone()).await;
    client.resume_editing(ENTRY, 20).await?.discard().await?;
    assert_commit(&server,json!({"writes":[{"update":{"name":page["name"],"fields":{"status":{"stringValue":"default"}}},"updateMask":{"fieldPaths":["status"]},"currentDocument":{"updateTime":page["updateTime"]}}]})).await;
    Ok(())
}
#[tokio::test]
async fn conflict_rereads_page_once_and_retains_nanoseconds() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let mut page = page_reads()[1].clone();
    page["updateTime"] = json!("2026-09-10T07:40:55.669759001Z");
    read(&server, page.clone(), 3).await;
    Mock::given(path("/documents:commit"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(
                json!({"error":{"status":"FAILED_PRECONDITION","message":"changed"}}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    match session.draft_now("conflicting").await {
        Err(Error::Conflict {
            path,
            current: Some(current),
        }) => {
            assert_eq!(path, page["name"]);
            assert_eq!(current.update_time, "2026-09-10T07:40:55.669759001Z");
            assert_eq!(current.fields["status"], "editing");
        }
        other => panic!("unexpected result: {other:?}"),
    }
    let requests = server.received_requests().await.unwrap();
    let write: Value = serde_json::from_slice(
        &requests
            .iter()
            .find(|r| r.url.path().ends_with(":commit"))
            .unwrap()
            .body,
    )?;
    assert_eq!(
        write["writes"][0]["currentDocument"]["updateTime"],
        page["updateTime"]
    );
    Ok(())
}
#[tokio::test]
async fn editing_checks_prevent_writes() -> Result<()> {
    for (editing, owner, blocked, action, message) in [
        (false, UID, true, "lock", "blocked"),
        (true, UID, false, "lock", "already being edited"),
        (false, UID, false, "resume", "not being edited"),
        (true, "other", false, "resume", "another user"),
    ] {
        let server = MockServer::start().await;
        let client = client(&server)?;
        let mut page = page_reads()[1].clone();
        page["fields"]["status"] = json!({"stringValue":if editing {"editing"} else {"default"}});
        page["fields"]["tempEditedBy"] = json!({"stringValue":owner});
        read(&server, page, 1).await;
        if action == "lock" {
            let mut project = project_read();
            project["fields"]["blockedUsers"] = json!({"arrayValue":{"values":if blocked {vec![json!({"stringValue":UID})]} else {vec![]}}});
            read(&server, project, 1).await;
        }
        let result = if action == "lock" {
            client.lock_page(ENTRY, 20, false).await
        } else {
            client.resume_editing(ENTRY, 20).await
        };
        match result {
            Err(error) => assert!(error.to_string().contains(message), "{error}"),
            Ok(_) => panic!("check did not reject"),
        }
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|r| !r.url.path().ends_with(":commit"))
        );
    }
    Ok(())
}
#[tokio::test]
async fn approvals_review_flags_and_raw_timeline_data() -> Result<()> {
    for approval in [None, Some(false), Some(true)] {
        let server = MockServer::start().await;
        let client = client(&server)?;
        let mut page = page_reads()[3].clone();
        page["fields"]["approvedBy"] = encode_value(&json!([UID, "reviewer-2"]));
        page["fields"]["editedBy"] = encode_value(&json!("original-editor"));
        page["fields"]["requestReview"] = encode_value(&json!(true));
        page["fields"]["futureField"] = encode_value(&json!({"nested":[null,"kept"]}));
        read(&server, page, 2).await;
        read(&server, project_read(), 1).await;
        commit_response(&server, commits("response")[3].clone()).await;
        let mut session = client.resume_editing(ENTRY, 20).await?;
        let saved = session
            .save(SaveOptions {
                status: Some(PageStatus::Completed),
                share: true,
                request_review: true,
                comment: "review".into(),
                is_approval: approval,
            })
            .await?;
        assert_eq!(
            saved.page.approved_by,
            Some(if approval == Some(false) {
                vec!["reviewer-2".into()]
            } else {
                vec![UID.into(), "reviewer-2".into()]
            })
        );
        let requests = server.received_requests().await.unwrap();
        let commit: Value = serde_json::from_slice(
            &requests
                .iter()
                .find(|r| r.url.path().ends_with(":commit"))
                .unwrap()
                .body,
        )?;
        let fields = &commit["writes"][0]["update"]["fields"];
        assert_eq!(fields.get("approvedBy").is_some(), approval.is_some());
        let event =
            decode_value(&json!({"mapValue":{"fields":commit["writes"][1]["update"]["fields"]}}))?;
        assert_eq!(event["isReview"], true);
        assert_eq!(event["comment"], "review");
        assert_eq!(event["status"], "completed");
        assert_eq!(event["share"], true);
        assert_eq!(event["requestReview"], true);
        assert_eq!(event["isApproval"], approval.unwrap_or(false));
        assert_eq!(
            event["data"]["futureField"],
            json!({"nested":[null,"kept"]})
        );
        assert!(event["data"].get("_firestore").is_none());
        assert_eq!(event["data"]["approvedBy"], json!([UID, "reviewer-2"]));
    }
    Ok(())
}
#[tokio::test]
async fn third_approver_is_rejected() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let mut page = page_reads()[3].clone();
    page["fields"]["approvedBy"] = encode_value(&json!(["one", "two"]));
    read(&server, page, 2).await;
    read(&server, project_read(), 1).await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    let error = session
        .save(SaveOptions {
            is_approval: Some(true),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("more than two approvers"));
    Ok(())
}
fn canonical(value: Value) -> Value {
    match value {
        Value::Object(fields) => {
            let mut fields: serde_json::Map<_, _> =
                fields.into_iter().map(|(k, v)| (k, canonical(v))).collect();
            for (tag, key, empty) in [
                ("arrayValue", "values", json!([])),
                ("mapValue", "fields", json!({})),
            ] {
                if let Some(value) = fields.get_mut(tag)
                    && value.get(key).is_none()
                {
                    value[key] = empty;
                }
            }
            Value::Object(fields)
        }
        Value::Array(values) => json!(values.into_iter().map(canonical).collect::<Vec<_>>()),
        value => value,
    }
}
#[test]
fn every_fixture_firestore_value_round_trips() -> Result<()> {
    fn visit(value: &Value, count: &mut usize) -> Result<()> {
        match value {
            Value::Object(fields) => {
                if fields.len() == 1
                    && fields.keys().any(|key| {
                        matches!(
                            key.as_str(),
                            "stringValue"
                                | "integerValue"
                                | "doubleValue"
                                | "booleanValue"
                                | "nullValue"
                                | "arrayValue"
                                | "mapValue"
                                | "timestampValue"
                        )
                    })
                {
                    assert_eq!(
                        encode_value(&decode_value(value)?),
                        canonical(value.clone())
                    );
                    *count += 1;
                }
                for value in fields.values() {
                    visit(value, count)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value, count)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut count = 0;
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/api"))? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|s| s == "json") {
            visit(
                &serde_json::from_slice::<Value>(&std::fs::read(entry.path())?)?,
                &mut count,
            )?;
        }
    }
    assert!(count > 300);
    let timestamp: Timestamp = serde_json::from_value(json!("2026-09-10T01:02:03.123456789Z"))?;
    assert_eq!(
        timestamp.firestore_value()?,
        json!({"timestampValue":"2026-09-10T01:02:03.123456789Z"})
    );
    assert_eq!(
        encode_value(&json!("2026-09-10T01:02:03Z")),
        json!({"stringValue":"2026-09-10T01:02:03Z"})
    );
    Ok(())
}

#[tokio::test]
async fn drafts_coalesce_while_a_write_is_in_flight() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    read(&server, page_reads()[1].clone(), 3).await;
    let started = Arc::new(Mutex::new(Vec::new()));
    let observed = started.clone();
    let response = commits("response")[1].clone();
    Mock::given(path("/documents:commit"))
        .respond_with(move |request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            observed.lock().unwrap().push((Instant::now(), body));
            ResponseTemplate::new(200)
                .set_body_json(response.clone())
                .set_delay(Duration::from_millis(250))
        })
        .expect(2)
        .mount(&server)
        .await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    let queue = session.draft_queue();
    let task = tokio::spawn(async move {
        session.draft("first").await?;
        Ok::<_, Error>(session)
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while started.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|_| Error::Timeout)?;
    queue.request_with_notes(
        "intermediate",
        Some(vec![json!({"content":"pending note"})]),
    )?;
    queue.request("latest")?;
    let session = task.await.map_err(|e| Error::Worker(e.to_string()))??;
    assert_eq!(session.page().temp_text.as_deref(), Some("latest"));
    let writes = started.lock().unwrap();
    assert_eq!(writes.len(), 2);
    assert!(writes[1].0.duration_since(writes[0].0) >= Duration::from_millis(2990));
    let texts: Vec<_> = writes
        .iter()
        .map(|(_, body)| {
            body["writes"][0]["update"]["fields"]["tempText"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(texts, vec!["first", "latest"]);
    assert_eq!(
        writes[1].1["writes"][0]["update"]["fields"]["tempNotes"],
        encode_value(&json!([{"content":"pending note"}]))
    );
    Ok(())
}
#[tokio::test]
async fn save_flushes_pending_text_then_reads_server_copy() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let initial = page_reads()[1].clone();
    let server_page = Arc::new(Mutex::new(initial.clone()));
    let page_response = server_page.clone();
    Mock::given(path("/documents:batchGet"))
        .and(body_json(json!({"documents":[initial["name"]]})))
        .respond_with(move |_: &wiremock::Request| {
            ResponseTemplate::new(200)
                .set_body_json(json!([{"found":*page_response.lock().unwrap()}]))
        })
        .expect(3)
        .mount(&server)
        .await;
    read(&server, project_read(), 1).await;
    let writes_seen = Arc::new(Mutex::new(Vec::new()));
    let observed = writes_seen.clone();
    Mock::given(path("/documents:commit"))
        .respond_with(move |request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let mut seen = observed.lock().unwrap();
            let response = if seen.is_empty() {
                let mut page = server_page.lock().unwrap();
                page["fields"]["tempText"] = json!({"stringValue":"server draft"});
                commits("response")[1].clone()
            } else {
                commits("response")[3].clone()
            };
            seen.push(body);
            ResponseTemplate::new(200).set_body_json(response)
        })
        .expect(2)
        .mount(&server)
        .await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    session.draft_queue().request("queued draft")?;
    let saved = session.save(SaveOptions::default()).await?;
    assert_eq!(saved.page.text, "server draft");
    let writes = writes_seen.lock().unwrap();
    assert_eq!(
        writes[0]["writes"][0]["update"]["fields"]["tempText"]["stringValue"],
        "queued draft"
    );
    assert_eq!(
        writes[1]["writes"][0]["update"]["fields"]["text"]["stringValue"],
        "server draft"
    );
    Ok(())
}
#[tokio::test]
async fn failed_draft_retains_latest_text_for_retry() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    read(&server, page_reads()[1].clone(), 2).await;
    Mock::given(path("/documents:commit"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    assert!(session.draft_now("retry me").await.is_err());
    server.verify().await;
    server.reset().await;
    read(&server, page_reads()[1].clone(), 1).await;
    commit_response(&server, commits("response")[1].clone()).await;
    session.flush_drafts(true).await?;
    assert_eq!(session.page().temp_text.as_deref(), Some("retry me"));
    Ok(())
}
#[tokio::test]
async fn batch_get_orders_missing_documents_and_rejects_incomplete_results() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let page = page_reads()[0].clone();
    let missing = client.document_name("transcriptions/missing_0")?;
    Mock::given(path("/documents:batchGet"))
        .and(body_json(json!({"documents":[page["name"],missing]})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"missing":missing},{"found":page}])),
        )
        .expect(1)
        .mount(&server)
        .await;
    let documents = client
        .batch_get([page["name"].as_str().unwrap(), &missing])
        .await?;
    assert_eq!(documents[0].as_ref().unwrap().name, page["name"]);
    assert!(documents[1].is_none());
    server.reset().await;
    Mock::given(path("/documents:batchGet"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        client
            .batch_get(["transcriptions/missing_0"])
            .await
            .is_err()
    );
    Ok(())
}
#[tokio::test]
async fn conflict_includes_missing_page() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let name = client.document_name("transcriptions/missing_0")?;
    Mock::given(path("/documents:commit"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error":{"status":"FAILED_PRECONDITION"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/documents:batchGet"))
        .and(body_json(json!({"documents":[name]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"missing":name}])))
        .expect(1)
        .mount(&server)
        .await;
    let result = client
        .commit(vec![Write::Update {
            name: name.clone(),
            fields: json!({"status":"default"}),
            update_mask: vec!["status".into()],
            update_transforms: vec![],
            precondition: Some(Precondition {
                update_time: "2026-09-10T00:00:00.123456789Z".into(),
            }),
        }])
        .await;
    assert!(matches!(result,Err(Error::Conflict{path,current:None}) if path == name));
    Ok(())
}

#[tokio::test]
async fn lock_without_project_defaults_null_notes_and_honors_sync() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let mut page = page_reads()[0].clone();
    page["fields"].as_object_mut().unwrap().remove("projectId");
    page["fields"]["notes"] = json!({"nullValue":null});
    read(&server, page.clone(), 1).await;
    commit_response(&server, commits("response")[0].clone()).await;
    let session = client.lock_page(ENTRY, 20, true).await?;
    assert_eq!(session.page().sync_mode, Some(true));
    assert_eq!(session.page().temp_notes, Some(vec![]));
    let mut expected = commits("request")[0].clone();
    expected["writes"].as_array_mut().unwrap().truncate(1);
    expected["writes"][0]["update"]["fields"]["syncMode"] = json!({"booleanValue":true});
    assert_commit(&server, expected).await;
    Ok(())
}
#[tokio::test]
async fn each_operation_rechecks_server_lock() -> Result<()> {
    for operation in ["draft", "save", "discard"] {
        for (status, owner, message) in [
            ("initiated", UID, "not being edited"),
            ("editing", "other", "another user"),
        ] {
            let server = MockServer::start().await;
            let client = client(&server)?;
            read(&server, page_reads()[1].clone(), 1).await;
            let mut session = client.resume_editing(ENTRY, 20).await?;
            server.verify().await;
            server.reset().await;
            let mut changed = page_reads()[1].clone();
            changed["fields"]["status"] = json!({"stringValue":status});
            changed["fields"]["tempEditedBy"] = json!({"stringValue":owner});
            read(&server, changed, 1).await;
            let result = match operation {
                "draft" => session.draft_now("text").await,
                "save" => session.save(SaveOptions::default()).await.map(|_| ()),
                _ => session.discard().await,
            };
            assert!(result.unwrap_err().to_string().contains(message));
            assert!(
                server
                    .received_requests()
                    .await
                    .unwrap()
                    .iter()
                    .all(|r| !r.url.path().ends_with(":commit"))
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn ocr_write_preserves_other_fields_and_uses_server_timestamps() -> Result<()> {
    let server = MockServer::start().await;
    let page = page_reads()[0].clone();
    read(&server, page.clone(), 1).await;
    commit_response(&server, commits("response")[0].clone()).await;
    client(&server)?
        .write_ocr(
            ENTRY,
            20,
            "minna",
            json!({"engine":"minna","text":"字","lines":[],"createdAt":"client time"}),
        )
        .await?;
    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|r| r.url.path().ends_with(":commit"))
        .unwrap();
    let payload: Value = serde_json::from_slice(&request.body)?;
    let write = &payload["writes"][0];
    assert_eq!(payload["writes"].as_array().unwrap().len(), 1);
    assert_eq!(write["updateMask"]["fieldPaths"], json!(["ocr.minna"]));
    assert_eq!(write["currentDocument"]["updateTime"], page["updateTime"]);
    assert_eq!(
        write["updateTransforms"],
        json!([
            {"fieldPath":"updatedAt","setToServerValue":"REQUEST_TIME"},
            {"fieldPath":"ocr.minna.createdAt","setToServerValue":"REQUEST_TIME"}
        ])
    );
    assert!(
        write["update"]["fields"]["ocr"]["mapValue"]["fields"]["minna"]["mapValue"]["fields"]
            .get("createdAt")
            .is_none()
    );
    assert!(write["update"]["fields"].get("text").is_none());
    Ok(())
}

#[tokio::test]
async fn notes_draft_writes_only_temp_notes_with_lock_precondition() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let document = page_reads()[1].clone();
    read(&server, document.clone(), 2).await;
    let mut response = commits("response")[1].clone();
    response["writeResults"][0]
        .as_object_mut()
        .unwrap()
        .remove("transformResults");
    commit_response(&server, response.clone()).await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    let previous_timestamp = session.page().updated_at.clone();
    let notes = vec![
        Value::Null,
        json!({"type":"note","content":"原本の書入れ","markdown":"原本の書入れ"}),
    ];
    session.draft_notes(&notes).await?;
    assert_eq!(session.page().updated_at, previous_timestamp);
    assert_commit(
        &server,
        json!({"writes":[{
            "update":{"name": document["name"],"fields":{"tempNotes":encode_value(&json!(notes))}},
            "updateMask":{"fieldPaths":["tempNotes"]},
            "currentDocument":{"updateTime":document["updateTime"]}
        }]}),
    )
    .await;
    assert_eq!(
        serde_json::to_value(&session.page().temp_notes)?,
        json!(notes)
    );
    assert_eq!(
        session.update_time(),
        response["writeResults"][0]["updateTime"].as_str().unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn notes_draft_rejects_a_lost_lock() -> Result<()> {
    for (status, owner) in [("editing", "another-user"), ("completed", UID)] {
        let server = MockServer::start().await;
        let client = client(&server)?;
        read(&server, page_reads()[1].clone(), 1).await;
        let mut session = client.resume_editing(ENTRY, 20).await?;
        server.reset().await;
        let mut changed = page_reads()[1].clone();
        changed["fields"]["status"] = json!({"stringValue": status});
        changed["fields"]["tempEditedBy"] = json!({"stringValue": owner});
        read(&server, changed, 1).await;
        assert!(session.draft_notes(&[]).await.is_err());
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|r| !r.url.path().ends_with(":commit"))
        );
    }
    Ok(())
}

#[tokio::test]
async fn note_delete_preserves_slots_and_server_notes() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let mut document = page_reads()[1].clone();
    let notes = json!([{"content":"first"},null,{"content":"latest server note"}]);
    document["fields"]["tempNotes"] = encode_value(&notes);
    read(&server, document.clone(), 2).await;
    commit_response(&server, commits("response")[1].clone()).await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    session.delete_note(0).await?;
    assert_commit(&server, json!({"writes":[{
        "update":{"name":document["name"],"fields":{"tempNotes":encode_value(&json!([null,null,{"content":"latest server note"}]))}},
        "updateMask":{"fieldPaths":["tempNotes"]},
        "currentDocument":{"updateTime":document["updateTime"]}
    }]})).await;
    Ok(())
}

#[tokio::test]
async fn combined_draft_keeps_region_and_timestamp_types() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let document = page_reads()[1].clone();
    read(&server, document.clone(), 2).await;
    commit_response(&server, commits("response")[1].clone()).await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    let note = json!({"id":"","type":"memo","content":"region","markdown":"region","createdBy":UID,"createdAt":"2026-09-10T01:00:00Z","updatedAt":"2026-09-10T02:00:00Z","image":"https://example.org/iiif/10,20,30,40/300,/0/default.jpg","xywh":[10,20,30,40]});
    session
        .draft_queue()
        .request_with_notes("本文", Some(vec![Value::Null, note.clone()]))?;
    session.flush_drafts(true).await?;
    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|r| r.url.path().ends_with(":commit"))
        .unwrap();
    let body: Value = serde_json::from_slice(&request.body)?;
    let fields = &body["writes"][0]["update"]["fields"];
    let stored = &fields["tempNotes"]["arrayValue"]["values"][1]["mapValue"]["fields"];
    assert!(stored["createdAt"]["timestampValue"].is_string());
    assert!(stored["updatedAt"]["timestampValue"].is_string());
    assert_eq!(stored["image"], encode_value(&note["image"]));
    assert_eq!(stored["xywh"], encode_value(&note["xywh"]));
    assert_eq!(fields["tempText"], json!({"stringValue":"本文"}));
    assert_eq!(
        session.page().temp_notes.as_ref().unwrap()[1]
            .as_ref()
            .unwrap()["content"],
        "region"
    );
    Ok(())
}

#[tokio::test]
async fn note_delete_retries_against_the_new_server_array() -> Result<()> {
    let server = MockServer::start().await;
    let client = client(&server)?;
    let mut document = page_reads()[1].clone();
    document["fields"]["tempNotes"] = encode_value(&json!([{"content":"remove"}]));
    let current = Arc::new(Mutex::new(document.clone()));
    let for_read = current.clone();
    Mock::given(path("/documents:batchGet"))
        .respond_with(move |_: &wiremock::Request| {
            ResponseTemplate::new(200).set_body_json(json!([{"found":*for_read.lock().unwrap()}]))
        })
        .expect(4)
        .mount(&server)
        .await;
    let attempts = Arc::new(Mutex::new(Vec::<Value>::new()));
    let observed = attempts.clone();
    Mock::given(path("/documents:commit"))
        .respond_with(move |request: &wiremock::Request| {
            let mut attempts = observed.lock().unwrap();
            attempts.push(serde_json::from_slice(&request.body).unwrap());
            if attempts.len() == 1 {
                let mut document = current.lock().unwrap();
                document["updateTime"] = json!("2026-09-10T07:41:42Z");
                document["fields"]["tempNotes"] =
                    encode_value(&json!([{"content":"remove"},{"content":"concurrent note"}]));
                ResponseTemplate::new(400)
                    .set_body_json(json!({"error":{"status":"FAILED_PRECONDITION"}}))
            } else {
                ResponseTemplate::new(200).set_body_json(commits("response")[1].clone())
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    let mut session = client.resume_editing(ENTRY, 20).await?;
    session.delete_note(0).await?;
    let attempts = attempts.lock().unwrap();
    assert_eq!(
        attempts[1]["writes"][0]["update"]["fields"]["tempNotes"],
        encode_value(&json!([null,{"content":"concurrent note"}]))
    );
    assert_eq!(
        attempts[1]["writes"][0]["currentDocument"]["updateTime"],
        "2026-09-10T07:41:42Z"
    );
    Ok(())
}
