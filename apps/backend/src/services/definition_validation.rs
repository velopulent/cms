use serde_json::Value;

pub const VALID_FIELD_TYPES: &[&str] = &[
    "text",
    "textarea",
    "rich_text",
    "number",
    "boolean",
    "date",
    "select",
    "email",
    "url",
    "json",
    "relation",
    "image_url",
    "image",
    "video",
    "audio",
    "document",
    "archive",
];

/// Field types whose value is a reference to another collection's entries.
pub const RELATION_FIELD_TYPE: &str = "relation";

/// Field types that store a file URL (or array of URLs when `multiple`).
pub const FILE_FIELD_TYPES: &[&str] = &["image", "video", "audio", "document", "archive"];

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

        match f.get("thumb_sizes") {
            Some(Value::String(s)) => {
                let v = s.clone();
                f["thumb_sizes"] = serde_json::json!([v]);
            }
            Some(Value::Array(_)) => {}
            Some(_) => {
                return Err(format!(
                    "field '{}' 'thumb_sizes' must be a string or array of sizes",
                    name
                ));
            }
            None => {}
        }

        if field_type == RELATION_FIELD_TYPE
            && f.get("target_collection")
                .and_then(|t| t.as_str())
                .map(str::is_empty)
                .unwrap_or(true)
        {
            return Err(format!(
                "relation field '{}' requires a non-empty 'target_collection'",
                name
            ));
        }

        normalized_fields.push(f);
    }

    let mut result = value.clone();
    result["fields"] = serde_json::json!(normalized_fields);
    Ok(result)
}

/// Read an optional numeric config key as f64.
fn cfg_f64(field_def: &Value, key: &str) -> Option<f64> {
    field_def.get(key).and_then(Value::as_f64)
}

/// Read an optional numeric config key as u64.
fn cfg_u64(field_def: &Value, key: &str) -> Option<u64> {
    field_def.get(key).and_then(Value::as_u64)
}

/// Whether the field is configured to hold multiple values.
fn is_multiple(field_def: &Value) -> bool {
    field_def.get("multiple").and_then(Value::as_bool).unwrap_or(false)
}

/// Coerce a value into the list of element values to validate. For `multiple`
/// fields the value is expected to be an array; otherwise it is a single scalar.
fn as_elements(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(arr) => arr.iter().collect(),
        other => vec![other],
    }
}

/// Lightweight email format check (no full RFC 5322 — catches obvious mistakes).
fn looks_like_email(s: &str) -> bool {
    let mut parts = s.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => {
            !local.is_empty()
                && !domain.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !s.chars().any(char::is_whitespace)
        }
        _ => false,
    }
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
                Some(Value::Array(arr)) if arr.is_empty() => {
                    return Some(format!("Required field '{}' cannot be empty", name));
                }
                _ => {}
            }
        }

        if let Some(v) = value
            && !v.is_null()
        {
            if let Some(err) = validate_field_value(name, field_type, field_def, v) {
                return Some(err);
            }
        }
    }

    None
}

/// Validate one present, non-null value against its field definition.
fn validate_field_value(name: &str, field_type: &str, field_def: &Value, v: &Value) -> Option<String> {
    // Enforce `max_select` for multi-value fields up front.
    if is_multiple(field_def) {
        if let Value::Array(arr) = v {
            if let Some(max) = cfg_u64(field_def, "max_select")
                && arr.len() as u64 > max
            {
                return Some(format!("Field '{}' allows at most {} selections", name, max));
            }
            if let Some(min) = cfg_u64(field_def, "min_select")
                && (arr.len() as u64) < min
            {
                return Some(format!("Field '{}' requires at least {} selections", name, min));
            }
        }
    }

    match field_type {
        "text" | "textarea" | "rich_text" => {
            for el in as_elements(v) {
                if let Some(err) = validate_text_value(name, field_type, field_def, el) {
                    return Some(err);
                }
            }
            None
        }
        "email" => {
            for el in as_elements(v) {
                match el.as_str() {
                    Some(s) if s.is_empty() || looks_like_email(s) => {}
                    Some(s) => return Some(format!("Field '{}' must be a valid email, got '{}'", name, s)),
                    None => return Some(format!("Field '{}' must be a string email", name)),
                }
            }
            None
        }
        "url" => {
            for el in as_elements(v) {
                match el.as_str() {
                    Some(s) if s.is_empty() => {}
                    Some(s) if url::Url::parse(s).is_ok() => {}
                    Some(s) => return Some(format!("Field '{}' must be a valid URL, got '{}'", name, s)),
                    None => return Some(format!("Field '{}' must be a string URL", name)),
                }
            }
            None
        }
        "json" => {
            // Any present JSON value is structurally valid; enforce only the byte cap.
            if let Some(max) = cfg_u64(field_def, "max_size") {
                let size = serde_json::to_string(v).map(|s| s.len()).unwrap_or(0) as u64;
                if size > max {
                    return Some(format!("Field '{}' exceeds max size of {} bytes", name, max));
                }
            }
            None
        }
        "number" => {
            if !v.is_number() {
                return Some(format!("Field '{}' must be a number", name));
            }
            let n = v.as_f64().unwrap_or(0.0);
            if let Some(min) = cfg_f64(field_def, "min")
                && n < min
            {
                return Some(format!("Field '{}' must be >= {}", name, min));
            }
            if let Some(max) = cfg_f64(field_def, "max")
                && n > max
            {
                return Some(format!("Field '{}' must be <= {}", name, max));
            }
            None
        }
        "boolean" => {
            if !v.is_boolean() {
                return Some(format!("Field '{}' must be a boolean", name));
            }
            None
        }
        "select" => {
            let valid: Vec<&str> = field_def
                .get("options")
                .and_then(|o| o.as_array())
                .map(|opts| opts.iter().filter_map(|o| o.as_str()).collect())
                .unwrap_or_default();
            if valid.is_empty() {
                return None;
            }
            for el in as_elements(v) {
                if let Some(s) = el.as_str()
                    && !valid.contains(&s)
                {
                    return Some(format!(
                        "Field '{}' value '{}' is not in allowed options: {:?}",
                        name, s, valid
                    ));
                }
            }
            None
        }
        t if FILE_FIELD_TYPES.contains(&t) => {
            for el in as_elements(v) {
                if let Some(s) = el.as_str()
                    && !s.is_empty()
                    && !s.starts_with("/api/files/")
                    && !s.starts_with("http")
                {
                    return Some(format!("Field '{}' must be a valid file URL, got '{}'", name, s));
                }
            }
            None
        }
        // `relation` existence is validated asynchronously in the entry/singleton
        // services (needs DB access); here we only ensure element shape.
        RELATION_FIELD_TYPE => {
            for el in as_elements(v) {
                if !el.is_string() {
                    return Some(format!("Field '{}' relation values must be entry ids (strings)", name));
                }
            }
            None
        }
        _ => None,
    }
}

/// Length + regex validation for text-like values.
fn validate_text_value(name: &str, field_type: &str, field_def: &Value, el: &Value) -> Option<String> {
    let s = match el.as_str() {
        Some(s) => s,
        None => return Some(format!("Field '{}' must be a string", name)),
    };

    let char_len = s.chars().count() as u64;
    if let Some(min) = cfg_u64(field_def, "min_length")
        && char_len < min
    {
        return Some(format!("Field '{}' must be at least {} characters", name, min));
    }
    if let Some(max) = cfg_u64(field_def, "max_length")
        && char_len > max
    {
        return Some(format!("Field '{}' must be at most {} characters", name, max));
    }

    // `max_size` (bytes) primarily for rich_text payloads.
    if let Some(max) = cfg_u64(field_def, "max_size")
        && (s.len() as u64) > max
    {
        return Some(format!("Field '{}' exceeds max size of {} bytes", name, max));
    }

    // Regex validation only for plain text fields, and only when a non-empty value.
    if field_type == "text"
        && !s.is_empty()
        && let Some(pattern) = field_def.get("pattern").and_then(|p| p.as_str())
        && !pattern.is_empty()
    {
        match regex::Regex::new(pattern) {
            Ok(re) => {
                if !re.is_match(s) {
                    return Some(format!("Field '{}' does not match required pattern", name));
                }
            }
            Err(_) => {
                return Some(format!("Field '{}' has an invalid validation pattern", name));
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

    #[test]
    fn normalize_definition_accepts_new_types() {
        for t in ["email", "url", "json"] {
            let input = json!({"fields": [{"name": "f", "type": t}]});
            assert!(normalize_definition(&input).is_ok(), "type {t} should be valid");
        }
    }

    #[test]
    fn normalize_definition_relation_requires_target() {
        let input = json!({"fields": [{"name": "author", "type": "relation"}]});
        assert!(normalize_definition(&input).is_err());
        let ok = json!({"fields": [{"name": "author", "type": "relation", "target_collection": "users"}]});
        assert!(normalize_definition(&ok).is_ok());
    }

    #[test]
    fn validate_text_min_max_length() {
        let fields = json!([{"name": "t", "type": "text", "min_length": 3, "max_length": 5}]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"t": "ab"}), f)
                .unwrap()
                .contains("at least 3")
        );
        assert!(
            validate_entry_data(&json!({"t": "abcdef"}), f)
                .unwrap()
                .contains("at most 5")
        );
        assert!(validate_entry_data(&json!({"t": "abcd"}), f).is_none());
    }

    #[test]
    fn validate_text_pattern() {
        let fields = json!([{"name": "code", "type": "text", "pattern": "^[a-z0-9]+$"}]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"code": "ABC"}), f)
                .unwrap()
                .contains("pattern")
        );
        assert!(validate_entry_data(&json!({"code": "abc123"}), f).is_none());
    }

    #[test]
    fn validate_email_format() {
        let fields = json!([{"name": "e", "type": "email"}]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"e": "nope"}), f)
                .unwrap()
                .contains("valid email")
        );
        assert!(validate_entry_data(&json!({"e": "a@b.com"}), f).is_none());
    }

    #[test]
    fn validate_url_format() {
        let fields = json!([{"name": "u", "type": "url"}]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"u": "not a url"}), f)
                .unwrap()
                .contains("valid URL")
        );
        assert!(validate_entry_data(&json!({"u": "https://example.com"}), f).is_none());
    }

    #[test]
    fn validate_json_max_size() {
        let fields = json!([{"name": "j", "type": "json", "max_size": 5}]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"j": {"a": "bbbbbb"}}), f)
                .unwrap()
                .contains("max size")
        );
        assert!(validate_entry_data(&json!({"j": 1}), f).is_none());
    }

    #[test]
    fn validate_number_min_max() {
        let fields = json!([{"name": "n", "type": "number", "min": 1, "max": 10}]);
        let f = fields.as_array().unwrap();
        assert!(validate_entry_data(&json!({"n": 0}), f).unwrap().contains(">= 1"));
        assert!(validate_entry_data(&json!({"n": 11}), f).unwrap().contains("<= 10"));
        assert!(validate_entry_data(&json!({"n": 5}), f).is_none());
    }

    #[test]
    fn validate_multiple_select_and_max_select() {
        let fields = json!([
            {"name": "tags", "type": "select", "multiple": true, "max_select": 2,
             "options": ["a", "b", "c"]}
        ]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"tags": ["a", "z"]}), f)
                .unwrap()
                .contains("allowed options")
        );
        assert!(
            validate_entry_data(&json!({"tags": ["a", "b", "c"]}), f)
                .unwrap()
                .contains("at most 2")
        );
        assert!(validate_entry_data(&json!({"tags": ["a", "b"]}), f).is_none());
    }

    #[test]
    fn validate_required_empty_array() {
        let fields = json!([{"name": "imgs", "type": "image", "multiple": true, "required": true}]);
        let f = fields.as_array().unwrap();
        assert!(
            validate_entry_data(&json!({"imgs": []}), f)
                .unwrap()
                .contains("cannot be empty")
        );
    }
}
