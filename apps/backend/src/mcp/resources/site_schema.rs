use std::sync::Arc;

use rmcp::model::{ListResourcesResult, ReadResourceResult, Resource, ResourceContents, Annotations, Annotated};
use rmcp::model::RawResource;
use rmcp::ErrorData as McpError;
use chrono::Utc;

use crate::middleware::auth::{Actor, Scope};
use crate::services::{Services, scope::ScopeChecker};

fn map_err(e: impl Into<crate::services::error::ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

fn site_to_resource(site: &serde_json::Value) -> Resource {
    let id = site["id"].as_str().unwrap_or("unknown");
    let name = site["name"].as_str().unwrap_or("Unknown");
    Annotated::new(
        RawResource {
            uri: format!("cms://{}/schema", id),
            name: format!("{} Schema", name),
            title: Some(format!("Content schema for {}", name)),
            description: Some(format!("Content schema for {}", name)),
            mime_type: Some("application/json".to_string()),
            size: None,
            icons: None,
            meta: None,
        },
        Some(Annotations::for_resource(0.5, Utc::now())),
    )
}

pub async fn list_resources(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    _request: Option<rmcp::model::PaginatedRequestParams>,
) -> Result<ListResourcesResult, McpError> {
    match actor {
        Actor::ApiKey(k) => {
            scope
                .require_site_scope(actor, &k.site_id, &Scope::SiteRead, "viewer")
                .await
                .map_err(map_err)?;
            let site = services
                .site
                .get_site(&k.site_id)
                .await
                .map_err(map_err)?
                .ok_or_else(|| McpError::invalid_request("Site not found", None))?;
            let site_value = serde_json::to_value(&site).unwrap_or_default();
            Ok(ListResourcesResult::with_all_items(vec![site_to_resource(&site_value)]))
        }
        Actor::User(_) => {
            scope
                .require_site_scope(actor, "", &Scope::SiteRead, "viewer")
                .await
                .map_err(map_err)?;
            let sites = services.site.list_sites_for_actor(actor).await.map_err(map_err)?;
            let resources: Vec<Resource> = sites.into_iter().map(|s| site_to_resource(&s)).collect();
            Ok(ListResourcesResult::with_all_items(resources))
        }
    }
}

pub async fn read_resource(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let site_id = uri
        .strip_prefix("cms://")
        .and_then(|s| s.strip_suffix("/schema"))
        .ok_or_else(|| McpError::invalid_request("Invalid resource URI", None))?;

    scope
        .require_site_scope(actor, site_id, &Scope::SiteRead, "viewer")
        .await
        .map_err(map_err)?;

    let site = services
        .site
        .get_site(site_id)
        .await
        .map_err(map_err)?
        .ok_or_else(|| McpError::invalid_request("Site not found", None))?;

    let collections = services.collection.list_collections(site_id).await.map_err(map_err)?;
    let singletons = services.singleton.list_singletons(site_id).await.map_err(map_err)?;

    let schema = serde_json::json!({
        "site": {
            "id": site.id,
            "name": site.name,
        },
        "collections": collections,
        "singletons": singletons,
    });

    let schema_json = serde_json::to_string_pretty(&schema)
        .map_err(|e| McpError::internal_error(format!("Failed to serialize schema: {}", e), None))?;

    Ok(ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text: schema_json,
        meta: None,
    }]))
}
