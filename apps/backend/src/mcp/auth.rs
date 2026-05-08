use std::str::FromStr;

use crate::config::Config;
use crate::middleware::auth::{Principal, verify_token, compute_key_hmac, is_token_not_expired, parse_scopes};
use crate::models::access_token::AccessTokenKind;
use crate::repository::Repository;
use rmcp::model::ErrorCode;
use rmcp::model::ErrorData;

type McpResult<T> = Result<T, ErrorData>;

fn mcp_error(code: ErrorCode, message: impl Into<String>) -> ErrorData {
    ErrorData::new(code, message.into(), None)
}

pub async fn resolve_principal_from_env(
    config: &Config,
    repository: &Repository,
) -> McpResult<Principal> {
    let token = std::env::var("CMS_TOKEN").unwrap_or_default();
    if token.is_empty() {
        return Err(mcp_error(ErrorCode::INVALID_PARAMS, "CMS_TOKEN environment variable is required for stdio MCP transport"));
    }
    resolve_principal_from_token(&token, config, repository).await
}

async fn resolve_principal_from_token(
    token: &str,
    config: &Config,
    repository: &Repository,
) -> McpResult<Principal> {
    if token.starts_with("cms_") {
        let prefix: String = token.chars().take(24).collect();
        let keys = repository.access_token.find_by_prefix(&prefix).await
            .map_err(|_| mcp_error(ErrorCode::INTERNAL_ERROR, "Failed to look up access token"))?;

        let token_hmac = compute_key_hmac(token, &config.hmac_secret);

        for (token_id, kind, site_id, stored_hash, stored_hmac, expires_at, revoked_at, scopes) in keys {
            if let Some(ref stored) = stored_hmac {
                if stored != &token_hmac {
                    continue;
                }
            } else {
                match bcrypt::verify(token, &stored_hash) {
                    Ok(true) => {}
                    _ => continue,
                }
            }

            if revoked_at.is_some() {
                return Err(mcp_error(ErrorCode::INVALID_REQUEST, "Access token has been revoked"));
            }

            if !is_token_not_expired(expires_at.as_deref()) {
                return Err(mcp_error(ErrorCode::INVALID_REQUEST, "Access token has expired"));
            }

            let _ = repository.access_token.update_last_used(&token_id).await;

            let kind_parsed = AccessTokenKind::from_str(&kind)
                .map_err(|e| mcp_error(ErrorCode::INTERNAL_ERROR, e))?;
            let scopes = parse_scopes(&scopes);

            return match kind_parsed {
                AccessTokenKind::Instance => Ok(Principal::InstanceToken { token_id, scopes }),
                AccessTokenKind::Site => {
                    let site_id = site_id.ok_or_else(|| mcp_error(ErrorCode::INTERNAL_ERROR, "Site token missing site binding"))?;
                    Ok(Principal::SiteToken { token_id, site_id, scopes })
                }
            };
        }

        return Err(mcp_error(ErrorCode::INVALID_REQUEST, "Invalid access token"));
    }

    verify_token(token, &config.jwt_secret)
        .map(|claims| Principal::UserSession { user_id: claims.sub })
        .map_err(|_| mcp_error(ErrorCode::INVALID_REQUEST, "Invalid or expired token"))
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
    use super::*;

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
}