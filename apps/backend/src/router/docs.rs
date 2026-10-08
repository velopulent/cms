use axum::{Json, Router, routing::get};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::openapi::CmsApiDoc;

pub fn docs_routes() -> Router {
    let spec = CmsApiDoc::openapi();
    Router::new()
        .route(
            "/api/v1/openapi.json",
            get({
                let spec = spec.clone();
                move || async move { Json(spec.clone()) }
            }),
        )
        .merge(Scalar::with_url("/api/v1/docs", spec))
}
