use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::config::Config;
use crate::mcp::auth::{map_err, ok_result, text_result};
use crate::middleware::auth::{Actor, Scope};
use crate::services::{Services, scope::ScopeChecker};
use crate::signed_upload::SignedUploadToken;

fn require_site_id(actor: &Actor) -> Result<String, McpError> {
    actor
        .bound_site_id()
        .map(String::from)
        .ok_or_else(|| McpError::invalid_request("No site context", None))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFilesParams {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub file_type: Option<String>,
    #[serde(default)]
    pub trashed: Option<bool>,
}

pub async fn list_files(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListFilesParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::FilesRead, "viewer")
        .await
        .map_err(map_err)?;

    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(50).clamp(1, 200);

    use crate::repository::traits::ListFilesParams as RepoListFilesParams;
    let list_params = RepoListFilesParams {
        site_id: &site_id,
        trashed: params.0.trashed.unwrap_or(false),
        search: params.0.search.as_deref(),
        file_type: params.0.file_type.as_deref(),
        page,
        per_page,
    };

    let result = services.file.list_files(list_params).await.map_err(map_err)?;
    ok_result(&serde_json::json!({
        "items": result.items,
        "total": result.total,
        "page": result.page,
        "per_page": result.per_page,
    }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileParams {
    pub file_id: String,
}

pub async fn get_file(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetFileParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::FilesRead, "viewer")
        .await
        .map_err(map_err)?;

    match services.file.get_file(&params.0.file_id, &site_id).await.map_err(map_err)? {
        Some(file) => ok_result(&file),
        None => Ok(text_result("File not found")),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateUploadUrlParams {
    pub filename: String,
    #[serde(default = "default_content_type")]
    pub content_type: String,
}

fn default_content_type() -> String {
    "application/octet-stream".to_string()
}

pub async fn create_upload_url(
    scope: &Arc<ScopeChecker>,
    config: &Arc<Config>,
    actor: &Actor,
    public_base_url: Option<String>,
    params: Parameters<CreateUploadUrlParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::FilesWrite, "editor")
        .await
        .map_err(map_err)?;

    let (token, upload_path) = SignedUploadToken::generate(&site_id, &params.0.filename, &params.0.content_type, &config.hmac_secret);

    let fallback_base_url = format!("http://{}", config.bind_address);
    let base_url = public_base_url
        .as_deref()
        .or(config.public_url.as_deref())
        .unwrap_or(fallback_base_url.as_str())
        .trim_end_matches('/');
    let base_url = format!("{}/api/v1/files/upload", base_url);
    let upload_url = format!("{}/{}", base_url, upload_path);

    ok_result(&serde_json::json!({
        "upload_url": upload_url,
        "file_id": token.file_id,
        "expires_at": token.expires_at(),
        "method": "PUT",
        "content_type": params.0.content_type,
    }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteFileParams {
    pub file_id: String,
}

pub async fn delete_file(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteFileParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::FilesWrite, "editor")
        .await
        .map_err(map_err)?;

    services.file.soft_delete(&params.0.file_id, &site_id).await.map_err(map_err)?;
    Ok(text_result("File deleted"))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreFileParams {
    pub file_id: String,
}

pub async fn restore_file(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<RestoreFileParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::FilesWrite, "editor")
        .await
        .map_err(map_err)?;

    let n = services.file.restore(&params.0.file_id, &site_id).await.map_err(map_err)?;
    if n > 0 {
        Ok(text_result("File restored"))
    } else {
        Ok(text_result("File not found or not deleted"))
    }
}
