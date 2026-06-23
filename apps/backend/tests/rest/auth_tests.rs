use crate::common::TestServer;

async fn register_user(
    server: &TestServer,
    client: &reqwest::Client,
    name: &str,
    email: &str,
    password: &str,
) -> reqwest::Response {
    let resp = client
        .post(format!("{}/api/auth/register", server.base_url))
        .json(&serde_json::json!({
            "name": name,
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .expect("Failed to send register request");

    assert!(
        resp.status().is_success(),
        "Register failed: {} {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    resp
}

async fn register_user_expect_error(
    server: &TestServer,
    client: &reqwest::Client,
    name: &str,
    email: &str,
    password: &str,
) -> reqwest::Response {
    client
        .post(format!("{}/api/auth/register", server.base_url))
        .json(&serde_json::json!({
            "name": name,
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .expect("Failed to send register request")
}

async fn me(server: &TestServer, client: &reqwest::Client) -> reqwest::Response {
    client
        .get(format!("{}/api/auth/me", server.base_url))
        .send()
        .await
        .expect("Failed to send me request")
}

fn extract_token_from_cookies(resp: &reqwest::Response) -> Option<String> {
    let headers = resp.headers();
    let cookies = headers.get_all("set-cookie").iter();
    for cookie in cookies {
        if let Ok(val) = cookie.to_str()
            && val.starts_with("token=")
        {
            let token_val = val.split(';').next()?.strip_prefix("token=")?;
            return Some(token_val.to_string());
        }
    }
    None
}

fn extract_csrf_from_cookies(resp: &reqwest::Response) -> Option<String> {
    let headers = resp.headers();
    let cookies = headers.get_all("set-cookie").iter();
    for cookie in cookies {
        if let Ok(val) = cookie.to_str()
            && val.starts_with("csrf=")
        {
            let csrf_val = val.split(';').next()?.strip_prefix("csrf=")?;
            return Some(csrf_val.to_string());
        }
    }
    None
}

#[tokio::test]
async fn test_register_success() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = register_user(&server, &client, "newuser", "new@example.com", "password123").await;

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["user"]["name"], "newuser");
    assert_eq!(body["user"]["email"], "new@example.com");
    assert!(body["user"]["id"].is_string());
}

#[tokio::test]
async fn test_register_validation_empty_name() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = register_user_expect_error(&server, &client, "", "test@example.com", "password123").await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_short_display_name_allowed() {
    // `name` is now a display name (non-unique, no length floor beyond non-empty),
    // so a short name like "ab" is accepted.
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = register_user(&server, &client, "ab", "test@example.com", "password123").await;

    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_register_validation_short_password() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = register_user_expect_error(&server, &client, "validuser", "test@example.com", "short").await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_validation_invalid_email() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = register_user_expect_error(&server, &client, "validuser", "not-an-email", "password123").await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_duplicate_email() {
    // Email is the unique login identity; a second registration with the same email
    // (even under a different display name) is rejected.
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    register_user(&server, &client, "First User", "dupe@example.com", "password123").await;

    let resp = register_user_expect_error(&server, &client, "Second User", "dupe@example.com", "password123").await;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    assert!(
        status == 409 || status == 400,
        "Expected 409 or 400 for duplicate email, got {}: {:?}",
        status,
        body
    );
}

#[tokio::test]
async fn test_login_success() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    register_user(&server, &client, "testuser", "test@example.com", "password123").await;

    let resp = server.login_user(&client, "test@example.com", "password123").await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_login_wrong_password() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    register_user(&server, &client, "testuser", "test@example.com", "password123").await;

    let resp = server.login_user(&client, "test@example.com", "wrongpassword").await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.login_user(&client, "nobody", "password123").await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_me_authenticated() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    register_user(&server, &client, "testuser", "test@example.com", "password123").await;

    let resp = server.login_user(&client, "test@example.com", "password123").await;
    assert_eq!(resp.status(), 200);

    let token = extract_token_from_cookies(&resp).expect("No token cookie");
    let csrf = extract_csrf_from_cookies(&resp).expect("No csrf cookie");

    let me_resp = client
        .get(format!("{}/api/auth/me", server.base_url))
        .header("Cookie", format!("token={}", token))
        .header("X-CSRF-Token", &csrf)
        .send()
        .await
        .unwrap();

    assert_eq!(me_resp.status(), 200);
    let body: serde_json::Value = me_resp.json().await.unwrap();
    assert_eq!(body["name"], "testuser");
}

// ── routing: the MCP auth layer must not leak onto the global fallback (issue 1) ──

#[tokio::test]
async fn test_dashboard_trailing_slash_does_not_hit_mcp_auth() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    // `/dashboard/` must route to the SPA handler, never the MCP auth fallback.
    let resp = client
        .get(format!("{}/dashboard/", server.base_url))
        .send()
        .await
        .unwrap();
    assert_ne!(resp.status(), 401, "/dashboard/ must not require MCP auth");
    let body = resp.text().await.unwrap_or_default();
    assert!(
        !body.contains("Missing Authorization bearer token"),
        "/dashboard/ returned the MCP auth error: {body}"
    );
}

#[tokio::test]
async fn test_unknown_path_returns_404_not_mcp_auth() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = client
        .get(format!("{}/this-route-does-not-exist", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "unknown paths should 404, not hit MCP auth");
    let body = resp.text().await.unwrap_or_default();
    assert!(!body.contains("Missing Authorization bearer token"));
}

#[tokio::test]
async fn test_mcp_still_requires_bearer_token() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    // The /mcp endpoint must remain protected by the MCP auth layer.
    let resp = client.get(format!("{}/mcp", server.base_url)).send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_me_unauthenticated() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = me(&server, &client).await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_logout() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    register_user(&server, &client, "testuser", "test@example.com", "password123").await;

    let resp = server.login_user(&client, "test@example.com", "password123").await;
    assert_eq!(resp.status(), 200);

    let token = extract_token_from_cookies(&resp).expect("No token cookie");
    let csrf = extract_csrf_from_cookies(&resp).expect("No csrf cookie");

    let logout_resp = client
        .post(format!("{}/api/auth/logout", server.base_url))
        .header("Cookie", format!("token={}; csrf={}", token, csrf))
        .header("X-CSRF-Token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(logout_resp.status(), 200);

    let me_resp = client
        .get(format!("{}/api/auth/me", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(me_resp.status(), 401);
}
