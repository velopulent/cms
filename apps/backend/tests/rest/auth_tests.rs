use crate::common::TestServer;

fn extract_token_from_cookies(resp: &reqwest::Response) -> Option<String> {
    let headers = resp.headers();
    let cookies = headers.get_all("set-cookie").iter();
    for cookie in cookies {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("token=") {
                let token_val = val.split(';').next()?.strip_prefix("token=")?;
                return Some(token_val.to_string());
            }
        }
    }
    None
}

fn extract_csrf_from_cookies(resp: &reqwest::Response) -> Option<String> {
    let headers = resp.headers();
    let cookies = headers.get_all("set-cookie").iter();
    for cookie in cookies {
        if let Ok(val) = cookie.to_str() {
            if val.starts_with("csrf=") {
                let csrf_val = val.split(';').next()?.strip_prefix("csrf=")?;
                return Some(csrf_val.to_string());
            }
        }
    }
    None
}

#[tokio::test]
async fn test_register_success() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server
        .register_user(&client, "newuser", "new@example.com", "password123")
        .await;

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["user"]["username"], "newuser");
    assert_eq!(body["user"]["email"], "new@example.com");
    assert!(body["user"]["id"].is_string());
}

#[tokio::test]
async fn test_register_validation_empty_username() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server
        .register_user_expect_error(&client, "", "test@example.com", "password123")
        .await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_validation_short_username() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server
        .register_user_expect_error(&client, "ab", "test@example.com", "password123")
        .await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_validation_short_password() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server
        .register_user_expect_error(&client, "validuser", "test@example.com", "short")
        .await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_validation_invalid_email() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server
        .register_user_expect_error(&client, "validuser", "not-an-email", "password123")
        .await;

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    server
        .register_user(&client, "testuser", "test@example.com", "password123")
        .await;

    let resp = server
        .register_user_expect_error(&client, "testuser", "other@example.com", "password123")
        .await;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    assert!(
        status == 409 || status == 400,
        "Expected 409 or 400 for duplicate username, got {}: {:?}",
        status,
        body
    );
}

#[tokio::test]
async fn test_login_success() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    server
        .register_user(&client, "testuser", "test@example.com", "password123")
        .await;

    let resp = server.login_user(&client, "testuser", "password123").await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_login_wrong_password() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    server
        .register_user(&client, "testuser", "test@example.com", "password123")
        .await;

    let resp = server.login_user(&client, "testuser", "wrongpassword").await;
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

    server
        .register_user(&client, "testuser", "test@example.com", "password123")
        .await;

    let resp = server.login_user(&client, "testuser", "password123").await;
    assert_eq!(resp.status(), 200);

    let jwt = extract_token_from_cookies(&resp).expect("No token cookie");
    let csrf = extract_csrf_from_cookies(&resp).expect("No csrf cookie");

    let me_resp = client
        .get(format!("{}/api/auth/me", server.base_url))
        .header("Cookie", format!("token={}", jwt))
        .header("X-CSRF-Token", &csrf)
        .send()
        .await
        .unwrap();

    assert_eq!(me_resp.status(), 200);
    let body: serde_json::Value = me_resp.json().await.unwrap();
    assert_eq!(body["username"], "testuser");
}

#[tokio::test]
async fn test_me_unauthenticated() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    let resp = server.me(&client).await;
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_logout() {
    let server = TestServer::start().await;
    let client = reqwest::Client::builder().build().unwrap();

    server
        .register_user(&client, "testuser", "test@example.com", "password123")
        .await;

    let resp = server.login_user(&client, "testuser", "password123").await;
    assert_eq!(resp.status(), 200);

    let jwt = extract_token_from_cookies(&resp).expect("No token cookie");
    let csrf = extract_csrf_from_cookies(&resp).expect("No csrf cookie");

    let logout_resp = client
        .post(format!("{}/api/auth/logout", server.base_url))
        .header("Cookie", format!("token={}; csrf={}", jwt, csrf))
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
