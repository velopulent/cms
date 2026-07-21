use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::config::Config;
use crate::mcp::auth::{ok_result, text_result, tool_error};
use crate::middleware::auth::Actor;
use crate::models::authorization::Action;
use crate::services::{Services, authorization::AuthorizationService};
use crate::signed_upload::SignedUploadToken;

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
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListFilesParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::FilesRead)
        .await
    {
        return Ok(tool_error(e));
    }

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

    match services.file.list_files(list_params).await {
        Ok(result) => ok_result(&serde_json::json!({
            "items": result.items,
            "total": result.total,
            "page": result.page,
            "per_page": result.per_page,
        })),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileParams {
    pub site_id: String,
    pub file_id: String,
}

pub async fn get_file(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetFileParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::FilesRead)
        .await
    {
        return Ok(tool_error(e));
    }

    match services.file.get_file(&params.0.file_id, &site_id).await {
        Ok(Some(file)) => ok_result(&file),
        Ok(None) => Ok(tool_error(crate::services::error::ServiceError::NotFound(
            "File not found".into(),
        ))),
        Err(e) => Ok(tool_error(e)),
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
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    config: &Arc<Config>,
    actor: &Actor,
    public_base_url: Option<String>,
    params: Parameters<CreateUploadUrlParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::FilesWrite)
        .await
    {
        return Ok(tool_error(e));
    }

    // Fail fast at mint time instead of returning a URL doomed to a 400 PUT.
    if !crate::utils::content_types::is_allowed(&params.0.content_type) {
        return Ok(tool_error(crate::services::error::ServiceError::BadRequest(format!(
            "Content type '{}' is not allowed. Accepted types: images, videos, audio, documents, archives",
            params.0.content_type
        ))));
    }

    // Mint against the site's actual storage provider, not a hardcoded default.
    let storage_provider = services
        .file
        .get_storage_provider(&site_id)
        .await
        .unwrap_or_else(|_| "filesystem".into());

    let (token, upload_path) = SignedUploadToken::generate_with_storage_provider(
        &site_id,
        &params.0.filename,
        &params.0.content_type,
        &storage_provider,
        &config.signed_upload_key,
        config.upload_token_expiry_secs,
    );

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
    pub site_id: String,
    pub file_id: String,
}

pub async fn delete_file(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteFileParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::FilesWrite)
        .await
    {
        return Ok(tool_error(e));
    }

    match services.file.soft_delete(&params.0.file_id, &site_id).await {
        Ok(_) => Ok(text_result("File deleted")),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreFileParams {
    pub site_id: String,
    pub file_id: String,
}

pub async fn restore_file(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<RestoreFileParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::FilesWrite)
        .await
    {
        return Ok(tool_error(e));
    }

    match services.file.restore(&params.0.file_id, &site_id).await {
        Ok(n) => {
            if n > 0 {
                Ok(text_result("File restored"))
            } else {
                Ok(tool_error(crate::services::error::ServiceError::NotFound(
                    "File not found or not deleted".into(),
                )))
            }
        }
        Err(e) => Ok(tool_error(e)),
    }
}
