use axum::{
    Json,
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rmcp::model::{CallToolResult, ContentBlock, ErrorCode, ErrorData};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::json;

use std::sync::Arc;

use crate::config::Config;
use crate::middleware::auth::{Actor, verify_access_token};
use crate::repository::Repository;

pub fn mcp_error(code: ErrorCode, message: impl Into<String>) -> ErrorData {
    ErrorData::new(code, message.into(), None)
}

pub fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, ErrorData> {
    let value = serde_json::to_value(data).map_err(|e| {
        mcp_error(
            ErrorCode::INTERNAL_ERROR,
            format!("Failed to serialize response: {}", e),
        )
    })?;
    let text = serde_json::to_string(&value).map_err(|e| {
        mcp_error(
            ErrorCode::INTERNAL_ERROR,
            format!("Failed to serialize response: {}", e),
        )
    })?;
    let mut result = CallToolResult::structured(value);
    // Keep a textual representation alongside structuredContent for clients
    // that still render the canonical content array while migrating to the
    // 2026-07-28 structured-output contract.
    result.content = vec![ContentBlock::text(text)];
    Ok(result)
}

pub fn text_result(message: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(message.into())])
}

pub fn map_err(e: impl Into<crate::services::error::ServiceError>) -> ErrorData {
    service_error_to_mcp(e.into())
}

/// Convert a service error into a tool execution error (CallToolResult with isError: true).
/// Use this for business logic errors that the LLM can act on (not found, validation, permission).
pub fn tool_error(e: impl Into<crate::services::error::ServiceError>) -> CallToolResult {
    let error = e.into();
    let code = match error.status_code() {
        http::StatusCode::UNAUTHORIZED => "unauthorized",
        http::StatusCode::FORBIDDEN => "forbidden",
        http::StatusCode::NOT_FOUND => "not_found",
        http::StatusCode::CONFLICT => "conflict",
        http::StatusCode::BAD_REQUEST => "invalid_request",
        http::StatusCode::PRECONDITION_FAILED => "precondition_failed",
        http::StatusCode::PAYLOAD_TOO_LARGE => "payload_too_large",
        _ => "internal_error",
    };
    let message = error.error_message();
    let mut result = CallToolResult::error(vec![ContentBlock::text(message.clone())]);
    result.structured_content = Some(json!({
        "error": {
            "code": code,
            "message": message,
        }
    }));
    result
}

pub fn resolve_actor(ctx: &RequestContext<RoleServer>) -> Result<Actor, ErrorData> {
    if let Some(actor) = ctx.extensions.get::<Actor>() {
        return Ok(actor.clone());
    }

    let parts = ctx
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| mcp_error(ErrorCode::INTERNAL_ERROR, "MCP request context missing HTTP parts"))?;

    let actor = parts
        .extensions
        .get::<Actor>()
        .cloned()
        .ok_or_else(|| mcp_error(ErrorCode::INVALID_REQUEST, "Missing MCP authentication"))?;

    Ok(actor)
}

pub async fn authenticate_mcp_request(mut request: Request<Body>, next: Next) -> Response {
    if request.uri().path() != "/mcp" && !request.uri().path().starts_with("/mcp/") {
        return next.run(request).await;
    }
    let token = match bearer_token(&request) {
        Some(token) if token.starts_with("vcms_site_") || token.starts_with("vcms_pat_") => token,
        Some(_) => return auth_response(StatusCode::UNAUTHORIZED, "MCP requires a VCMS access token"),
        None => return auth_response(StatusCode::UNAUTHORIZED, "Missing Authorization bearer token"),
    };
    let repository = match request.extensions().get::<Arc<Repository>>() {
        Some(repository) => repository.clone(),
        None => match request.extensions().get::<Repository>() {
            Some(repository) => Arc::new(repository.clone()),
            None => return auth_response(StatusCode::INTERNAL_SERVER_ERROR, "MCP repository extension missing"),
        },
    };

    let config = match request.extensions().get::<Arc<Config>>() {
        Some(config) => config.clone(),
        None => match request.extensions().get::<Config>() {
            Some(config) => Arc::new(config.clone()),
            None => return auth_response(StatusCode::INTERNAL_SERVER_ERROR, "MCP config extension missing"),
        },
    };

    if !origin_allowed(request.headers(), &config) {
        return auth_response(StatusCode::FORBIDDEN, "MCP Origin is not allowed");
    }

    match verify_access_token(&token, &repository, &config.token_index_key).await {
        Ok(actor) => {
            let has_mcp = match &actor {
                Actor::ApiKey(k) => k.scopes.contains(&crate::models::access_token::TokenScope::McpUse),
                Actor::PersonalToken(k) => k.scopes.contains(&crate::models::access_token::TokenScope::McpUse),
                Actor::User(_) => false,
            };
            if !has_mcp {
                return auth_response(StatusCode::FORBIDDEN, "Token is missing mcp.use scope");
            }
            request.extensions_mut().insert(actor);
            next.run(request).await
        }
        Err((status, Json(error))) => auth_response(status, &error.message),
    }
}

// rmcp treats an empty origin allowlist as disabling validation. Our empty
// list instead permits only same-origin requests, preserving a safe default.
fn origin_allowed(headers: &http::HeaderMap, config: &Config) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Some(origin) = origin.to_str().ok().and_then(|value| url::Url::parse(value).ok()) else {
        return false;
    };
    if !matches!(origin.scheme(), "http" | "https")
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.path() != "/"
        || origin.query().is_some()
        || origin.fragment().is_some()
    {
        return false;
    }
    if !config.mcp_allowed_origins.is_empty() {
        return config
            .mcp_allowed_origins
            .iter()
            .any(|allowed| url::Url::parse(allowed).is_ok_and(|allowed| allowed.origin() == origin.origin()));
    }
    let scheme = if config.trust_proxy_headers {
        headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("http")
    } else {
        config
            .public_url
            .as_deref()
            .and_then(|value| value.split_once("://").map(|(scheme, _)| scheme))
            .unwrap_or("http")
    };
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|host| url::Url::parse(&format!("{scheme}://{host}")).ok())
        .is_some_and(|request_origin| request_origin.origin() == origin.origin())
}

pub async fn mcp_rate_limit_middleware(request: Request<Body>, next: Next) -> Response {
    if request.uri().path() != "/mcp" && !request.uri().path().starts_with("/mcp/") {
        return next.run(request).await;
    }
    let Some(crate::middleware::rate_limit::ApiRateLimiter(limiter)) = request
        .extensions()
        .get::<crate::middleware::rate_limit::ApiRateLimiter>(
    ) else {
        return auth_response(StatusCode::INTERNAL_SERVER_ERROR, "MCP rate limiter unavailable");
    };
    let key = limiter.client_key(&request);
    if limiter.check(&key) {
        next.run(request).await
    } else {
        auth_response(StatusCode::TOO_MANY_REQUESTS, "Too many MCP requests")
    }
}

fn bearer_token(request: &Request<Body>) -> Option<String> {
    let auth_header = request.headers().get("Authorization")?.to_str().ok()?;
    crate::middleware::auth::parse_bearer_header(auth_header).map(str::to_owned)
}

fn auth_response(status: StatusCode, message: &str) -> Response {
    let mut response = (status, Json(json!({ "error": message }))).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    } else if status == StatusCode::FORBIDDEN {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer error=\"insufficient_scope\", scope=\"mcp.use\""),
        );
    }
    response
}

pub fn service_error_to_mcp(error: crate::services::error::ServiceError) -> ErrorData {
    let message = error.error_message();
    let code = match error.status_code() {
        StatusCode::NOT_FOUND => ErrorCode::RESOURCE_NOT_FOUND,
        StatusCode::BAD_REQUEST
        | StatusCode::UNPROCESSABLE_ENTITY
        | StatusCode::CONFLICT
        | StatusCode::PRECONDITION_FAILED => ErrorCode::INVALID_PARAMS,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ErrorCode::INVALID_REQUEST,
        _ => ErrorCode::INTERNAL_ERROR,
    };
    ErrorData::new(code, message, None)
}

#[cfg(test)]
mod tests {
    use crate::database::init_db;
    use crate::middleware::auth::{Actor, is_token_not_expired, verify_access_token};
    use crate::models::access_token::{TokenScope, TokenScopes};
    use crate::repository::Repository;
    use crate::services::access_token::AccessTokenService;

    #[test]
    fn test_is_token_not_expired_no_expiry() {
        assert!(is_token_not_expired(None));
    }

    #[test]
    fn test_is_token_not_expired_future() {
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        assert!(is_token_not_expired(Some(&future)));
    }

    #[test]
    fn test_is_token_not_expired_past() {
        let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!is_token_not_expired(Some(&past)));
    }

    #[tokio::test]
    async fn test_verify_site_access_token() {
        let hmac_secret = "test-hmac-secret";
        let pool = init_db("sqlite::memory:").await.expect("db should initialize");
        let repository = Repository::new(&pool);
        let password_hash = bcrypt::hash("password", bcrypt::DEFAULT_COST).expect("password should hash");
        repository
            .user
            .create("user-123", "mcp-user", "mcp@example.com", &password_hash)
            .await
            .expect("user should be created");
        repository
            .site
            .create_with_storage_profile("site-123", "Test Site", "local-filesystem", "user-123")
            .await
            .expect("site should be created");
        let service = AccessTokenService::new(repository.access_token.clone(), hmac_secret.to_string());
        let token = service
            .create_site_token(
                "site-123",
                "MCP".to_string(),
                [TokenScope::SiteRead].into_iter().collect::<TokenScopes>(),
                None,
                None,
            )
            .await
            .expect("token should be created");

        let actor = verify_access_token(&token.token, &repository, hmac_secret)
            .await
            .expect("token should verify");

        match actor {
            Actor::ApiKey(k) => {
                assert_eq!(k.site_id, "site-123");
                assert!(k.scopes.contains(&TokenScope::SiteRead));
            }
            _ => panic!("expected API key actor"),
        }
    }

    #[tokio::test]
    async fn test_verify_instance_access_token_is_rejected() {
        let hmac_secret = "test-hmac-secret";
        let pool = init_db("sqlite::memory:").await.expect("db should initialize");
        let repository = Repository::new(&pool);
        assert!(
            verify_access_token(
                &format!("{}{}", "cms_", "inst_abcdefghijklmnopqrstuvwxyz"),
                &repository,
                hmac_secret
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn test_dashboard_session_is_not_a_valid_access_token() {
        let hmac_secret = "test-hmac-secret";
        let pool = init_db("sqlite::memory:").await.expect("db should initialize");
        let repository = Repository::new(&pool);

        assert!(
            verify_access_token("opaque-dashboard-session", &repository, hmac_secret)
                .await
                .is_err()
        );
    }
}
