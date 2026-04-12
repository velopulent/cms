use axum::Router;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::openapi::{AdminApiDoc, SiteApiDoc};

pub fn docs_routes() -> Router {
    Router::new()
        .merge(Scalar::with_url("/api/v1/docs/site", SiteApiDoc::openapi()))
        .merge(Scalar::with_url("/api/v1/docs/admin", AdminApiDoc::openapi()))
        .merge(Scalar::with_url("/api/v1/docs", SiteApiDoc::openapi()))
}
