use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::repository::error::RepositoryError;

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    PreconditionFailed(String),
    TooManyRequests(String),
    BadGateway(String),
    Internal(String),
}

/// RFC 9457-compatible public problem envelope. The legacy `error` member is
/// retained as human-readable text for existing dashboard clients. The `code`
/// member provides a stable machine-readable value.
pub fn problem_response(status: StatusCode, code: &str, detail: impl Into<String>) -> Response {
    let detail = detail.into();
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/problem+json")],
        Json(json!({
            "type": "about:blank",
            "title": status.canonical_reason().unwrap_or("Request error"),
            "status": status.as_u16(),
            "code": code,
            "error": detail,
            "detail": detail,
        })),
    )
        .into_response()
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            AppError::PreconditionFailed(msg) => (StatusCode::PRECONDITION_FAILED, "precondition_failed", msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, "rate_limited", msg),
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, "bad_gateway", msg),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Internal server error".to_string(),
            ),
        };
        problem_response(status, code, message)
    }
}

impl From<RepositoryError> for AppError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::PreconditionFailed => AppError::PreconditionFailed("Version precondition failed".into()),
            RepositoryError::NotFound => AppError::NotFound("Resource not found".into()),
            RepositoryError::UniqueViolation(msg) => AppError::Conflict(msg),
            RepositoryError::Database(msg) => AppError::Internal(msg),
        }
    }
}
