use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::middleware::auth::Principal;
use crate::models::site::{Site, SiteMember};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{SiteRepository, UserRepository};

#[derive(Clone)]
pub struct SiteService {
    site_repo: Arc<dyn SiteRepository>,
    user_repo: Arc<dyn UserRepository>,
}

#[derive(Error, Debug)]
pub enum SiteError {
    #[error("Not found")]
    NotFound,

    #[error("Invalid storage provider: {0}")]
    InvalidStorageProvider(String),

    #[error("Invalid name: {0}")]
    InvalidName(String),

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
            SiteError::InvalidName(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
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
    pub fn new(site_repo: Arc<dyn SiteRepository>, user_repo: Arc<dyn UserRepository>) -> Self {
        Self { site_repo, user_repo }
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
        self.site_repo
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
        self.site_repo
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
        self.site_repo
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
        debug!(
            "Creating site: name={}, storage_provider={:?}, created_by={}",
            name, storage_provider, created_by
        );

        if name.is_empty() {
            warn!("Site creation failed: name is empty");
            return Err(SiteError::InvalidName("Name is required".into()));
        }

        let storage_provider = storage_provider.unwrap_or("filesystem");
        if storage_provider != "filesystem" && storage_provider != "s3" {
            warn!("Site creation failed: invalid storage_provider={}", storage_provider);
            return Err(SiteError::InvalidStorageProvider(
                "Invalid storage provider. Must be 'filesystem' or 's3'".into(),
            ));
        }

        let site_id = Uuid::now_v7().to_string();
        info!(
            "Creating new site: id={}, name={}, storage_provider={}, created_by={}",
            site_id, name, storage_provider, created_by
        );

        match self
            .site_repo
            .create(&site_id, name, storage_provider, created_by)
            .await
        {
            Ok(site) => {
                info!("Site created successfully: id={}", site.id);
                Ok(site)
            }
            Err(e) => {
                error!("Failed to create site: id={}, error={}", site_id, e);
                Err(SiteError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn update_site(&self, site_id: &str, name: Option<&str>) -> Result<Site, SiteError> {
        debug!("Updating site: id={}, name={:?}", site_id, name);

        let name = name.map(|n| n.trim());
        if let Some(n) = name {
            if n.is_empty() {
                warn!("Site update failed: name is empty");
                return Err(SiteError::InvalidName("Name is required".into()));
            }
        }

        let existing = self
            .site_repo
            .get_by_id(site_id)
            .await
            .map_err(|e| {
                error!("Failed to fetch existing site for update: id={}, error={}", site_id, e);
                SiteError::DatabaseError(e.to_string())
            })?
            .ok_or(SiteError::NotFound)?;

        debug!("Fetched existing site: id={}, name={}", site_id, existing.name);

        let name = name.unwrap_or(&existing.name);
        info!("Updating site name: id={}, from={} to={}", site_id, existing.name, name);

        match self.site_repo.update(site_id, name).await {
            Ok(site) => {
                info!("Site updated successfully: id={}", site.id);
                Ok(site)
            }
            Err(e) => {
                error!("Failed to update site: id={}, error={}", site_id, e);
                Err(SiteError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn delete_site(&self, site_id: &str) -> Result<u64, SiteError> {
        info!("Deleting site: id={}", site_id);

        match self.site_repo.delete(site_id).await {
            Ok(deleted_count) => {
                info!(
                    "Site deleted successfully: id={}, deleted_count={}",
                    site_id, deleted_count
                );
                Ok(deleted_count)
            }
            Err(e) => {
                error!("Failed to delete site: id={}, error={}", site_id, e);
                Err(SiteError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn list_members(&self, site_id: &str) -> Result<Vec<SiteMember>, SiteError> {
        self.site_repo
            .list_members(site_id)
            .await
            .map_err(|e| SiteError::DatabaseError(e.to_string()))
    }

    pub async fn invite_member(&self, site_id: &str, username: &str, role: &str) -> Result<SiteMember, SiteError> {
        debug!(
            "Inviting member to site: site_id={}, username={}, role={}",
            site_id, username, role
        );

        if !VALID_ROLES.contains(&role) {
            warn!("Invite member failed: invalid role={}", role);
            return Err(SiteError::InvalidRole(
                "Invalid role. Must be owner, admin, editor, or viewer".into(),
            ));
        }

        debug!("Looking up user by username: {}", username);
        let user_id = self
            .user_repo
            .find_id_by_username(username)
            .await
            .map_err(|e| {
                error!("Failed to look up user by username={}: error={}", username, e);
                SiteError::DatabaseError(e.to_string())
            })?
            .ok_or(SiteError::UserNotFound)?;

        debug!("Found user: user_id={}", user_id);
        let member_id = Uuid::now_v7().to_string();
        debug!(
            "Adding member to site: member_id={}, site_id={}, user_id={}, role={}",
            member_id, site_id, user_id, role
        );

        match self.site_repo.add_member(&member_id, site_id, &user_id, role).await {
            Ok(member) => {
                info!(
                    "Member invited successfully: member_id={}, site_id={}, user_id={}, role={}",
                    member.id, site_id, user_id, role
                );
                Ok(member)
            }
            Err(e) => {
                error!(
                    "Failed to invite member: site_id={}, user_id={}, role={}, error={}",
                    site_id, user_id, role, e
                );
                Err(match e {
                    RepositoryError::UniqueViolation(_) => SiteError::AlreadyMember,
                    _ => SiteError::DatabaseError(e.to_string()),
                })
            }
        }
    }

    pub async fn update_member_role(
        &self,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<Option<SiteMember>, SiteError> {
        debug!(
            "Updating member role: site_id={}, user_id={}, role={}",
            site_id, user_id, role
        );

        if !VALID_ROLES.contains(&role) {
            warn!("Update member role failed: invalid role={}", role);
            return Err(SiteError::InvalidRole("Invalid role".into()));
        }

        match self.site_repo.update_member_role(site_id, user_id, role).await {
            Ok(Some(member)) => {
                info!(
                    "Member role updated successfully: site_id={}, user_id={}, new_role={}",
                    site_id, user_id, role
                );
                Ok(Some(member))
            }
            Ok(None) => {
                warn!(
                    "Member not found for role update: site_id={}, user_id={}",
                    site_id, user_id
                );
                Ok(None)
            }
            Err(e) => {
                error!(
                    "Failed to update member role: site_id={}, user_id={}, role={}, error={}",
                    site_id, user_id, role, e
                );
                Err(SiteError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn remove_member(&self, site_id: &str, user_id: &str, by_user_id: &str) -> Result<u64, SiteError> {
        debug!(
            "Removing member from site: site_id={}, user_id={}, by_user_id={}",
            site_id, user_id, by_user_id
        );

        if user_id == by_user_id {
            warn!(
                "Remove member failed: user cannot remove themselves: site_id={}, user_id={}",
                site_id, user_id
            );
            return Err(SiteError::CannotRemoveSelf);
        }

        match self.site_repo.remove_member(site_id, user_id).await {
            Ok(removed_count) => {
                info!(
                    "Member removed successfully: site_id={}, user_id={}, removed_count={}",
                    site_id, user_id, removed_count
                );
                Ok(removed_count)
            }
            Err(e) => {
                error!(
                    "Failed to remove member: site_id={}, user_id={}, error={}",
                    site_id, user_id, e
                );
                Err(SiteError::DatabaseError(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::auth::Principal;
    use crate::models::site::Site;
    use crate::test_helpers::{InMemorySiteRepository, InMemoryUserRepository};
    use std::sync::Arc;

    fn test_site_repo() -> Arc<InMemorySiteRepository> {
        Arc::new(InMemorySiteRepository::new())
    }

    fn test_user_repo() -> Arc<InMemoryUserRepository> {
        Arc::new(InMemoryUserRepository::new())
    }

    fn create_test_site() -> Site {
        Site {
            id: "site-123".to_string(),
            name: "Test Site".to_string(),
            storage_provider: "filesystem".to_string(),
            created_by: "user-123".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    #[tokio::test]
    async fn test_list_sites_instance() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        site_repo.add_site(create_test_site());
        let service = SiteService::new(site_repo, user_repo);

        let result = service.list_sites_instance().await;
        assert!(result.is_ok());
        let sites = result.unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0]["name"], "Test Site");
        assert_eq!(sites[0]["role"], "instance_admin");
    }

    #[tokio::test]
    async fn test_list_sites_for_user() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let principal = Principal::UserSession {
            user_id: "user-123".to_string(),
        };
        let result = service.list_sites_for_principal(&principal).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_site_found() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        site_repo.add_site(create_test_site());
        let service = SiteService::new(site_repo, user_repo);

        let result = service.get_site("site-123").await;
        assert!(result.is_ok());
        let site = result.unwrap();
        assert!(site.is_some());
        assert_eq!(site.unwrap().name, "Test Site");
    }

    #[tokio::test]
    async fn test_get_site_not_found() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.get_site("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_create_site_success() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.create_site("My New Site", Some("filesystem"), "user-123").await;
        assert!(result.is_ok());
        let site = result.unwrap();
        assert_eq!(site.name, "My New Site");
        assert_eq!(site.storage_provider, "filesystem");
    }

    #[tokio::test]
    async fn test_create_site_default_storage() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.create_site("My Site", None, "user-123").await;
        assert!(result.is_ok());
        let site = result.unwrap();
        assert_eq!(site.storage_provider, "filesystem");
    }

    #[tokio::test]
    async fn test_create_site_s3_provider() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.create_site("My S3 Site", Some("s3"), "user-123").await;
        assert!(result.is_ok());
        let site = result.unwrap();
        assert_eq!(site.storage_provider, "s3");
    }

    #[tokio::test]
    async fn test_create_site_empty_name() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.create_site("", Some("filesystem"), "user-123").await;
        assert!(matches!(result, Err(SiteError::InvalidName(msg)) if msg.contains("Name is required")));
    }

    #[tokio::test]
    async fn test_create_site_whitespace_name() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.create_site("   ", Some("filesystem"), "user-123").await;
        assert!(matches!(result, Err(SiteError::InvalidName(msg)) if msg.contains("Name is required")));
    }

    #[tokio::test]
    async fn test_create_site_invalid_provider() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.create_site("My Site", Some("invalid"), "user-123").await;
        assert!(
            matches!(result, Err(SiteError::InvalidStorageProvider(msg)) if msg.contains("Invalid storage provider"))
        );
    }

    #[tokio::test]
    async fn test_update_site_success() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        site_repo.add_site(create_test_site());
        let service = SiteService::new(site_repo, user_repo);

        let result = service.update_site("site-123", Some("Updated Site")).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Updated Site");
    }

    #[tokio::test]
    async fn test_update_site_not_found() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.update_site("nonexistent", Some("Updated")).await;
        assert!(matches!(result, Err(SiteError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_site_no_name_provided() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        site_repo.add_site(create_test_site());
        let service = SiteService::new(site_repo, user_repo);

        let result = service.update_site("site-123", None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Test Site");
    }

    #[tokio::test]
    async fn test_delete_site_success() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        site_repo.add_site(create_test_site());
        let service = SiteService::new(site_repo, user_repo);

        let result = service.delete_site("site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete_site_not_found() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.delete_site("nonexistent").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_list_members() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.list_members("site-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invite_member_invalid_role() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.invite_member("site-123", "username", "invalid_role").await;
        assert!(matches!(result, Err(SiteError::InvalidRole(msg)) if msg.contains("owner, admin, editor, or viewer")));
    }

    #[tokio::test]
    async fn test_invite_member_user_not_found() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.invite_member("site-123", "nonexistent_user", "viewer").await;
        assert!(matches!(result, Err(SiteError::UserNotFound)));
    }

    #[tokio::test]
    async fn test_invite_member_success() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();

        user_repo.add_user(crate::models::user::User {
            id: "user-456".to_string(),
            username: "newuser".to_string(),
            email: "new@example.com".to_string(),
            password_hash: "hash".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        });

        let service = SiteService::new(site_repo, user_repo.clone());

        let result = service.invite_member("site-123", "newuser", "editor").await;
        assert!(result.is_ok());
        let member = result.unwrap();
        assert_eq!(member.role, "editor");
    }

    #[tokio::test]
    async fn test_invite_member_all_valid_roles() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();

        for role in ["owner", "admin", "editor", "viewer"] {
            user_repo.add_user(crate::models::user::User {
                id: format!("user-{}", role),
                username: format!("user_{}", role),
                email: format!("{}@example.com", role),
                password_hash: "hash".to_string(),
                created_at: "2024-01-01 00:00:00".to_string(),
                updated_at: "2024-01-01 00:00:00".to_string(),
            });
        }

        let service = SiteService::new(site_repo, user_repo.clone());

        for role in ["owner", "admin", "editor", "viewer"] {
            let result = service.invite_member("site-123", &format!("user_{}", role), role).await;
            assert!(result.is_ok(), "Failed for role: {}", role);
        }
    }

    #[tokio::test]
    async fn test_update_member_role_invalid_role() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.update_member_role("site-123", "user-456", "invalid").await;
        assert!(matches!(result, Err(SiteError::InvalidRole(msg)) if msg.contains("Invalid role")));
    }

    #[tokio::test]
    async fn test_update_member_role_success() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.update_member_role("site-123", "user-456", "admin").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_remove_member_cannot_remove_self() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.remove_member("site-123", "user-123", "user-123").await;
        assert!(matches!(result, Err(SiteError::CannotRemoveSelf)));
    }

    #[tokio::test]
    async fn test_remove_member_success() {
        let site_repo = test_site_repo();
        let user_repo = test_user_repo();
        let service = SiteService::new(site_repo, user_repo);

        let result = service.remove_member("site-123", "user-456", "user-123").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_site_error_into_response() {
        assert_eq!(
            SiteError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            SiteError::InvalidStorageProvider("bad".into()).into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            SiteError::InvalidName("bad".into()).into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            SiteError::InvalidRole("bad".into()).into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            SiteError::CannotRemoveSelf.into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            SiteError::UserNotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            SiteError::AlreadyMember.into_response().status(),
            axum::http::StatusCode::CONFLICT
        );
        assert_eq!(
            SiteError::MemberNotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            SiteError::DatabaseError("bad".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
