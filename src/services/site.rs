use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::middleware::auth::Principal;
use crate::models::site::{Site, SiteMember};
use crate::repository::Repository;
use crate::repository::error::RepositoryError;

#[derive(Clone)]
pub struct SiteService {
    repository: Arc<Repository>,
}

#[derive(Error, Debug)]
pub enum SiteError {
    #[error("Not found")]
    NotFound,

    #[error("Invalid storage provider: {0}")]
    InvalidStorageProvider(String),

    #[error("Invalid role: {0}")]
    InvalidRole(String),

    #[error("Cannot remove yourself from the site")]
    CannotRemoveSelf,

    #[error("User not found")]
    UserNotFound,

    #[error("User is already a member of this site")]
    AlreadyMember,

    #[error("Member not found")]
    MemberNotFound,

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl SiteError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            SiteError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))),
            SiteError::InvalidStorageProvider(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            SiteError::InvalidRole(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            SiteError::CannotRemoveSelf => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Cannot remove yourself from the site"})),
            ),
            SiteError::UserNotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))),
            SiteError::AlreadyMember => (
                StatusCode::CONFLICT,
                Json(json!({"error": "User is already a member of this site"})),
            ),
            SiteError::MemberNotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))),
            SiteError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))),
        };
        (status, body).into_response()
    }
}

const VALID_ROLES: [&str; 4] = ["owner", "admin", "editor", "viewer"];

impl SiteService {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }

    pub async fn list_sites_for_principal(&self, principal: &Principal) -> Result<Vec<serde_json::Value>, SiteError> {
        match principal {
            Principal::InstanceToken { .. } => self.list_sites_instance().await,
            Principal::UserSession { user_id } => self.list_sites_for_user(user_id).await,
            Principal::SiteToken { .. } => {
                unreachable!("SiteToken should not be used for listing sites")
            }
        }
    }

    pub async fn list_sites_instance(&self) -> Result<Vec<serde_json::Value>, SiteError> {
        self.repository
            .site
            .list_all()
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
            .map(|sites| {
                sites
                    .into_iter()
                    .map(|site| {
                        json!({
                            "id": site.id,
                            "name": site.name,
                            "storage_provider": site.storage_provider,
                            "created_by": site.created_by,
                            "created_at": site.created_at,
                            "updated_at": site.updated_at,
                            "role": "instance_admin",
                        })
                    })
                    .collect()
            })
    }

    pub async fn list_sites_for_user(&self, user_id: &str) -> Result<Vec<serde_json::Value>, SiteError> {
        self.repository
            .site
            .list_for_user(user_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
            .map(|sites| {
                sites
                    .into_iter()
                    .map(|site| serde_json::to_value(site).unwrap_or_default())
                    .collect()
            })
    }

    pub async fn get_site(&self, site_id: &str) -> Result<Option<Site>, SiteError> {
        self.repository
            .site
            .get_by_id(site_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn create_site(
        &self,
        name: &str,
        storage_provider: Option<&str>,
        created_by: &str,
    ) -> Result<Site, SiteError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SiteError::InvalidStorageProvider("Name is required".into()));
        }

        let storage_provider = storage_provider.unwrap_or("filesystem");
        if storage_provider != "filesystem" && storage_provider != "s3" {
            return Err(SiteError::InvalidStorageProvider(
                "Invalid storage provider. Must be 'filesystem' or 's3'".into(),
            ));
        }

        let site_id = Uuid::now_v7().to_string();

        self.repository
            .site
            .create(&site_id, name, storage_provider, created_by)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn update_site(
        &self,
        site_id: &str,
        name: Option<&str>,
        storage_provider: Option<&str>,
    ) -> Result<Site, SiteError> {
        let existing = self
            .repository
            .site
            .get_by_id(site_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))?
            .ok_or(SiteError::NotFound)?;

        let name = name.unwrap_or(&existing.name);
        let storage_provider = storage_provider
            .filter(|v| *v == "filesystem" || *v == "s3")
            .unwrap_or(&existing.storage_provider);

        self.repository
            .site
            .update(site_id, name, storage_provider)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn delete_site(&self, site_id: &str) -> Result<u64, SiteError> {
        self.repository
            .site
            .delete(site_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn list_members(&self, site_id: &str) -> Result<Vec<SiteMember>, SiteError> {
        self.repository
            .site
            .list_members(site_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn invite_member(&self, site_id: &str, username: &str, role: &str) -> Result<SiteMember, SiteError> {
        if !VALID_ROLES.contains(&role) {
            return Err(SiteError::InvalidRole(
                "Invalid role. Must be owner, admin, editor, or viewer".into(),
            ));
        }

        let user_id = self
            .repository
            .user
            .find_id_by_username(username)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))?
            .ok_or(SiteError::UserNotFound)?;

        let member_id = Uuid::now_v7().to_string();

        self.repository
            .site
            .add_member(&member_id, site_id, &user_id, role)
            .await
            .map_err(|e| match e {
                RepositoryError::UniqueViolation(_) => SiteError::AlreadyMember,
                _ => SiteError::DatabaseError(e.to_string()),
            })
    }

    pub async fn update_member_role(
        &self,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<Option<SiteMember>, SiteError> {
        if !VALID_ROLES.contains(&role) {
            return Err(SiteError::InvalidRole("Invalid role".into()));
        }

        self.repository
            .site
            .update_member_role(site_id, user_id, role)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn remove_member(&self, site_id: &str, user_id: &str, by_user_id: &str) -> Result<u64, SiteError> {
        if user_id == by_user_id {
            return Err(SiteError::CannotRemoveSelf);
        }

        self.repository
            .site
            .remove_member(site_id, user_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }
}
