use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

#[cfg(feature = "embed-dashboard")]
use axum::http::HeaderMap;
#[cfg(feature = "embed-dashboard")]
use mime_guess::from_path;

#[cfg(feature = "embed-dashboard")]
use rust_embed::RustEmbed;

#[cfg(feature = "embed-dashboard")]
#[derive(RustEmbed)]
#[folder = "../dashboard/dist"]
pub struct Assets;

/// Cache policy for an embedded asset path: Vite content-hashes everything
/// under `assets/`, so those are immutable; `index.html` (and the SPA
/// fallback) must revalidate every time; the rest (favicon etc.) get a short
/// TTL. ETags (embed-time sha256) let revalidations answer with a 304.
#[cfg(feature = "embed-dashboard")]
fn cache_control_for(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=3600"
    }
}

#[cfg(feature = "embed-dashboard")]
fn serve_embedded(path: &str, content: rust_embed::EmbeddedFile, headers: &HeaderMap) -> Response {
    let etag = format!("\"{}\"", hex::encode(content.metadata.sha256_hash()));
    let cache_control = cache_control_for(path);

    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|inm| inm == etag)
    {
        return (
            StatusCode::NOT_MODIFIED,
            [(header::ETAG, etag), (header::CACHE_CONTROL, cache_control.to_string())],
        )
            .into_response();
    }

    let mime = from_path(path).first_or_octet_stream();
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime.as_ref().to_string()),
            (header::ETAG, etag),
            (header::CACHE_CONTROL, cache_control.to_string()),
        ],
        content.data,
    )
        .into_response()
}

#[cfg(feature = "embed-dashboard")]
pub async fn dashboard_handler(Path(path): Path<String>, headers: HeaderMap) -> Response {
    let requested_path = if path.is_empty() { "index.html" } else { path.as_str() };

    match Assets::get(requested_path) {
        Some(content) => serve_embedded(requested_path, content, &headers),
        None => {
            // SPA fallback
            if let Some(index) = Assets::get("index.html") {
                serve_embedded("index.html", index, &headers)
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}

#[cfg(all(test, feature = "embed-dashboard"))]
mod tests {
    use super::*;

    #[test]
    fn cache_control_hashed_assets_are_immutable() {
        assert_eq!(
            cache_control_for("assets/index-abc123.js"),
            "public, max-age=31536000, immutable"
        );
    }

    #[test]
    fn cache_control_index_html_revalidates() {
        assert_eq!(cache_control_for("index.html"), "no-cache");
    }

    #[test]
    fn cache_control_other_files_get_short_ttl() {
        assert_eq!(cache_control_for("favicon.ico"), "public, max-age=3600");
    }

    #[test]
    fn serve_embedded_returns_304_on_matching_etag() {
        let Some(content) = Assets::get("index.html") else {
            return; // no dashboard build embedded in this test run
        };
        let etag = format!("\"{}\"", hex::encode(content.metadata.sha256_hash()));
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag.parse().unwrap());
        let response = serve_embedded("index.html", content, &headers);
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }
}

#[cfg(not(feature = "embed-dashboard"))]
pub async fn dashboard_handler(Path(_path): Path<String>, _headers: axum::http::HeaderMap) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain")],
        "Dashboard not available in development mode. \
         Run with the embed-dashboard feature enabled, or use the Vite dev server (nx dev dashboard).",
    )
        .into_response()
}
