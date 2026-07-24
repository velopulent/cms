use std::sync::Arc;

use chrono::Utc;
use rmcp::ErrorData as McpError;
use rmcp::model::{Annotations, ListResourcesResult, ReadResourceResult, Resource, ResourceContents};

use crate::middleware::auth::Actor;
use crate::models::authorization::Action;
use crate::models::collection::Collection;
use crate::services::{Services, authorization::AuthorizationService};

fn map_err(e: impl Into<crate::services::error::ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

fn resource_uri(site_id: &str, path: &str) -> String {
    format!("cms://{}{}", site_id, path)
}

fn make_resource(uri: &str, name: &str, title: &str, description: &str) -> Resource {
    Resource::new(uri, name)
        .with_title(title)
        .with_description(description)
        .with_mime_type("application/json")
        .with_annotations(Annotations::for_resource(0.5, Utc::now()))
}

fn collection_to_schema_value(c: &Collection) -> serde_json::Value {
    let definition: serde_json::Value =
        serde_json::from_str(&c.definition).unwrap_or(serde_json::json!({"fields": []}));
    serde_json::json!({
        "id": c.id,
        "site_id": c.site_id,
        "name": c.name,
        "slug": c.slug,
        "definition": definition,
        "is_singleton": c.is_singleton,
        "created_at": c.created_at,
        "updated_at": c.updated_at,
    })
}

pub async fn list_resources(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    _request: Option<rmcp::model::PaginatedRequestParams>,
) -> Result<ListResourcesResult, McpError> {
    let sites = services.site.list_sites_for_actor(actor).await.map_err(map_err)?;
    let mut resources = Vec::new();
    for site in sites {
        let Some(site_id) = site.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        authorization
            .require_site_action(actor, site_id, Action::SiteRead)
            .await
            .map_err(map_err)?;
        let site_name = site.get("name").and_then(|value| value.as_str()).unwrap_or(site_id);
        resources.push(make_resource(
            &resource_uri(site_id, "/schema"),
            &format!("{} Schema", site_name),
            &format!("Content schema for {}", site_name),
            &format!("Full content schema for {}", site_name),
        ));
        for collection in services.collection.list_collections(site_id).await.map_err(map_err)? {
            resources.push(make_resource(
                &resource_uri(site_id, &format!("/collections/{}", collection.slug)),
                &format!("{}/{}", site_name, collection.name),
                &format!("Collection: {}", collection.name),
                &format!("Schema for {} collection", collection.name),
            ));
        }
        for singleton in services.singleton.list_singletons(site_id).await.map_err(map_err)? {
            resources.push(make_resource(
                &resource_uri(site_id, &format!("/singletons/{}", singleton.slug)),
                &format!("{}/{}", site_name, singleton.name),
                &format!("Singleton: {}", singleton.name),
                &format!("Schema for {} singleton", singleton.name),
            ));
        }
    }

    Ok(ListResourcesResult::with_all_items(resources))
}

pub async fn read_resource(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let remainder = uri
        .strip_prefix("cms://")
        .ok_or_else(|| McpError::invalid_request("Invalid resource URI", None))?;

    let (site_id, path) = remainder
        .split_once('/')
        .ok_or_else(|| McpError::invalid_request("Invalid resource URI", None))?;

    authorization
        .require_site_action(actor, site_id, Action::SiteRead)
        .await
        .map_err(map_err)?;

    match path {
        "schema" => read_schema_resource(services, site_id, uri).await,
        p if p.starts_with("collections/") => {
            let slug = &p["collections/".len()..];
            read_collection_resource(services, site_id, slug, uri).await
        }
        p if p.starts_with("singletons/") => {
            let slug = &p["singletons/".len()..];
            read_singleton_resource(services, site_id, slug, uri).await
        }
        _ => Err(McpError::invalid_request("Unknown resource path", None)),
    }
}

async fn read_schema_resource(
    services: &Arc<Services>,
    site_id: &str,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let site = services
        .site
        .get_site(site_id)
        .await
        .map_err(map_err)?
        .ok_or_else(|| McpError::invalid_request("Site not found", None))?;

    let collections = services.collection.list_collections(site_id).await.map_err(map_err)?;
    let singletons = services.singleton.list_singletons(site_id).await.map_err(map_err)?;

    let collections_json: Vec<serde_json::Value> = collections.iter().map(collection_to_schema_value).collect();

    let schema = serde_json::json!({
        "site": {
            "id": site.id,
            "name": site.name,
        },
        "collections": collections_json,
        "singletons": singletons,
        "field_types": [
            {"type": "text", "label": "Text"},
            {"type": "textarea", "label": "Text Area"},
            {"type": "rich_text", "label": "Rich Text (Tiptap)"},
            {"type": "number", "label": "Number"},
            {"type": "boolean", "label": "Boolean"},
            {"type": "date", "label": "Date"},
            {"type": "select", "label": "Select", "properties": ["options"]},
            {"type": "email", "label": "Email"},
            {"type": "url", "label": "URL", "properties": ["except_domains", "only_domains"]},
            {"type": "json", "label": "JSON", "properties": ["max_size"]},
            {"type": "relation", "label": "Relation", "properties": ["target_collection", "multiple", "min_select", "max_select"]},
            {"type": "file", "label": "File", "properties": ["accept", "max_size", "multiple"]}
        ],
        "content_types": {
            "categories": ["image", "video", "audio", "document", "archive"],
            "mime_types": {
                "image": crate::utils::content_types::IMAGE_TYPES,
                "video": crate::utils::content_types::VIDEO_TYPES,
                "audio": crate::utils::content_types::AUDIO_TYPES,
                "document": crate::utils::content_types::DOCUMENT_TYPES,
                "archive": crate::utils::content_types::ARCHIVE_TYPES
            }
        }
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

async fn read_collection_resource(
    services: &Arc<Services>,
    site_id: &str,
    slug: &str,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let collection = services
        .collection
        .get_collection(site_id, slug)
        .await
        .map_err(map_err)?
        .ok_or_else(|| McpError::invalid_request("Collection not found", None))?;

    let value = collection_to_schema_value(&collection);
    let json = serde_json::to_string_pretty(&value)
        .map_err(|e| McpError::internal_error(format!("Failed to serialize: {}", e), None))?;

    Ok(ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text: json,
        meta: None,
    }]))
}

async fn read_singleton_resource(
    services: &Arc<Services>,
    site_id: &str,
    slug: &str,
    uri: &str,
) -> Result<ReadResourceResult, McpError> {
    let singletons = services.singleton.list_singletons(site_id).await.map_err(map_err)?;

    let singleton = singletons
        .iter()
        .find(|s| s.slug == slug)
        .ok_or_else(|| McpError::invalid_request("Singleton not found", None))?;

    let json = serde_json::to_string_pretty(singleton)
        .map_err(|e| McpError::internal_error(format!("Failed to serialize: {}", e), None))?;

    Ok(ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
        uri: uri.to_string(),
        mime_type: Some("application/json".to_string()),
        text: json,
        meta: None,
    }]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::Collection;

    fn make_collection(definition: &str) -> Collection {
        Collection {
            id: "col-1".into(),
            site_id: "site-1".into(),
            name: "Blog".into(),
            slug: "blog".into(),
            definition: definition.into(),
            is_singleton: false,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-02T00:00:00Z".into(),
        }
    }

    #[test]
    fn definition_is_object_not_string() {
        let def_json = r#"{"fields":[{"name":"title","type":"text","required":true}]}"#;
        let c = make_collection(def_json);
        let value = collection_to_schema_value(&c);

        let def = value.get("definition").expect("definition missing");
        assert!(def.is_object(), "definition should be a JSON object, got: {}", def);
        let fields = def.get("fields").expect("fields missing");
        assert!(fields.is_array(), "fields should be an array");
        assert_eq!(fields.as_array().unwrap().len(), 1);
    }

    #[test]
    fn invalid_json_definition_falls_back_to_empty_fields() {
        let c = make_collection("not-valid-json{{");
        let value = collection_to_schema_value(&c);

        let def = value.get("definition").expect("definition missing");
        assert!(def.is_object());
        let fields = def.get("fields").expect("fields missing");
        assert!(fields.is_array());
        assert!(fields.as_array().unwrap().is_empty());
    }

    #[test]
    fn schema_output_is_valid_json() {
        let def_json = r#"{"fields":[{"name":"title","type":"text","required":true}]}"#;
        let c = make_collection(def_json);
        let collections_json = vec![collection_to_schema_value(&c)];

        let schema = serde_json::json!({
            "site": {"id": "site-1", "name": "Test Site"},
            "collections": collections_json,
            "singletons": [],
        });

        let pretty = serde_json::to_string_pretty(&schema).expect("serialization failed");
        let parsed: serde_json::Value = serde_json::from_str(&pretty).expect("output is not valid JSON");

        assert_eq!(parsed["site"]["id"], "site-1");
        assert_eq!(parsed["collections"][0]["name"], "Blog");
    }

    #[test]
    fn no_escaped_json_in_schema_output() {
        let def_json = r#"{"fields":[{"name":"title","type":"text","required":true}]}"#;
        let c = make_collection(def_json);
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
        assert!(!pretty.contains(r#""{\"fields\""#), "no escaped JSON fields allowed");
        assert!(!pretty.contains(r#"\"fields\""#), "no escaped field names allowed");
    }

    #[test]
    fn resource_uri_format() {
        assert_eq!(resource_uri("site-1", "/schema"), "cms://site-1/schema");
        assert_eq!(
            resource_uri("site-1", "/collections/blog"),
            "cms://site-1/collections/blog"
        );
        assert_eq!(
            resource_uri("site-1", "/singletons/settings"),
            "cms://site-1/singletons/settings"
        );
    }
}
