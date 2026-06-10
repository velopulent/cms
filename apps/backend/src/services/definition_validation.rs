use serde_json::Value;

pub const VALID_FIELD_TYPES: &[&str] = &[
    "text",
    "textarea",
    "rich_text",
    "number",
    "boolean",
    "date",
    "select",
    "image_url",
    "image",
    "video",
    "audio",
    "document",
    "archive",
];

pub fn normalize_definition(value: &Value) -> Result<Value, String> {
    let fields = match value.get("fields") {
        Some(Value::Array(arr)) => arr.clone(),
        Some(_) => {
            return Err("definition 'fields' must be an array".to_string());
        }
        None => {
            let mut result = value.clone();
            result["fields"] = serde_json::json!([]);
            return Ok(result);
        }
    };

    let mut normalized_fields = Vec::new();

    for field in fields {
        let mut f = field.clone();

        let name = f
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| "field missing 'name'".to_string())?
            .to_string();
        let field_type = f
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| format!("field '{}' missing 'type'", name))?
            .to_string();

        if !VALID_FIELD_TYPES.contains(&field_type.as_str()) {
            return Err(format!(
                "field '{}' has invalid type '{}'. Valid types: {}",
                name,
                field_type,
                VALID_FIELD_TYPES.join(", ")
            ));
        }

        match f.get("accept") {
            Some(Value::String(s)) => {
                let v = s.clone();
                f["accept"] = serde_json::json!([v]);
            }
            Some(Value::Array(_)) => {}
            Some(_) => {
                return Err(format!(
                    "field '{}' 'accept' must be a string or array of MIME types",
                    name
                ));
            }
            None => {}
        }

        match f.get("options") {
            Some(Value::String(s)) => {
                let v = s.clone();
                f["options"] = serde_json::json!([v]);
            }
            Some(Value::Array(_)) => {}
            Some(_) => {
                return Err(format!(
                    "field '{}' 'options' must be a string or array of values",
                    name
                ));
            }
            None => {}
        }

        normalized_fields.push(f);
    }

    let mut result = value.clone();
    result["fields"] = serde_json::json!(normalized_fields);
    Ok(result)
}

pub fn validate_entry_data(data: &Value, fields: &[Value]) -> Option<String> {
    let obj = match data.as_object() {
        Some(o) => o,
        None => return Some("Entry data must be a JSON object".to_string()),
    };

    for field_def in fields {
        let name = match field_def.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };
        let field_type = field_def.get("type").and_then(|t| t.as_str()).unwrap_or("text");
        let required = field_def.get("required").and_then(|r| r.as_bool()).unwrap_or(false);

        let value = obj.get(name);

        if required {
            match value {
                None => {
                    return Some(format!("Required field '{}' is missing", name));
                }
                Some(v) if v.is_null() => {
                    return Some(format!("Required field '{}' cannot be null", name));
                }
                Some(v) if v.is_string() && v.as_str().unwrap_or("").is_empty() => {
                    return Some(format!("Required field '{}' cannot be empty", name));
                }
                _ => {}
            }
        }

        if let Some(v) = value
            && !v.is_null()
        {
            match field_type {
                "number" => {
                    if !v.is_number() {
                        return Some(format!("Field '{}' must be a number", name));
                    }
                }
                "boolean" => {
                    if !v.is_boolean() {
                        return Some(format!("Field '{}' must be a boolean", name));
                    }
                }
                "select" => {
                    if let Some(options) = field_def.get("options").and_then(|o| o.as_array()) {
                        let valid: Vec<&str> = options.iter().filter_map(|o| o.as_str()).collect();
                        if let Some(s) = v.as_str()
                            && !valid.is_empty()
                            && !valid.contains(&s)
                        {
                            return Some(format!(
                                "Field '{}' value '{}' is not in allowed options: {:?}",
                                name, s, valid
                            ));
                        }
                    }
                }
                "image" | "video" | "audio" | "document" | "archive" => {
                    if let Some(s) = v.as_str()
                        && !s.is_empty()
                        && !s.starts_with("/api/files/")
                        && !s.starts_with("http")
                    {
                        return Some(format!("Field '{}' must be a valid file URL, got '{}'", name, s));
                    }
                }
                _ => {}
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_definition_converts_accept_string_to_array() {
        let input = json!({
            "fields": [
                {"name": "cover", "type": "image", "accept": "image/jpeg"}
            ]
        });
        let result = normalize_definition(&input).unwrap();
        let fields = result["fields"].as_array().unwrap();
        assert!(fields[0]["accept"].is_array());
        assert_eq!(fields[0]["accept"][0], "image/jpeg");
    }

    #[test]
    fn normalize_definition_keeps_accept_array() {
        let input = json!({
            "fields": [
                {"name": "cover", "type": "image", "accept": ["image/jpeg", "image/png"]}
            ]
        });
        let result = normalize_definition(&input).unwrap();
        let fields = result["fields"].as_array().unwrap();
        assert_eq!(fields[0]["accept"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn normalize_definition_converts_options_string_to_array() {
        let input = json!({
            "fields": [
                {"name": "status", "type": "select", "options": "draft"}
            ]
        });
        let result = normalize_definition(&input).unwrap();
        let fields = result["fields"].as_array().unwrap();
        assert!(fields[0]["options"].is_array());
        assert_eq!(fields[0]["options"][0], "draft");
    }

    #[test]
    fn normalize_definition_rejects_invalid_type() {
        let input = json!({
            "fields": [
                {"name": "cover", "type": "media"}
            ]
        });
        assert!(normalize_definition(&input).is_err());
    }

    #[test]
    fn normalize_definition_handles_missing_fields_array() {
        let input = json!({"not_fields": []});
        let result = normalize_definition(&input);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val["fields"].as_array().unwrap().is_empty());
    }

    #[test]
    fn validate_entry_data_checks_required() {
        let fields = json!([
            {"name": "title", "type": "text", "required": true}
        ]);
        let data = json!({});
        let err = validate_entry_data(&data, fields.as_array().unwrap());
        assert_eq!(err.unwrap(), "Required field 'title' is missing");
    }

    #[test]
    fn validate_entry_data_checks_number_type() {
        let fields = json!([{"name": "count", "type": "number"}]);
        let data = json!({"count": "not a number"});
        let err = validate_entry_data(&data, fields.as_array().unwrap());
        assert_eq!(err.unwrap(), "Field 'count' must be a number");
    }

    #[test]
    fn validate_entry_data_checks_select_options() {
        let fields = json!([
            {"name": "status", "type": "select", "options": ["draft", "published"]}
        ]);
        let data = json!({"status": "archived"});
        let err = validate_entry_data(&data, fields.as_array().unwrap());
        assert!(err.unwrap().contains("not in allowed options"));
    }

    #[test]
    fn validate_entry_data_passes_valid_data() {
        let fields = json!([
            {"name": "title", "type": "text", "required": true},
            {"name": "count", "type": "number"},
            {"name": "status", "type": "select", "options": ["draft", "published"]}
        ]);
        let data = json!({"title": "Hello", "count": 42, "status": "draft"});
        assert!(validate_entry_data(&data, fields.as_array().unwrap()).is_none());
    }
}
