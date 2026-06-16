use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;

use crate::services::access_token::TokenError;
use crate::services::collection::CollectionError;
use crate::services::entry::EntryError;
use crate::services::file::FileError;
use crate::services::singleton::SingletonError;
use crate::services::site::SiteError;
use crate::services::webhook::WebhookError;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Insufficient permission: {0}")]
    InsufficientPermission(String),

    #[error("Site token denied")]
    SiteTokenDenied,

    #[error("Missing site context")]
    MissingSiteContext,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Site error: {0}")]
    Site(#[from] SiteError),

    #[error("Collection error: {0}")]
    Collection(#[from] CollectionError),

    #[error("Entry error: {0}")]
    Entry(#[from] EntryError),

    #[error("File error: {0}")]
    File(#[from] FileError),

    #[error("Singleton error: {0}")]
    Singleton(#[from] SingletonError),

    #[error("Webhook error: {0}")]
    Webhook(#[from] WebhookError),

    #[error("Token error: {0}")]
    Token(#[from] TokenError),
}

impl ServiceError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ServiceError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ServiceError::Forbidden(_) => StatusCode::FORBIDDEN,
            ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            ServiceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServiceError::Conflict(_) => StatusCode::CONFLICT,
            ServiceError::InsufficientPermission(_) => StatusCode::FORBIDDEN,
            ServiceError::SiteTokenDenied => StatusCode::FORBIDDEN,
            ServiceError::MissingSiteContext => StatusCode::BAD_REQUEST,
            ServiceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::Site(e) => match e {
                SiteError::NotFound => StatusCode::NOT_FOUND,
                SiteError::InvalidStorageProvider(_) | SiteError::InvalidName(_) | SiteError::InvalidRole(_) => {
                    StatusCode::BAD_REQUEST
                }
                SiteError::CannotRemoveSelf => StatusCode::BAD_REQUEST,
                SiteError::UserNotFound => StatusCode::NOT_FOUND,
                SiteError::AlreadyMember => StatusCode::CONFLICT,
                SiteError::CannotInviteOperator => StatusCode::BAD_REQUEST,
                SiteError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ServiceError::Collection(e) => match e {
                CollectionError::NotFound => StatusCode::NOT_FOUND,
                CollectionError::AlreadyExists => StatusCode::CONFLICT,
                CollectionError::InvalidDefinition(_) => StatusCode::BAD_REQUEST,
                CollectionError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ServiceError::Entry(e) => match e {
                EntryError::NotFound => StatusCode::NOT_FOUND,
                EntryError::RevisionNotFound => StatusCode::NOT_FOUND,
                EntryError::AlreadyExists => StatusCode::CONFLICT,
                EntryError::ValidationFailed(_) => StatusCode::BAD_REQUEST,
                EntryError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ServiceError::File(e) => match e {
                FileError::NotFound => StatusCode::NOT_FOUND,
                FileError::NotFoundOrNotDeleted => StatusCode::NOT_FOUND,
                FileError::NoFileProvided => StatusCode::BAD_REQUEST,
                FileError::FileTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
                FileError::InvalidContentType(_) => StatusCode::BAD_REQUEST,
                FileError::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                FileError::NoStorageConfigured => StatusCode::INTERNAL_SERVER_ERROR,
                FileError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ServiceError::Singleton(e) => match e {
                SingletonError::NotFound | SingletonError::NotASingleton => StatusCode::NOT_FOUND,
                SingletonError::ValidationFailed(_) => StatusCode::BAD_REQUEST,
                SingletonError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ServiceError::Webhook(e) => match e {
                WebhookError::NotFound => StatusCode::NOT_FOUND,
                WebhookError::InvalidUrl(_) | WebhookError::InvalidLabel(_) => StatusCode::BAD_REQUEST,
                WebhookError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
                WebhookError::DeliveryFailed(_) => StatusCode::BAD_GATEWAY,
            },
            ServiceError::Token(e) => match e {
                TokenError::NameRequired => StatusCode::BAD_REQUEST,
                TokenError::NotFound => StatusCode::NOT_FOUND,
                TokenError::HashError(_) | TokenError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
        }
    }

    pub fn error_message(&self) -> String {
        match self {
            ServiceError::Unauthorized(msg) => msg.clone(),
            ServiceError::Forbidden(msg) => msg.clone(),
            ServiceError::NotFound(msg) => msg.clone(),
            ServiceError::BadRequest(msg) => msg.clone(),
            ServiceError::Conflict(msg) => msg.clone(),
            ServiceError::InsufficientPermission(permission) => format!("Insufficient permission: {}", permission),
            ServiceError::SiteTokenDenied => "Site token denied".to_string(),
            ServiceError::MissingSiteContext => "Missing site context".to_string(),
            ServiceError::Internal(_) => "Internal server error".to_string(),
            ServiceError::Site(e) => e.to_string(),
            ServiceError::Collection(e) => e.to_string(),
            ServiceError::Entry(e) => e.to_string(),
            ServiceError::File(e) => e.to_string(),
            ServiceError::Singleton(e) => e.to_string(),
            ServiceError::Webhook(e) => e.to_string(),
            ServiceError::Token(e) => e.to_string(),
        }
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        // Log full error details server-side
        if matches!(self, ServiceError::Internal(_)) {
            tracing::error!("Internal error: {}", self);
        }
        let status = self.status_code();
        let message = self.error_message();
        (status, Json(json!({"error": message}))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_code_mapping() {
        assert_eq!(
            ServiceError::Unauthorized("".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(ServiceError::Forbidden("".into()).status_code(), StatusCode::FORBIDDEN);
        assert_eq!(ServiceError::NotFound("".into()).status_code(), StatusCode::NOT_FOUND);
        assert_eq!(
            ServiceError::BadRequest("".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(ServiceError::Conflict("".into()).status_code(), StatusCode::CONFLICT);
        assert_eq!(
            ServiceError::InsufficientPermission("".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(ServiceError::SiteTokenDenied.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(ServiceError::MissingSiteContext.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(
            ServiceError::Internal("".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_message() {
        assert_eq!(ServiceError::Unauthorized("msg".into()).error_message(), "msg");
        assert_eq!(ServiceError::Forbidden("msg".into()).error_message(), "msg");
        assert_eq!(ServiceError::NotFound("msg".into()).error_message(), "msg");
        assert_eq!(ServiceError::BadRequest("msg".into()).error_message(), "msg");
        assert_eq!(ServiceError::Conflict("msg".into()).error_message(), "msg");
        assert_eq!(
            ServiceError::InsufficientPermission("admin".into()).error_message(),
            "Insufficient permission: admin"
        );
        assert_eq!(ServiceError::SiteTokenDenied.error_message(), "Site token denied");
        assert_eq!(ServiceError::MissingSiteContext.error_message(), "Missing site context");
        assert_eq!(
            ServiceError::Internal("secret".into()).error_message(),
            "Internal server error"
        );
    }

    #[test]
    fn test_status_codes_for_domain_errors() {
        assert_eq!(
            ServiceError::Site(SiteError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::Site(SiteError::InvalidStorageProvider("x".into())).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Site(SiteError::AlreadyMember).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ServiceError::Collection(CollectionError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::Collection(CollectionError::AlreadyExists).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ServiceError::Collection(CollectionError::InvalidDefinition("x".into())).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Entry(EntryError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::Entry(EntryError::AlreadyExists).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ServiceError::Entry(EntryError::ValidationFailed("x".into())).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::File(FileError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::File(FileError::FileTooLarge("too large".into())).status_code(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            ServiceError::File(FileError::InvalidContentType("x".into())).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Singleton(SingletonError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::Singleton(SingletonError::NotASingleton).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::Singleton(SingletonError::ValidationFailed("x".into())).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Webhook(WebhookError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::Webhook(WebhookError::InvalidUrl("x".into())).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Webhook(WebhookError::DeliveryFailed("x".into())).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            ServiceError::Token(TokenError::NameRequired).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Token(TokenError::NotFound).status_code(),
            StatusCode::NOT_FOUND
        );
    }
}
