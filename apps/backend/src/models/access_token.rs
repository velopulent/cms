use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessTokenPermission {
    Read,
    Write,
}

pub type ApiKeyPermission = AccessTokenPermission;

impl AccessTokenPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    pub fn can_write(&self) -> bool {
        matches!(self, Self::Write)
    }
}

impl std::fmt::Display for AccessTokenPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AccessTokenPermission {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            other => Err(format!("Unknown token permission '{}'", other)),
        }
    }
}

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct AccessToken {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub token_prefix: String,
    pub permission: String,
    pub created_by_user_id: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    #[sqlx(skip)]
    pub token_hmac: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateSiteToken {
    pub name: String,
    pub permission: AccessTokenPermission,
}

#[derive(Serialize, ToSchema)]
pub struct AccessTokenResponse {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub token: String,
    pub token_prefix: String,
    pub permission: String,
    pub created_at: String,
}
