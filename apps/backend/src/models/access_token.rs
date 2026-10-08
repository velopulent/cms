use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
pub enum TokenScope {
    #[serde(rename = "site.read")]
    SiteRead,
    #[serde(rename = "content.read")]
    ContentRead,
    #[serde(rename = "content.preview.read")]
    ContentPreviewRead,
    #[serde(rename = "content.write")]
    ContentWrite,
    #[serde(rename = "content.publish")]
    ContentPublish,
    #[serde(rename = "files.read")]
    FilesRead,
    #[serde(rename = "files.write")]
    FilesWrite,
    #[serde(rename = "schema.read")]
    SchemaRead,
    #[serde(rename = "mcp.use")]
    McpUse,
}

impl TokenScope {
    /// Wire name, identical to the serde representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            TokenScope::SiteRead => "site.read",
            TokenScope::ContentRead => "content.read",
            TokenScope::ContentPreviewRead => "content.preview.read",
            TokenScope::ContentWrite => "content.write",
            TokenScope::ContentPublish => "content.publish",
            TokenScope::FilesRead => "files.read",
            TokenScope::FilesWrite => "files.write",
            TokenScope::SchemaRead => "schema.read",
            TokenScope::McpUse => "mcp.use",
        }
    }
}

pub type TokenScopes = BTreeSet<TokenScope>;
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

#[cfg(test)]
mod scope_name_tests {
    use super::TokenScope;

    #[test]
    fn scope_names_match_their_wire_format() {
        for scope in [
            TokenScope::SiteRead,
            TokenScope::ContentRead,
            TokenScope::ContentPreviewRead,
            TokenScope::ContentWrite,
            TokenScope::ContentPublish,
            TokenScope::FilesRead,
            TokenScope::FilesWrite,
            TokenScope::SchemaRead,
            TokenScope::McpUse,
        ] {
            assert_eq!(serde_json::to_value(scope).unwrap(), scope.as_str());
        }
    }
}
