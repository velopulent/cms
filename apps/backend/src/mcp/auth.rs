use axum::{
    Json,
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rmcp::model::{CallToolResult, Content, ErrorCode, ErrorData};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::json;

use crate::config::Config;
use crate::middleware::auth::{Principal, verify_access_token};
use crate::repository::Repository;

pub fn mcp_error(code: ErrorCode, message: impl Into<String>) -> ErrorData {
    ErrorData::new(code, message.into(), None)
}

pub fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, ErrorData> {
    let json = serde_json::to_string_pretty(data).map_err(|e| {
        mcp_error(
            ErrorCode::INTERNAL_ERROR,
            format!("Failed to serialize response: {}", e),
        )
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

pub fn text_result(message: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![Content::text(message.into())])
}

pub fn map_err(e: impl Into<crate::services::error::ServiceError>) -> ErrorData {
    service_error_to_mcp(e.into())
}

pub fn resolve_principal(ctx: &RequestContext<RoleServer>) -> Result<Principal, ErrorData> {
    let parts = ctx
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| mcp_error(ErrorCode::INTERNAL_ERROR, "MCP request context missing HTTP parts"))?;

    parts
        .extensions
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| mcp_error(ErrorCode::INVALID_REQUEST, "Missing MCP authentication"))
}

pub async fn authenticate_mcp_request(mut request: Request<Body>, next: Next) -> Response {
    let repository = match request.extensions().get::<Repository>() {
        Some(repository) => repository.clone(),
        None => return auth_response(StatusCode::INTERNAL_SERVER_ERROR, "MCP repository extension missing"),
    };

    let config = match request.extensions().get::<Config>() {
        Some(config) => config.clone(),
        None => return auth_response(StatusCode::INTERNAL_SERVER_ERROR, "MCP config extension missing"),
    };

    let token = match bearer_token(&request) {
        Some(token) if token.starts_with("cms_site_") || token.starts_with("cms_inst_") => token,
        Some(_) => return auth_response(StatusCode::UNAUTHORIZED, "MCP requires a CMS access token"),
        None => return auth_response(StatusCode::UNAUTHORIZED, "Missing Authorization bearer token"),
    };

    match verify_access_token(&token, &repository, &config.hmac_secret).await {
        Ok(principal) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        Err((status, Json(error))) => auth_response(status, &error.message),
    }
}

fn bearer_token(request: &Request<Body>) -> Option<String> {
    let auth_header = request.headers().get("Authorization")?.to_str().ok()?;
    let token = auth_header.strip_prefix("Bearer ")?;
    Some(token.trim().to_string())
}

fn auth_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

pub fn service_error_to_mcp(error: crate::services::error::ServiceError) -> ErrorData {
    let message = error.error_message();
    let code = match &error {
        crate::services::error::ServiceError::Unauthorized(_) => ErrorCode::INVALID_REQUEST,
        crate::services::error::ServiceError::Forbidden(_)
        | crate::services::error::ServiceError::InsufficientScope(_)
        | crate::services::error::ServiceError::SiteTokenDenied
        | crate::services::error::ServiceError::InstanceTokenDenied => ErrorCode::INVALID_REQUEST,
        crate::services::error::ServiceError::NotFound(_) => ErrorCode::RESOURCE_NOT_FOUND,
        crate::services::error::ServiceError::BadRequest(_) => ErrorCode::INVALID_PARAMS,
        crate::services::error::ServiceError::Conflict(_) => ErrorCode::INVALID_PARAMS,
        crate::services::error::ServiceError::MissingSiteContext => ErrorCode::INVALID_PARAMS,
        crate::services::error::ServiceError::Internal(_) => ErrorCode::INTERNAL_ERROR,
        _ => ErrorCode::INTERNAL_ERROR,
    };
    ErrorData::new(code, message, None)
}

#[cfg(test)]
mod tests {
    use crate::database::init_db;
    use crate::middleware::auth::{Principal, create_token, is_token_not_expired, verify_access_token};
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
            .create("site-123", "Test Site", "filesystem", "user-123")
            .await
            .expect("site should be created");
        let service = AccessTokenService::new(repository.access_token.clone(), hmac_secret.to_string());
        let token = service
            .create_site_token("site-123", "MCP".to_string(), vec!["content:read".to_string()], None)
            .await
            .expect("token should be created");

        let principal = verify_access_token(&token.token, &repository, hmac_secret)
            .await
            .expect("token should verify");

        match principal {
            Principal::SiteToken { site_id, scopes, .. } => {
                assert_eq!(site_id, "site-123");
                assert!(scopes.contains("content:read"));
            }
            other => panic!("expected site token, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_verify_instance_access_token() {
        let hmac_secret = "test-hmac-secret";
        let pool = init_db("sqlite::memory:").await.expect("db should initialize");
        let repository = Repository::new(&pool);
        let service = AccessTokenService::new(repository.access_token.clone(), hmac_secret.to_string());
        let token = service
            .create_instance_token("MCP".to_string(), vec!["sites:read".to_string()])
            .await
            .expect("token should be created");

        let principal = verify_access_token(&token.token, &repository, hmac_secret)
            .await
            .expect("token should verify");

        assert!(matches!(principal, Principal::InstanceToken { .. }));
    }

    #[tokio::test]
    async fn test_jwt_is_not_a_valid_access_token() {
        let hmac_secret = "test-hmac-secret";
        let pool = init_db("sqlite::memory:").await.expect("db should initialize");
        let repository = Repository::new(&pool);
        let jwt = create_token("user-123".to_string(), "jwt-secret").expect("jwt should be created");

        assert!(verify_access_token(&jwt, &repository, hmac_secret).await.is_err());
    }
}
