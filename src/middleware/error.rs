use axum::{http::StatusCode, Json};
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
    pub fn instance_token_required() -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(Self {
                error: "instance_token_required".into(),
                message: "This endpoint requires a cms_ik_* token.".into(),
            }),
        )
    }

    pub fn site_token_required() -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(Self {
                error: "site_token_required".into(),
                message: "This endpoint requires a cms_sk_* token.".into(),
            }),
        )
    }

    pub fn insufficient_scope(scope: &str) -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "insufficient_scope".into(),
                message: format!("Token is missing required scope: {}.", scope),
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

    pub fn instance_token_denied() -> (StatusCode, Json<AuthError>) {
        (
            StatusCode::FORBIDDEN,
            Json(Self {
                error: "instance_token_denied".into(),
                message: "Instance tokens cannot access this endpoint.".into(),
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
}
