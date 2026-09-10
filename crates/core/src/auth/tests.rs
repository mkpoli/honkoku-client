use super::*;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string, header, method, path, query_param},
};

fn session(minutes: i64) -> Result<Session> {
    let mut session: Session = serde_json::from_str(include_str!("dev-session.fixture.json"))?;
    session.expires_at = Timestamp(OffsetDateTime::now_utc() + time::Duration::minutes(minutes));
    Ok(session)
}
fn refreshed() -> serde_json::Value {
    json!({"id_token":"new-id", "refresh_token":"rotated-refresh", "expires_in":"3600", "user_id":"fixture-user"})
}
#[test]
fn file_round_trip_import_and_permissions() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let fixture = dir.path().join("import.json");
    std::fs::write(&fixture, include_str!("dev-session.fixture.json"))?;
    let imported = import_dev_session(&fixture)?;
    assert_eq!(imported.uid, "fixture-user");
    assert_eq!(imported.display_name.as_deref(), Some("試験利用者"));
    assert!(imported.email.is_none());
    assert_eq!(imported.providers, ["twitter.com"]);
    let store = FileStore::new(dir.path().join("nested/session.json"));
    assert!(store.load()?.is_none());
    store.save(&imported)?;
    let loaded = store.load()?.ok_or(Error::SignedOut)?;
    assert_eq!(
        serde_json::to_value(&loaded)?,
        serde_json::to_value(&imported)?
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&store.0)?.permissions().mode() & 0o777,
            0o600
        );
        std::fs::set_permissions(&store.0, std::fs::Permissions::from_mode(0o644))?;
        store.save(&imported)?;
        assert_eq!(
            std::fs::metadata(&store.0)?.permissions().mode() & 0o777,
            0o600
        );
    }
    store.clear()?;
    store.clear()?;
    assert!(store.load()?.is_none());
    std::fs::write(&store.0, "invalid")?;
    assert!(store.load().is_err());
    Ok(())
}
#[tokio::test]
async fn fresh_token_does_not_refresh() -> Result<()> {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir()?;
    let manager = TokenManager::with_endpoint(
        session(6)?,
        Arc::new(FileStore::new(dir.path().join("session.json"))),
        &server.uri(),
    )?;
    assert_eq!(manager.id_token().await?, "fixture-id-token");
    assert_eq!(server.received_requests().await.map(|r| r.len()), Some(0));
    Ok(())
}
#[tokio::test]
async fn refresh_is_single_flight_and_persists_rotation() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(query_param("key", "fixture-api-key"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string(
            "grant_type=refresh_token&refresh_token=fixture-refresh-token",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(refreshed())
                .set_delay(Duration::from_millis(50)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let store = Arc::new(FileStore::new(dir.path().join("session.json")));
    let manager = TokenManager::with_endpoint(
        session(4)?,
        store.clone(),
        &format!("{}/token", server.uri()),
    )?;
    let tokens = futures_util::future::try_join_all((0..12).map(|_| manager.id_token())).await?;
    assert!(tokens.iter().all(|token| token == "new-id"));
    let saved = store.load()?.ok_or(Error::SignedOut)?;
    assert_eq!(saved.refresh_token, "rotated-refresh");
    assert_eq!(saved.id_token, "new-id");
    assert!(saved.expires_at.0 > OffsetDateTime::now_utc() + time::Duration::minutes(55));
    Ok(())
}
#[tokio::test]
async fn revoked_sessions_clear_disk_and_memory() -> Result<()> {
    for code in [
        "TOKEN_EXPIRED",
        "USER_DISABLED",
        "USER_NOT_FOUND",
        "INVALID_REFRESH_TOKEN",
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"error":{"message":code}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir()?;
        let store = Arc::new(FileStore::new(dir.path().join("session.json")));
        let session = session(-1)?;
        store.save(&session)?;
        let manager = TokenManager::with_endpoint(session, store.clone(), &server.uri())?;
        assert!(matches!(manager.id_token().await, Err(Error::SignedOut)));
        assert!(matches!(manager.id_token().await, Err(Error::SignedOut)));
        assert!(matches!(manager.uid().await, Err(Error::SignedOut)));
        assert!(store.load()?.is_none());
    }
    Ok(())
}
#[tokio::test]
async fn transient_failure_preserves_session_and_retries() -> Result<()> {
    let server = MockServer::start().await;
    let failed = Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(503).set_body_json(json!({"error":{"message":"UNAVAILABLE"}})),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let store = Arc::new(FileStore::new(dir.path().join("session.json")));
    let session = session(-1)?;
    store.save(&session)?;
    let manager = TokenManager::with_endpoint(session, store.clone(), &server.uri())?;
    assert!(manager.id_token().await.is_err());
    assert!(store.load()?.is_some());
    drop(failed);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refreshed()))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(manager.id_token().await?, "new-id");
    Ok(())
}
#[tokio::test]
async fn failed_persistence_keeps_rotated_token_for_retry() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(refreshed()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir()?;
    let parent = dir.path().join("blocked");
    std::fs::write(&parent, "blocks directory creation")?;
    let store = Arc::new(FileStore::new(parent.join("session.json")));
    let manager = TokenManager::with_endpoint(session(-1)?, store.clone(), &server.uri())?;
    assert!(manager.id_token().await.is_err());
    std::fs::remove_file(parent)?;
    assert_eq!(manager.id_token().await?, "new-id");
    assert_eq!(
        store.load()?.ok_or(Error::SignedOut)?.refresh_token,
        "rotated-refresh"
    );
    Ok(())
}

fn capture() -> CapturedSession {
    CapturedSession {
        attempt_id: "attempt".into(),
        uid: "fixture-user".into(),
        refresh_token: "fixture-refresh-token".into(),
        id_token: "captured-id".into(),
    }
}
fn lookup_user() -> serde_json::Value {
    json!({"localId":"fixture-user", "email":"verified@example.test", "displayName":"確認済み",
        "providerUserInfo":[{"providerId":"twitter.com"},{"providerId":"google.com"}]})
}
async fn lookup_mock(server: &MockServer, users: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/lookup"))
        .and(query_param("key", FIREBASE_API_KEY))
        .and(wiremock::matchers::body_json(
            json!({"idToken":"captured-id"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"users":users})))
        .expect(1)
        .mount(server)
        .await;
}
async fn refresh_mock(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/refresh"))
        .and(query_param("key", FIREBASE_API_KEY))
        .and(body_string(
            "grant_type=refresh_token&refresh_token=fixture-refresh-token",
        ))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}
#[tokio::test]
async fn capture_verifies_lookup_then_refreshes_before_persistence() -> Result<()> {
    let server = MockServer::start().await;
    lookup_mock(&server, json!([lookup_user()])).await;
    refresh_mock(
        &server,
        ResponseTemplate::new(200).set_body_json(refreshed()),
    )
    .await;
    let session = capture()
        .verify_with_endpoints(
            SignInProvider::Twitter,
            &format!("{}/lookup", server.uri()),
            &format!("{}/refresh", server.uri()),
        )
        .await?;
    assert_eq!(session.uid, "fixture-user");
    assert_eq!(session.email.as_deref(), Some("verified@example.test"));
    assert_eq!(session.display_name.as_deref(), Some("確認済み"));
    assert_eq!(session.providers, ["twitter.com", "google.com"]);
    assert_eq!(session.id_token, "new-id");
    assert_eq!(session.refresh_token, "rotated-refresh");
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.iter().map(|r| r.url.path()).collect::<Vec<_>>(),
        ["/lookup", "/refresh"]
    );
    let dir = tempfile::tempdir()?;
    let store = FileStore::new(dir.path().join("session.json"));
    store.save(&session)?;
    assert_eq!(store.load()?.ok_or(Error::SignedOut)?.id_token, "new-id");
    Ok(())
}
#[tokio::test]
async fn capture_rejects_lookup_uid_provider_and_user_count() {
    let mut wrong_uid = lookup_user();
    wrong_uid["localId"] = json!("someone-else");
    let mut wrong_provider = lookup_user();
    wrong_provider["providerUserInfo"] = json!([{"providerId":"google.com"}]);
    for users in [
        json!([wrong_uid]),
        json!([wrong_provider]),
        json!([]),
        json!([lookup_user(), lookup_user()]),
    ] {
        let server = MockServer::start().await;
        lookup_mock(&server, users).await;
        assert!(
            capture()
                .verify_with_endpoints(
                    SignInProvider::Twitter,
                    &format!("{}/lookup", server.uri()),
                    &format!("{}/refresh", server.uri())
                )
                .await
                .is_err()
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
#[tokio::test]
async fn capture_rejects_failed_refresh_and_different_refresh_identity() {
    let mut wrong_uid = refreshed();
    wrong_uid["user_id"] = json!("someone-else");
    for response in [
        ResponseTemplate::new(400)
            .set_body_json(json!({"error":{"message":"INVALID_REFRESH_TOKEN"}})),
        ResponseTemplate::new(503).set_body_json(json!({})),
        ResponseTemplate::new(200).set_body_json(wrong_uid),
    ] {
        let server = MockServer::start().await;
        lookup_mock(&server, json!([lookup_user()])).await;
        refresh_mock(&server, response).await;
        assert!(
            capture()
                .verify_with_endpoints(
                    SignInProvider::Twitter,
                    &format!("{}/lookup", server.uri()),
                    &format!("{}/refresh", server.uri())
                )
                .await
                .is_err()
        );
    }
}
#[test]
fn desktop_prefers_keyring_and_migrates_file_without_retaining_a_copy() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let keyring = Arc::new(FileStore::new(dir.path().join("fake-keyring.json")));
    let path = dir.path().join("session.json");
    FileStore::new(&path).save(&session(60)?)?;
    let store = DesktopStore::select(Ok(keyring.clone()), FileStore::new(&path));
    assert_eq!(store.kind(), CredentialStore::Os);
    assert!(!path.exists());
    assert_eq!(store.load()?.ok_or(Error::SignedOut)?.uid, "fixture-user");
    // An existing OS credential takes precedence even over a malformed file.
    std::fs::write(&path, "malformed")?;
    let store = DesktopStore::select(Ok(keyring), FileStore::new(&path));
    assert_eq!(store.kind(), CredentialStore::Os);
    assert!(!path.exists());
    store.clear()?;
    assert!(store.load()?.is_none());
    Ok(())
}
#[test]
fn desktop_falls_back_to_the_file_for_any_keyring_failure() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("session.json");
    let store = DesktopStore::select(
        Err(Error::Keyring(keyring::Error::NoDefaultStore)),
        FileStore::new(&path),
    );
    assert_eq!(store.kind(), CredentialStore::File);
    assert!(store.fallback_reason().is_some());
    store.save(&session(60)?)?;
    assert!(path.exists());
    // A credential that cannot be written to the OS store keeps the file copy.
    let store = DesktopStore::select(
        Err(Error::Keyring(keyring::Error::TooLong(
            "session".into(),
            1200,
        ))),
        FileStore::new(&path),
    );
    assert_eq!(store.kind(), CredentialStore::File);
    assert!(path.exists());
    assert_eq!(store.load()?.ok_or(Error::SignedOut)?.uid, "fixture-user");
    Ok(())
}
#[test]
fn stored_credential_drops_the_id_token_and_expires_immediately() -> Result<()> {
    let original = session(60)?;
    let restored: Session = StoredCredential::from(&original).into();
    assert_eq!(restored.uid, original.uid);
    assert_eq!(restored.refresh_token, original.refresh_token);
    assert!(restored.id_token.is_empty());
    assert!(restored.expires_at.0 < OffsetDateTime::now_utc());
    let json = serde_json::to_string(&StoredCredential::from(&original))?;
    assert!(json.chars().count() <= MAX_CREDENTIAL_CHARS);
    Ok(())
}
#[test]
#[ignore = "probes the host credential service without writing credentials"]
fn desktop_host_store_probe() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = DesktopStore::new(dir.path().join("session.json"));
    println!(
        "desktop credential store: {:?} ({})",
        store.kind(),
        store.fallback_reason().unwrap_or("os store in use")
    );
    Ok(())
}
