use axum::{extract::Request, http::HeaderName, middleware::Next, response::Response};
use tracing::{Instrument, info_span};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, prelude::*};
use uuid::Uuid;

pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

pub fn init_tracing(config: &crate::config::Config) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter = EnvFilter::new(&config.log_level);

    let log_output = config.log_output.as_str();
    let env_filter_str = env_filter.to_string();

    match log_output {
        "file" => {
            let log_dir = config.log_dir.as_str();
            let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "cms.log");
            let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = fmt::layer().with_writer(file_writer).with_target(true).json();

            tracing_subscriber::registry().with(env_filter).with(file_layer).init();

            tracing::info!(
                log_output = %log_output,
                log_format = "json",
                log_dir = %log_dir,
                rust_log = %env_filter_str,
                "Tracing initialized"
            );

            Some(guard)
        }
        _ => {
            let stdout_layer = fmt::layer().with_target(true).pretty();

            tracing_subscriber::registry()
                .with(env_filter)
                .with(stdout_layer)
                .init();

            tracing::info!(
                log_output = %log_output,
                log_format = "pretty",
                rust_log = %env_filter_str,
                "Tracing initialized"
            );

            None
        }
    }
}

/// Minimal stderr tracing for `vcms mcp stdio`, which is a thin proxy that loads no
/// `Config` (it touches no disk). stdout is the MCP protocol channel, so logs go to stderr.
pub fn init_proxy_tracing() {
    let env_filter = EnvFilter::new("vcms=info,cms=info");
    let layer = fmt::layer().with_writer(std::io::stderr).with_target(true);
    tracing_subscriber::registry().with(env_filter).with(layer).init();
}

pub async fn trace_request(req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER.as_str())
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    let public_api = req.uri().path().starts_with("/api/v1/");
    let span = info_span!(
        "http_request",
        request_id = %request_id,
        method = %req.method(),
        uri = %safe_request_uri(req.uri()),
    );

    let mut response = next.run(req).instrument(span).await;
    if public_api && (response.status().is_client_error() || response.status().is_server_error()) {
        response = public_problem(response, &request_id).await;
    }
    let _ = response
        .headers_mut()
        .insert(REQUEST_ID_HEADER.clone(), request_id.parse().unwrap());
    response
}

/// Signed upload URLs contain bearer credentials in their path.
pub fn safe_request_uri(uri: &axum::http::Uri) -> String {
    if uri.path().starts_with("/api/v1/files/upload/") {
        "/api/v1/files/upload/[redacted]".into()
    } else {
        uri.path().to_owned()
    }
}

async fn public_problem(response: Response, request_id: &str) -> Response {
    use axum::response::IntoResponse;
    let (mut parts, body) = response.into_parts();
    let status = parts.status;
    let bytes = axum::body::to_bytes(body, 64 * 1024).await.unwrap_or_default();
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default();
    let code = value
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(match status.as_u16() {
            400 | 422 => "invalid_request",
            401 => "unauthorized",
            403 => "forbidden",
            404 => "not_found",
            405 => "method_not_allowed",
            409 => "conflict",
            412 => "precondition_failed",
            413 => "payload_too_large",
            429 => "rate_limited",
            500..=599 => "internal_error",
            _ => "request_failed",
        });
    let detail = if status.is_server_error() {
        "Internal server error".to_owned()
    } else {
        value
            .get("detail")
            .or_else(|| value.get("message"))
            .or_else(|| value.get("error"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| status.canonical_reason().unwrap_or("Request failed").to_owned())
    };
    let body = serde_json::json!({"type":"about:blank", "title":status.canonical_reason().unwrap_or("Request failed"),
        "status":status.as_u16(), "code":code, "detail":detail, "error":detail, "request_id":request_id});
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    parts.headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/problem+json"),
    );
    Response::from_parts(parts, axum::Json(body).into_response().into_body())
}
