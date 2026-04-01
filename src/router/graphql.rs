use axum::{Extension, Router, response::{Html, IntoResponse}, routing::get};
use axum::http::HeaderMap;
use sqlx::SqlitePool;

use crate::graphql::context::GqlContext;
use crate::graphql::schema::CmsSchema;
use crate::handlers::file_handler::StorageManager;

async fn graphql_handler(
    axum::extract::Extension(schema): axum::extract::Extension<CmsSchema>,
    axum::extract::Extension(pool): axum::extract::Extension<SqlitePool>,
    axum::extract::Extension(storage): axum::extract::Extension<StorageManager>,
    headers: HeaderMap,
    req: async_graphql_axum::GraphQLRequest,
) -> async_graphql_axum::GraphQLResponse {
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let gql_ctx = GqlContext::from_request(pool, storage, auth_header).await;

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
    use crate::graphql::schema::build_schema;

    Router::new()
        .route("/api/graphql", get(graphiql_handler).post(graphql_handler))
        .layer(Extension(build_schema()))
}
