use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

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
pub fn scopes_can_write(scopes: &TokenScopes) -> bool {
    scopes.iter().any(|scope| {
        matches!(
            scope,
            TokenScope::SiteSettingsWrite
                | TokenScope::ContentWrite
                | TokenScope::FilesWrite
                | TokenScope::SchemaWrite
                | TokenScope::WebhooksWrite
                | TokenScope::WebhooksTrigger
                | TokenScope::DeploymentsWrite
                | TokenScope::DeploymentsTrigger
        )
    })
}

pub fn encode_scopes(scopes: &TokenScopes) -> Result<String, serde_json::Error> {
    serde_json::to_string(scopes)
}

pub fn decode_scopes(value: &str) -> Result<TokenScopes, serde_json::Error> {
    serde_json::from_str(value)
}

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct AccessToken {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub token_prefix: String,
    pub scopes_json: String,
    pub created_by_user_id: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Serialize, ToSchema, Clone)]
pub struct AccessTokenView {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub token_prefix: String,
    pub scopes: TokenScopes,
    pub created_by_user_id: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
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
    pub scopes: TokenScopes,
    pub created_at: String,
    pub expires_at: Option<String>,
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
