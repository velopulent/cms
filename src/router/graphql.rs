use axum::http::HeaderMap;
use axum::{
    Extension, Router,
    response::{Html, IntoResponse},
    routing::get,
};
use std::sync::Arc;

use crate::config::Config;
use crate::graphql::context::GqlContext;
use crate::graphql::schema::{CmsSchema, build_schema};
use crate::handlers::file_handler::StorageManager;
use crate::repository::Repository;

async fn graphql_handler(
    axum::extract::Extension(schema): axum::extract::Extension<Arc<CmsSchema>>,
    axum::extract::Extension(repository): axum::extract::Extension<Repository>,
    axum::extract::Extension(storage): axum::extract::Extension<StorageManager>,
    axum::extract::Extension(config): axum::extract::Extension<Config>,
    headers: HeaderMap,
    req: async_graphql_axum::GraphQLRequest,
) -> async_graphql_axum::GraphQLResponse {
    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());

    let gql_ctx = GqlContext::from_request(repository, storage, auth_header, &config.hmac_secret).await;

    let response = schema.execute(req.into_inner().data(gql_ctx)).await;
    async_graphql_axum::GraphQLResponse::from(response)
}

async fn graphiql_handler() -> impl IntoResponse {
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/api/graphql")
            .finish(),
    )
}

pub fn graphql_routes() -> Router {
    Router::new()
        .route("/api/graphql", get(graphiql_handler).post(graphql_handler))
        .layer(Extension(Arc::new(build_schema())))
}
