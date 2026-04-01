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
        match value {
            Value::String(s) => {
                let v: serde_json::Value = serde_json::from_str(&s)
                    .map_err(|e| InputValueError::custom(format!("Invalid JSON: {}", e)))?;
                Ok(Json(v))
            }
            Value::Null => Ok(Json(serde_json::Value::Null)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Json(serde_json::Value::Number(i.into())))
                } else if let Some(f) = n.as_f64() {
                    Ok(Json(
                        serde_json::Number::from_f64(f)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    ))
                } else {
                    Ok(Json(serde_json::Value::Null))
                }
            }
            Value::Boolean(b) => Ok(Json(serde_json::Value::Bool(b))),
            Value::Enum(s) => Ok(Json(serde_json::Value::String(s.to_string()))),
            _ => {
                let json_val = serde_json::to_value(&value)
                    .map_err(|e| InputValueError::custom(e.to_string()))?;
                Ok(Json(json_val))
            }
        }
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
