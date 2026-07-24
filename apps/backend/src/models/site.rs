use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub storage_provider: String,
    #[sqlx(default)]
    pub storage_profile_id: Option<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct SiteWithRole {
    pub id: String,
    pub name: String,
    pub storage_provider: String,
    #[sqlx(default)]
    pub storage_profile_id: Option<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub role: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateSite {
    pub name: String,
    #[serde(default = "default_storage_kind")]
    pub storage_provider: String,
    pub storage_profile_id: Option<String>,
}
fn default_storage_kind() -> String {
    "filesystem".into()
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSite {
    pub name: Option<String>,
}

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct SiteMember {
    pub id: String,
    pub site_id: String,
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct InviteMember {
    /// Email of the existing user to add (email is the login identity).
    pub email: String,
    pub role: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMemberRole {
    pub role: String,
}
