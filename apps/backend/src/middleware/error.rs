use axum::{Json, http::StatusCode};
use serde::Serialize;

pub fn unauthorized_error(message: &str) -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, message.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

impl AuthError {
    pub fn site_token_required() -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(Self {
                error: "site_token_required".into(),
                message: "This endpoint requires a vcms_site_* token.".into(),
            }),
        )
    }

    pub fn insufficient_permission(permission: &str) -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "insufficient_permission".into(),
                message: format!("Token requires '{}' permission.", permission),
            }),
        )
    }

    pub fn insufficient_role(role: &str) -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "insufficient_role".into(),
                message: format!("This action requires the '{}' role or higher.", role),
            }),
        )
    }

    pub fn site_token_denied() -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "site_token_denied".into(),
                message: "Site tokens cannot access this endpoint.".into(),
            }),
        )
    }

    pub fn unauthorized(message: &str) -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(Self {
                error: "unauthorized".into(),
                message: message.into(),
            }),
        )
    }

    pub fn forbidden(message: &str) -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "forbidden".into(),
                message: message.into(),
            }),
        )
    }

    pub fn csrf_error(message: &str) -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "csrf_error".into(),
                message: message.into(),
            }),
        )
    }
}
