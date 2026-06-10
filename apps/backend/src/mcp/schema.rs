use std::borrow::Cow;
use std::sync::Arc;

use schemars::json_schema;
use schemars::{JsonSchema, Schema, generate::SchemaGenerator};

use rmcp::model::JsonObject;

/// Stand-in type used with `#[schemars(with = "ArbitraryJson")]` to generate
/// a permissive object schema (`{"type": "object", "additionalProperties": true}`)
/// instead of the boolean `true` that `schemars` emits for `serde_json::Value`.
///
/// This type is never constructed — it exists only for its `JsonSchema` impl.
pub struct ArbitraryJson;

impl JsonSchema for ArbitraryJson {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ArbitraryJson")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "additionalProperties": true
        })
    }
}

/// Post-process an MCP tool `input_schema` to improve compatibility with MCP clients.
///
/// Removes or simplifies features that MCP Inspector and Postman have trouble with:
/// - Root-level `$schema` and `title`
/// - Per-property `$schema` and `title`
/// - Boolean schema values (`true`) → replaced with permissive object
/// - `"type": ["string", "null"]` → simplified to `"type": "string"`
pub fn clean_input_schema(schema: Arc<JsonObject>) -> Arc<JsonObject> {
    let mut map = (*schema).clone();

    map.remove("$schema");
    map.remove("title");

    if let Some(properties) = map.get_mut("properties").and_then(|v| v.as_object_mut()) {
        for value in properties.values_mut() {
            match value {
                serde_json::Value::Object(obj) => {
                    obj.remove("$schema");
                    obj.remove("title");
                    simplify_nullable_type(obj);
                }
                serde_json::Value::Bool(_) => {
                    *value = serde_json::json!({
                        "type": "object",
                        "additionalProperties": true
                    });
                }
                _ => {}
            }
        }
    }

    Arc::new(map)
}

fn simplify_nullable_type(obj: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(type_val) = obj.get("type")
        && let Some(types) = type_val.as_array()
    {
        let non_null: Vec<&serde_json::Value> = types.iter().filter(|v| v.as_str() != Some("null")).collect();
        if non_null.len() == 1 {
            obj.insert("type".to_string(), non_null[0].clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_removes_schema_and_title() {
        let raw = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Test",
            "type": "object",
            "properties": {}
        });
        let schema = serde_json::from_value::<JsonObject>(raw).unwrap();
        let cleaned = clean_input_schema(Arc::new(schema));
        assert!(!cleaned.contains_key("$schema"));
        assert!(!cleaned.contains_key("title"));
    }

    #[test]
    fn test_clean_simplifies_nullable_type() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": ["string", "null"]
                }
            },
            "required": []
        });
        let schema = serde_json::from_value::<JsonObject>(raw).unwrap();
        let cleaned = clean_input_schema(Arc::new(schema));
        let name = &cleaned["properties"]["name"];
        assert_eq!(name["type"], "string");
    }

    #[test]
    fn test_clean_replaces_boolean_schema() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": {
                "data": true
            },
            "required": ["data"]
        });
        let schema = serde_json::from_value::<JsonObject>(raw).unwrap();
        let cleaned = clean_input_schema(Arc::new(schema));
        let data = &cleaned["properties"]["data"];
        assert!(data.is_object());
        assert_eq!(data["type"], "object");
        assert_eq!(data["additionalProperties"], true);
    }

    #[test]
    fn test_clean_leaves_valid_schema_unchanged() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["name"]
        });
        let schema = serde_json::from_value::<JsonObject>(raw).unwrap();
        let cleaned = clean_input_schema(Arc::new(schema));
        assert_eq!(cleaned["type"], "object");
        assert_eq!(cleaned["properties"]["name"]["type"], "string");
        assert_eq!(cleaned["properties"]["count"]["type"], "integer");
        assert!(cleaned["required"].as_array().unwrap().contains(&"name".into()));
    }
}
