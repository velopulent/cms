pub mod cms;
pub mod interceptor;
pub mod server;

/// Convert dynamic CMS JSON objects into protobuf's typed Struct representation.
pub fn json_to_struct(value: &serde_json::Value) -> Option<prost_types::Struct> {
    let serde_json::Value::Object(object) = value else {
        return None;
    };
    Some(prost_types::Struct {
        fields: object
            .iter()
            .map(|(key, value)| (key.clone(), json_to_value(value)))
            .collect(),
    })
}

pub fn struct_to_json(value: &prost_types::Struct) -> Result<serde_json::Value, tonic::Status> {
    Ok(serde_json::Value::Object(
        value
            .fields
            .iter()
            .map(|(key, value)| Ok((key.clone(), value_to_json(value)?)))
            .collect::<Result<_, tonic::Status>>()?,
    ))
}

/// Parse stored JSON text (entry data, collection definitions) into a Struct.
pub fn json_text_to_struct(text: &str) -> Option<prost_types::Struct> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| json_to_struct(&value))
}

pub const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 200;

pub fn page_size(requested: i32) -> i64 {
    if requested <= 0 {
        DEFAULT_PAGE_SIZE
    } else {
        i64::from(requested).min(MAX_PAGE_SIZE)
    }
}

/// First page without a token, otherwise the page a token issued for this exact
/// query (same `fingerprint`) points at.
pub fn resolve_page(page_token: &str, fingerprint: &str, secret: &str) -> Result<i64, tonic::Status> {
    if page_token.is_empty() {
        return Ok(1);
    }
    crate::utils::cursor::resolve_page(Some(page_token), fingerprint, secret).map_err(tonic::Status::invalid_argument)
}

pub fn next_page_token(page: i64, per_page: i64, total: i64, fingerprint: String, secret: &str) -> String {
    if page.saturating_mul(per_page) >= total {
        return String::new();
    }
    crate::utils::cursor::encode(
        &crate::utils::cursor::PageCursor {
            version: 1,
            page: page + 1,
            fingerprint,
        },
        secret,
    )
}

pub fn timestamp_from_text(value: &str) -> Option<prost_types::Timestamp> {
    let datetime = crate::utils::timestamp::parse_db_timestamp(value)?;
    Some(prost_types::Timestamp {
        seconds: datetime.timestamp(),
        nanos: datetime.timestamp_subsec_nanos() as i32,
    })
}

fn value_to_json(value: &prost_types::Value) -> Result<serde_json::Value, tonic::Status> {
    use prost_types::value::Kind;
    Ok(match value.kind.as_ref() {
        Some(Kind::NullValue(0)) => serde_json::Value::Null,
        Some(Kind::NullValue(_)) | None => return Err(tonic::Status::invalid_argument("Invalid protobuf Value kind")),
        Some(Kind::NumberValue(value)) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| tonic::Status::invalid_argument("Content numbers must be finite"))?,
        Some(Kind::StringValue(value)) => serde_json::Value::String(value.clone()),
        Some(Kind::BoolValue(value)) => serde_json::Value::Bool(*value),
        Some(Kind::StructValue(value)) => struct_to_json(value)?,
        Some(Kind::ListValue(value)) => {
            serde_json::Value::Array(value.values.iter().map(value_to_json).collect::<Result<_, _>>()?)
        }
    })
}

/// Preserve actionable domain failures without leaking infrastructure details.
pub fn service_error(error: impl Into<crate::services::error::ServiceError>) -> tonic::Status {
    let error = error.into();
    let status = error.status_code();
    let code = match status {
        http::StatusCode::UNAUTHORIZED => tonic::Code::Unauthenticated,
        http::StatusCode::FORBIDDEN => tonic::Code::PermissionDenied,
        http::StatusCode::NOT_FOUND => tonic::Code::NotFound,
        http::StatusCode::CONFLICT => tonic::Code::AlreadyExists,
        http::StatusCode::PRECONDITION_FAILED => tonic::Code::FailedPrecondition,
        http::StatusCode::PAYLOAD_TOO_LARGE | http::StatusCode::TOO_MANY_REQUESTS => tonic::Code::ResourceExhausted,
        http::StatusCode::BAD_REQUEST | http::StatusCode::UNPROCESSABLE_ENTITY => tonic::Code::InvalidArgument,
        _ => tonic::Code::Internal,
    };
    let message = if status.is_server_error() {
        tracing::error!(error = ?error, "gRPC operation failed");
        "Internal server error".to_owned()
    } else {
        error.error_message()
    };
    tonic::Status::new(code, message)
}

fn json_to_value(value: &serde_json::Value) -> prost_types::Value {
    use prost_types::value::Kind;
    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(value) => Kind::BoolValue(*value),
        serde_json::Value::Number(value) => Kind::NumberValue(value.as_f64().unwrap_or_default()),
        serde_json::Value::String(value) => Kind::StringValue(value.clone()),
        serde_json::Value::Array(values) => Kind::ListValue(prost_types::ListValue {
            values: values.iter().map(json_to_value).collect(),
        }),
        serde_json::Value::Object(values) => Kind::StructValue(prost_types::Struct {
            fields: values
                .iter()
                .map(|(key, value)| (key.clone(), json_to_value(value)))
                .collect(),
        }),
    };
    prost_types::Value { kind: Some(kind) }
}

pub mod services;

#[cfg(test)]
mod timestamp_tests {
    use super::*;
    #[test]
    fn typed_timestamps_preserve_sqlite_and_postgres_instants() {
        let canonical = timestamp_from_text("2026-09-16T08:00:00.123456Z").unwrap();
        for value in [
            "2026-09-16 08:00:00.123456",
            "2026-09-16 08:00:00.123456+00",
            "2026-09-16 13:30:00.123456+05:30",
        ] {
            assert_eq!(timestamp_from_text(value).unwrap(), canonical, "{value}");
        }
        assert_eq!(canonical.nanos, 123456000);
        assert!(timestamp_from_text("invalid").is_none());
    }
}
