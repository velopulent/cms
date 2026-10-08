use std::fmt;

use async_graphql::{InputValueError, InputValueResult, Scalar, ScalarType, Value};

/// A JSON scalar type for GraphQL.
///
/// Wraps `serde_json::Value` to work around Rust orphan rules
/// (can't implement async-graphql traits for foreign types).
#[derive(Debug, Clone, PartialEq)]
pub struct Json(pub serde_json::Value);

#[Scalar(name = "JSON")]
impl ScalarType for Json {
    fn parse(value: Value) -> InputValueResult<Self> {
        value
            .into_json()
            .map(Json)
            .map_err(|error| InputValueError::custom(error.to_string()))
    }

    fn to_value(&self) -> Value {
        serde_json::from_value(self.0.clone()).unwrap_or(Value::Null)
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<serde_json::Value> for Json {
    fn from(v: serde_json::Value) -> Self {
        Json(v)
    }
}

impl From<Json> for serde_json::Value {
    fn from(j: Json) -> Self {
        j.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_scalar_preserves_values_without_double_decoding_or_precision_loss() {
        for original in [
            serde_json::json!("{\"x\":1}"),
            serde_json::json!(u64::MAX),
            serde_json::json!({"nested":[null, true, 1.5, "hello"]}),
        ] {
            let parsed = Json::parse(Value::from_json(original.clone()).unwrap()).unwrap();
            assert_eq!(parsed.0, original);
            assert_eq!(parsed.to_value().into_json().unwrap(), original);
        }
    }
}
