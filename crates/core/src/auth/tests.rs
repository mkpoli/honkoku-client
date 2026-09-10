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
