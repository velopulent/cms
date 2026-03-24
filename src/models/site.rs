use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, FromRow)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, FromRow)]
pub struct SiteWithRole {
    pub id: String,
    pub name: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct CreateSite {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateSite {
    pub name: Option<String>,
}

#[derive(Serialize, FromRow)]
pub struct SiteMember {
    pub id: String,
    pub site_id: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct InviteMember {
    pub username: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct UpdateMemberRole {
    pub role: String,
}
