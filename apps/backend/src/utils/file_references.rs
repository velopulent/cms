use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub fn extract_file_ids_from_value(value: &Value) -> Vec<String> {
    let mut ids = extract_file_references_from_value(value)
        .into_iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

pub fn extract_file_references_from_value(value: &Value) -> Vec<(String, String)> {
    static FILE_URL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"/api/files/([a-fA-F0-9-]+)(?:[/?.#\s"<>]|$)"#).unwrap());
    let mut refs = Vec::new();
    collect_file_references(value, &FILE_URL_RE, "", &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect_file_references(value: &Value, re: &Regex, path: &str, refs: &mut Vec<(String, String)>) {
    match value {
        Value::String(s) => {
            // Absolute URLs are external links, even if another CMS uses the
            // same route shape. Internal references use canonical relative URLs.
            if url::Url::parse(s).is_ok_and(|url| matches!(url.scheme(), "http" | "https")) {
                return;
            }
            for cap in re.captures_iter(s) {
                let start = cap.get(0).expect("matched file URL").start();
                let prefix = s[..start]
                    .rsplit([' ', '\n', '\r', '\t', '"', '\'', '<', '>'])
                    .next()
                    .unwrap_or_default();
                if prefix.contains("://") {
                    continue;
                }
                if let Some(m) = cap.get(1) {
                    refs.push((m.as_str().to_ascii_lowercase(), path.to_string()));
                }
            }
        }
        Value::Array(arr) => {
            for (index, item) in arr.iter().enumerate() {
                collect_file_references(item, re, &format!("{path}[{index}]"), refs);
            }
        }
        Value::Object(obj) => {
            for (key, val) in obj {
                let next = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_file_references(val, re, &next, refs);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_cms_urls_are_not_internal_ownership_references() {
        let value = serde_json::json!({"external":"https://other.example/api/files/abc-123","html":"<img src=\"https://other.example/api/files/abc-123\">","internal":"/api/files/abc-123"});
        assert_eq!(
            super::extract_file_references_from_value(&value),
            vec![("abc-123".into(), "internal".into())]
        );
    }
}
