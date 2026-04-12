use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessTokenKind {
    Instance,
    Site,
}

impl AccessTokenKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Instance => "instance",
            Self::Site => "site",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Instance => "cms_inst_",
            Self::Site => "cms_site_",
        }
    }
}

impl std::fmt::Display for AccessTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AccessTokenKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "instance" => Ok(Self::Instance),
            "site" => Ok(Self::Site),
            other => Err(format!("Unknown token kind '{}'", other)),
        }
    }
}

#[derive(Serialize, FromRow, ToSchema)]
pub struct AccessToken {
    pub id: String,
    pub kind: String,
    pub site_id: Option<String>,
    pub name: String,
    pub token_prefix: String,
    pub scopes: String,
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
    pub scopes: Option<Vec<String>>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateInstanceToken {
    pub name: String,
    pub scopes: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub struct AccessTokenResponse {
    pub id: String,
    pub kind: String,
    pub site_id: Option<String>,
    pub name: String,
    pub token: String,
    pub token_prefix: String,
    pub scopes: Vec<String>,
    pub created_at: String,
}
