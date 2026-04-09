use cms::config::Config;
use cms::middleware::auth::{create_token, verify_token, check_read_access_repo, check_write_access_repo, AuthContext, extract_user_id, compute_key_hmac};
use cms::repository::Repository;
use cms::database::pool::DbPool;
use sqlx::sqlite::SqlitePoolOptions;

async fn setup_test_db() -> (DbPool, Repository) {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db_pool = DbPool::Sqlite(pool);
    let schema = include_str!("../src/database/schema/sqlite.sql");
    for statement in schema.split(';').filter(|s| !s.trim().is_empty()) {
        sqlx::query(statement)
            .execute(match &db_pool {
                DbPool::Sqlite(p) => p,
                _ => unreachable!(),
            })
            .await
            .unwrap();
    }
    let repo = Repository::new(&db_pool);
    (db_pool, repo)
}

#[test]
fn test_create_and_verify_token_roundtrip() {
    let token = create_token("user-123".to_string(), "my-secret").unwrap();
    let claims = verify_token(&token, "my-secret").unwrap();
    assert_eq!(claims.sub, "user-123");
}

#[test]
fn test_verify_token_wrong_secret_fails() {
    let token = create_token("user-123".to_string(), "secret-a").unwrap();
    assert!(verify_token(&token, "secret-b").is_err());
}

#[test]
fn test_verify_token_malformed_fails() {
    assert!(verify_token("garbage", "secret").is_err());
    assert!(verify_token("", "secret").is_err());
}

#[test]
fn test_verify_token_expired_fails() {
    let token = create_token("user-1".to_string(), "s").unwrap();
    assert!(verify_token(&token, "s").is_ok());
}

#[test]
fn test_extract_user_id_from_auth_context() {
    let jwt_ctx = AuthContext::Jwt { user_id: "u1".to_string() };
    assert_eq!(extract_user_id(&jwt_ctx), Some("u1"));

    let api_ctx = AuthContext::ApiKey { site_id: "s1".to_string(), permissions: "read".to_string() };
    assert_eq!(extract_user_id(&api_ctx), None);
}

#[test]
fn test_compute_key_hmac_deterministic() {
    let key = "cms_abcdefgh_1234567890123456789012";
    let secret = "test-hmac-secret";
    let h1 = compute_key_hmac(key, secret);
    let h2 = compute_key_hmac(key, secret);
    assert_eq!(h1, h2);
}

#[test]
fn test_compute_key_hmac_different_keys() {
    let secret = "test-hmac-secret";
    let h1 = compute_key_hmac("key1", secret);
    let h2 = compute_key_hmac("key2", secret);
    assert_ne!(h1, h2);
}

#[tokio::test]
async fn test_check_read_access_with_jwt_viewer_role() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::Jwt { user_id: "u1".to_string() };
    let result = check_read_access_repo(&auth, &repo, "s1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_read_access_with_jwt_no_membership() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.user.create("u2", "bob", "bob@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::Jwt { user_id: "u2".to_string() };
    let result = check_read_access_repo(&auth, &repo, "s1").await;
    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_check_write_access_with_api_key_correct_site() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::ApiKey { site_id: "s1".to_string(), permissions: "write".to_string() };
    let result = check_write_access_repo(&auth, &repo, "s1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_write_access_with_api_key_read_only() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::ApiKey { site_id: "s1".to_string(), permissions: "read".to_string() };
    let result = check_write_access_repo(&auth, &repo, "s1").await;
    assert!(result.is_err());
    let (status, body) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "API key does not have write permissions");
}

#[tokio::test]
async fn test_check_write_access_with_api_key_wrong_site() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site1", "filesystem", "u1").await.unwrap();
    repo.site.create("s2", "Site2", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::ApiKey { site_id: "s1".to_string(), permissions: "write".to_string() };
    let result = check_write_access_repo(&auth, &repo, "s2").await;
    assert!(result.is_err());
    let (status, body) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "API key does not have access to this site");
}

#[tokio::test]
async fn test_check_read_access_with_api_key_correct_site() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::ApiKey { site_id: "s1".to_string(), permissions: "read".to_string() };
    let result = check_read_access_repo(&auth, &repo, "s1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_read_access_with_api_key_wrong_site() {
    let (_pool, repo) = setup_test_db().await;
    repo.user.create("u1", "alice", "alice@t.com", "h").await.unwrap();
    repo.site.create("s1", "Site1", "filesystem", "u1").await.unwrap();
    repo.site.create("s2", "Site2", "filesystem", "u1").await.unwrap();

    let auth = AuthContext::ApiKey { site_id: "s1".to_string(), permissions: "read".to_string() };
    let result = check_read_access_repo(&auth, &repo, "s2").await;
    assert!(result.is_err());
    let (status, _) = result.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN);
}

#[test]
fn test_config_has_s3() {
    let with_s3 = Config {
        database_url: "sqlite:cms.db".into(),
        jwt_secret: "s".into(),
        bind_address: "0.0.0.0:3000".into(),
        grpc_bind_address: "0.0.0.0:50051".into(),
        storage_fs_path: None,
        s3_access_key_id: Some("key".into()),
        s3_secret_access_key: Some("secret".into()),
        s3_bucket: Some("bucket".into()),
        s3_region: None,
        s3_endpoint: None,
        s3_public_url: None,
        max_upload_size_bytes: 50 * 1024 * 1024,
        cookie_secure: false,
        db_max_connections: 10,
        db_min_connections: 2,
        db_acquire_timeout_secs: 30,
        db_idle_timeout_secs: 600,
        rate_limit_max_requests: 100,
        rate_limit_window_secs: 60,
        hmac_secret: "hmac".into(),
    };
    assert!(with_s3.has_s3());

    let without_bucket = Config {
        database_url: "sqlite:cms.db".into(),
        jwt_secret: "s".into(),
        bind_address: "0.0.0.0:3000".into(),
        grpc_bind_address: "0.0.0.0:50051".into(),
        storage_fs_path: None,
        s3_access_key_id: Some("key".into()),
        s3_secret_access_key: Some("secret".into()),
        s3_bucket: None,
        s3_region: None,
        s3_endpoint: None,
        s3_public_url: None,
        max_upload_size_bytes: 50 * 1024 * 1024,
        cookie_secure: false,
        db_max_connections: 10,
        db_min_connections: 2,
        db_acquire_timeout_secs: 30,
        db_idle_timeout_secs: 600,
        rate_limit_max_requests: 100,
        rate_limit_window_secs: 60,
        hmac_secret: "hmac".into(),
    };
    assert!(!without_bucket.has_s3());
}