use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub storage_provider: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct SiteWithRole {
    pub id: String,
    pub name: String,
    pub storage_provider: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub role: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateSite {
    pub name: String,
    pub storage_provider: String,
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
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct InviteMember {
    pub username: String,
    pub role: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMemberRole {
    pub role: String,
}
