use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use mime_guess::from_path;

#[derive(RustEmbed)]
#[folder = "ui/dist"]
pub struct Assets;

pub async fn ui_handler(Path(path): Path<String>) -> Response {
    let requested_path = if path.is_empty() {
        "index.html"
    } else {
        path.as_str()
    };

    match Assets::get(requested_path) {
        Some(content) => {
            let mime = from_path(requested_path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data,
            )
                .into_response()
        }
        None => {
            // SPA fallback
            if let Some(index) = Assets::get("index.html") {
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html")],
                    index.data,
                )
                    .into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}
