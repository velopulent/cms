use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessTokenPermission {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
pub enum TokenScope {
    #[serde(rename = "site.read")]
    SiteRead,
    #[serde(rename = "site.settings.read")]
    SiteSettingsRead,
    #[serde(rename = "site.settings.write")]
    SiteSettingsWrite,
    #[serde(rename = "content.read")]
    ContentRead,
    #[serde(rename = "content.write")]
    ContentWrite,
    #[serde(rename = "files.read")]
    FilesRead,
    #[serde(rename = "files.write")]
    FilesWrite,
    #[serde(rename = "schema.read")]
    SchemaRead,
    #[serde(rename = "schema.write")]
    SchemaWrite,
    #[serde(rename = "webhooks.read")]
    WebhooksRead,
    #[serde(rename = "webhooks.write")]
    WebhooksWrite,
    #[serde(rename = "webhooks.trigger")]
    WebhooksTrigger,
    #[serde(rename = "deployments.read")]
    DeploymentsRead,
    #[serde(rename = "deployments.write")]
    DeploymentsWrite,
    #[serde(rename = "deployments.trigger")]
    DeploymentsTrigger,
    #[serde(rename = "mcp.use")]
    McpUse,
}

pub type TokenScopes = BTreeSet<TokenScope>;
impl From<AccessTokenPermission> for TokenScopes {
    fn from(value: AccessTokenPermission) -> Self {
        match value {
            AccessTokenPermission::Read => decode_scopes("read").unwrap_or_default(),
            AccessTokenPermission::Write => decode_scopes("write").unwrap_or_default(),
        }
    }
}

pub fn encode_scopes(scopes: &TokenScopes) -> Result<String, serde_json::Error> {
    serde_json::to_string(scopes)
}

pub fn decode_scopes(value: &str) -> Result<TokenScopes, serde_json::Error> {
    // Existing development keys remain readable during rolling developer upgrades.
    match value {
        "read" => Ok([
            TokenScope::SiteRead,
            TokenScope::ContentRead,
            TokenScope::FilesRead,
            TokenScope::SchemaRead,
        ]
        .into_iter()
        .collect()),
        "write" => Ok([
            TokenScope::SiteRead,
            TokenScope::SiteSettingsRead,
            TokenScope::SiteSettingsWrite,
            TokenScope::ContentRead,
            TokenScope::ContentWrite,
            TokenScope::FilesRead,
            TokenScope::FilesWrite,
            TokenScope::SchemaRead,
            TokenScope::SchemaWrite,
            TokenScope::WebhooksRead,
            TokenScope::WebhooksWrite,
            TokenScope::WebhooksTrigger,
        ]
        .into_iter()
        .collect()),
        _ => serde_json::from_str(value),
    }
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
    pub scopes: TokenScopes,
    pub expires_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AccessTokenResponse {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub token: String,
    pub token_prefix: String,
    pub permission: String,
    pub scopes: TokenScopes,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct PersonalAccessToken {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub token_prefix: String,
    pub scopes_json: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreatePersonalAccessToken {
    pub name: String,
    pub scopes: TokenScopes,
    pub expires_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PersonalAccessTokenView {
    pub id: String,
    pub name: String,
    pub token_prefix: String,
    pub scopes: TokenScopes,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PersonalAccessTokenResponse {
    #[serde(flatten)]
    pub token_info: PersonalAccessTokenView,
    pub token: String,
}
