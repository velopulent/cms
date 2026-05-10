use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::AnnotateAble;
use rmcp::model::{ListResourcesResult, PaginatedRequestParams, RawResource, ReadResourceResult, ResourceContents};

use crate::middleware::auth::{Principal, SCOPE_SCHEMA_READ, SCOPE_SITES_READ};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};

pub async fn list_resources(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    _request: Option<PaginatedRequestParams>,
) -> Result<ListResourcesResult, McpError> {
    match principal {
        Principal::SiteToken { .. } => scope.check_scope(principal, SCOPE_SCHEMA_READ),
        Principal::InstanceToken { .. } => scope.check_scope(principal, SCOPE_SITES_READ),
        Principal::UserSession { .. } => Ok(()),
    }
    .map_err(crate::mcp::auth::service_error_to_mcp)?;

    let sites = match principal {
        Principal::SiteToken { site_id, .. } => match services.site.get_site(site_id).await {
            Ok(Some(site)) => vec![serde_json::to_value(site).unwrap_or_default()],
            Ok(None) => Vec::new(),
            Err(e) => return Err(crate::mcp::auth::service_error_to_mcp(ServiceError::Site(e))),
        },
        _ => match services.site.list_sites_for_principal(principal).await {
            Ok(sites) => sites,
            Err(e) => return Err(crate::mcp::auth::service_error_to_mcp(ServiceError::Site(e))),
        },
    };

    let mut resources = Vec::new();

    for site in sites {
        let site_id = site.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let site_name = site.get("name").and_then(|v| v.as_str()).unwrap_or("Site");

        resources.push(RawResource::new(format!("cms://sites/{}", site_id), site_name).no_annotation());

        resources.push(
            RawResource::new(
                format!("cms://sites/{}/collections", site_id),
                format!("Collections for {}", site_name),
            )
            .no_annotation(),
        );
    }

    Ok(ListResourcesResult::with_all_items(resources))
}

pub async fn read_resource(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let parts: Vec<&str> = uri.strip_prefix("cms://").unwrap_or("").split('/').collect();

    if parts.len() < 2 {
        return Err(McpError::invalid_params("Invalid resource URI", None));
    }

    let site_id = parts[1];

    if parts.len() == 2 {
        match services.site.get_site(site_id).await {
            Ok(Some(site)) => {
                if let Err(e) = scope
                    .require_site_scope(principal, Some(site_id), SCOPE_SCHEMA_READ, "viewer")
                    .await
                {
                    return Err(crate::mcp::auth::service_error_to_mcp(e));
                }
                let text = serde_json::to_string_pretty(&site).unwrap_or_default();
                return Ok(ReadResourceResult::new(vec![ResourceContents::text(text, uri)]));
            }
            Ok(None) => return Err(McpError::invalid_params("Site not found", None)),
            Err(e) => return Err(crate::mcp::auth::service_error_to_mcp(ServiceError::Site(e))),
        }
    }

    if parts.len() >= 3 && parts[2] == "collections" {
        if let Err(e) = scope
            .require_site_scope(principal, Some(site_id), SCOPE_SCHEMA_READ, "viewer")
            .await
        {
            return Err(crate::mcp::auth::service_error_to_mcp(e));
        }
        match services.collection.list_collections(site_id).await {
            Ok(collections) => {
                let text = serde_json::to_string_pretty(&collections).unwrap_or_default();
                return Ok(ReadResourceResult::new(vec![ResourceContents::text(text, uri)]));
            }
            Err(e) => return Err(crate::mcp::auth::service_error_to_mcp(ServiceError::Collection(e))),
        }
    }

    Err(McpError::invalid_params("Unknown resource URI", None))
}
