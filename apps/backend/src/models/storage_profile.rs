use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
#[derive(Clone, Debug, Serialize, FromRow, ToSchema)]
pub struct StorageProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub bucket: Option<String>,
    pub public_url: Option<String>,
    pub enabled: bool,
    pub immutable: bool,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateStorageProfile {
    pub name: String,
    pub endpoint: String,
    pub region: Option<String>,
    pub bucket: String,
    pub public_url: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateStorageProfile {
    pub name: String,
    pub endpoint: String,
    pub region: Option<String>,
    pub bucket: String,
    pub public_url: Option<String>,
    pub enabled: bool,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StorageProbeResult {
    pub ok: bool,
}
