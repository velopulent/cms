use axum::Router;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use super::openapi::ApiDoc;

pub fn docs_routes() -> Router {
    Router::new().merge(Scalar::with_url("/api/v1/docs", ApiDoc::openapi()))
}
