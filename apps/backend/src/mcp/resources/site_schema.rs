use std::sync::Arc;

use rmcp::model::{ListResourcesResult, ReadResourceResult, Resource, ResourceContents, Annotations, Annotated};
use rmcp::model::RawResource;
use rmcp::ErrorData as McpError;
use chrono::Utc;

use crate::middleware::auth::{Actor, Scope};
use crate::models::collection::Collection;
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

fn collection_to_schema_value(c: &Collection) -> serde_json::Value {
    let definition: serde_json::Value =
        serde_json::from_str(&c.definition).unwrap_or(serde_json::json!({"fields": []}));
    let singleton_data: Option<serde_json::Value> = c
        .singleton_data
        .as_ref()
        .and_then(|d| serde_json::from_str(d).ok());
    serde_json::json!({
        "id": c.id,
        "site_id": c.site_id,
        "name": c.name,
        "slug": c.slug,
        "definition": definition,
        "is_singleton": c.is_singleton,
        "singleton_data": singleton_data,
        "created_at": c.created_at,
        "updated_at": c.updated_at,
    })
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

    let collections_json: Vec<serde_json::Value> =
        collections.iter().map(collection_to_schema_value).collect();

    let schema = serde_json::json!({
        "site": {
            "id": site.id,
            "name": site.name,
        },
        "collections": collections_json,
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

#[cfg(test)]
mod tests {
    use super::*;
use crate::models::collection::Collection;

    fn make_collection(definition: &str, singleton_data: Option<&str>) -> Collection {
        Collection {
            id: "col-1".into(),
            site_id: "site-1".into(),
            name: "Blog".into(),
            slug: "blog".into(),
            definition: definition.into(),
            is_singleton: false,
            singleton_data: singleton_data.map(|s| s.into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-02T00:00:00Z".into(),
        }
    }

    #[test]
    fn definition_is_object_not_string() {
        let def_json = r#"{"fields":[{"name":"title","type":"text","required":true}]}"#;
        let c = make_collection(def_json, None);
        let value = collection_to_schema_value(&c);

        let def = value.get("definition").expect("definition missing");
        assert!(
            def.is_object(),
            "definition should be a JSON object, got: {}",
            def
        );
        let fields = def.get("fields").expect("fields missing");
        assert!(fields.is_array(), "fields should be an array");
        assert_eq!(fields.as_array().unwrap().len(), 1);
    }

    #[test]
    fn singleton_data_is_structured() {
        let data_json = r#"{"title":"Hello","count":42}"#;
        let c = make_collection(r#"{"fields":[]}"#, Some(data_json));
        let value = collection_to_schema_value(&c);

        let data = value.get("singleton_data").expect("singleton_data missing");
        assert!(
            data.is_object(),
            "singleton_data should be a JSON object, got: {}",
            data
        );
        assert_eq!(data["title"], "Hello");
        assert_eq!(data["count"], 42);
    }

    #[test]
    fn invalid_json_definition_falls_back_to_empty_fields() {
        let c = make_collection("not-valid-json{{", None);
        let value = collection_to_schema_value(&c);

        let def = value.get("definition").expect("definition missing");
        assert!(def.is_object());
        let fields = def.get("fields").expect("fields missing");
        assert!(fields.is_array());
        assert!(fields.as_array().unwrap().is_empty());
    }

    #[test]
    fn invalid_json_singleton_data_becomes_null() {
        let c = make_collection(r#"{"fields":[]}"#, Some("not-json"));
        let value = collection_to_schema_value(&c);

        let data = value.get("singleton_data").expect("singleton_data key missing");
        assert!(data.is_null(), "singleton_data should be null for invalid JSON");
    }

    #[test]
    fn schema_output_is_valid_json() {
        let def_json = r#"{"fields":[{"name":"title","type":"text","required":true}]}"#;
        let c = make_collection(def_json, None);
        let collections_json = vec![collection_to_schema_value(&c)];

        let schema = serde_json::json!({
            "site": {"id": "site-1", "name": "Test Site"},
            "collections": collections_json,
            "singletons": [],
        });

        let pretty = serde_json::to_string_pretty(&schema).expect("serialization failed");
        let parsed: serde_json::Value =
            serde_json::from_str(&pretty).expect("output is not valid JSON");

        assert_eq!(parsed["site"]["id"], "site-1");
        assert_eq!(parsed["collections"][0]["name"], "Blog");
    }

    #[test]
    fn no_escaped_json_in_schema_output() {
        let def_json = r#"{"fields":[{"name":"title","type":"text","required":true}]}"#;
        let c = make_collection(def_json, None);
        let collections_json = vec![collection_to_schema_value(&c)];

        let schema = serde_json::json!({
            "site": {"id": "site-1", "name": "Test Site"},
            "collections": collections_json,
            "singletons": [],
        });

        let pretty = serde_json::to_string_pretty(&schema).expect("serialization failed");

        assert!(
            !pretty.contains(r#""definition":"{"#),
            "definition should not be a stringified JSON blob"
        );
        assert!(
            !pretty.contains(r#""{\"fields\""#),
            "no escaped JSON fields allowed"
        );
        assert!(
            !pretty.contains(r#"\"fields\""#),
            "no escaped field names allowed"
        );
    }
}
