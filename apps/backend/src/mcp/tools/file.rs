use std::sync::Arc;

use rmcp::model::CallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::config::Config;
use crate::middleware::auth::{Principal, SCOPE_ASSETS_READ, SCOPE_ASSETS_WRITE};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};
use crate::signed_upload::SignedUploadToken;

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).unwrap_or_default();
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFilesParams {
    pub site_id: String,
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
    principal: &Principal,
    params: Parameters<ListFilesParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_ASSETS_READ, "viewer")
        .await.map_err(map_err)?;

    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(50).clamp(1, 200);

    use crate::repository::traits::ListFilesParams as RepoListFilesParams;
    let list_params = RepoListFilesParams {
        site_id: &site.site_id,
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
    pub site_id: String,
    pub file_id: String,
}

pub async fn get_file(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<GetFileParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_ASSETS_READ, "viewer")
        .await.map_err(map_err)?;

    match services.file.get_file(&params.0.file_id, &site.site_id).await.map_err(map_err)? {
        Some(file) => ok_result(&file),
        None => Ok(CallToolResult::success(vec![Content::text("File not found")])),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateUploadUrlParams {
    pub site_id: String,
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
    principal: &Principal,
    params: Parameters<CreateUploadUrlParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_ASSETS_WRITE, "editor")
        .await.map_err(map_err)?;

    let (token, upload_path) = SignedUploadToken::generate(
        &site.site_id,
        &params.0.filename,
        &params.0.content_type,
        &config.hmac_secret,
    );

    let base_url = format!("{}/api/v1/files/upload", config.bind_address);
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
    pub site_id: String,
    pub file_id: String,
}

pub async fn delete_file(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteFileParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_ASSETS_WRITE, "editor")
        .await.map_err(map_err)?;

    services.file.soft_delete(&params.0.file_id, &site.site_id).await.map_err(map_err)?;
    Ok(CallToolResult::success(vec![Content::text("File deleted")]))
}