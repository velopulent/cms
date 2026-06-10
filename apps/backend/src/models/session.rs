use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub csrf_token_hash: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_seen_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_seen_at: String,
    pub current: bool,
}
