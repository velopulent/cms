use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
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
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({"error": message}));
        (status, body).into_response()
    }
}

impl From<RepositoryError> for AppError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound => AppError::NotFound("Resource not found".into()),
            RepositoryError::UniqueViolation(msg) => AppError::Conflict(msg),
            RepositoryError::Database(msg) => AppError::Internal(msg),
        }
    }
}
