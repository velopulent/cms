use axum::{extract::Request, http::HeaderName, middleware::Next, response::Response};
use tracing::{Instrument, info_span};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, prelude::*};
use uuid::Uuid;

pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

pub fn init_tracing(config: &crate::config::Config) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let env_filter = EnvFilter::new(&config.log_level);

    let log_output = config.log_output.as_str();
    let log_format = config.log_format.as_str();
    let log_annotations = config.log_annotations;

    let env_filter_str = env_filter.to_string();

    match (log_output, log_format) {
        ("file", "json") | ("file", _) => {
            let log_dir = config.log_dir.as_str();
            let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "cms.log");
            let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = fmt::layer()
                .with_writer(file_writer)
                .with_target(true)
                .with_file(log_annotations)
                .with_line_number(log_annotations)
                .json();

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
        ("stdout", "json") => {
            let stdout_layer = fmt::layer()
                .with_target(true)
                .with_file(log_annotations)
                .with_line_number(log_annotations)
                .json();

            tracing_subscriber::registry()
                .with(env_filter)
                .with(stdout_layer)
                .init();

            tracing::info!(
                log_output = %log_output,
                log_format = "json",
                rust_log = %env_filter_str,
                "Tracing initialized"
            );

            None
        }
        _ => {
            let stdout_layer = fmt::layer()
                .with_target(true)
                .with_file(log_annotations)
                .with_line_number(log_annotations)
                .pretty();

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

pub fn init_stdio_tracing(config: &crate::config::Config) {
    let env_filter = EnvFilter::new(&config.log_level);
    let annotations = config.log_annotations;

    if config.log_format == "json" {
        let layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true)
            .with_file(annotations)
            .with_line_number(annotations)
            .json();
        tracing_subscriber::registry().with(env_filter).with(layer).init();
    } else {
        let layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true)
            .with_file(annotations)
            .with_line_number(annotations)
            .pretty();
        tracing_subscriber::registry().with(env_filter).with(layer).init();
    }

    tracing::info!(
        log_output = "stderr",
        log_format = %config.log_format,
        "MCP stdio tracing initialized"
    );
}

pub async fn trace_request(req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER.as_str())
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    let span = info_span!(
        "http_request",
        request_id = %request_id,
        method = %req.method(),
        uri = %req.uri(),
    );

    let mut response = next.run(req).instrument(span).await;
    let _ = response
        .headers_mut()
        .insert(REQUEST_ID_HEADER.clone(), request_id.parse().unwrap());
    response
}
