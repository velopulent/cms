use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct DeploymentTrigger {
    pub id: String,
    pub site_id: String,
    pub label: String,
    pub provider: String,
    pub enabled: bool,
    pub is_primary: bool,
    pub cooldown_seconds: i64,
    pub daily_quota: i64,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDeploymentTrigger {
    pub label: String,
    pub provider: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: i64,
    #[serde(default = "default_quota")]
    pub daily_quota: i64,
}
fn default_true() -> bool {
    true
}
fn default_cooldown() -> i64 {
    60
}
fn default_quota() -> i64 {
    20
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct DeploymentJob {
    pub id: String,
    pub trigger_id: String,
    pub site_id: String,
    pub status: String,
    pub status_code: Option<i32>,
    pub error_category: Option<String>,
    pub response_body: Option<String>,
    pub retry_after_seconds: Option<i64>,
    pub duration_ms: Option<i64>,
    pub triggered_by: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
