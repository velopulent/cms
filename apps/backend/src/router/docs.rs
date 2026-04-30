use axum::Router;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::openapi::CmsApiDoc;

pub fn docs_routes() -> Router {
    Router::new().merge(Scalar::with_url("/api/v1/docs", CmsApiDoc::openapi()))
}
