use axum::http::HeaderMap;
use axum::{
    Extension, Router,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use std::sync::Arc;

use crate::config::Config;
use crate::graphql::context::GqlContext;
use crate::graphql::schema::{CmsSchema, build_schema};
use crate::repository::Repository;
use crate::services::Services;

async fn graphql_handler(
    axum::extract::Extension(schema): axum::extract::Extension<Arc<CmsSchema>>,
    axum::extract::Extension(repository): axum::extract::Extension<Repository>,
    axum::extract::Extension(services): axum::extract::Extension<Services>,
    axum::extract::Extension(config): axum::extract::Extension<Config>,
    headers: HeaderMap,
    req: async_graphql_axum::GraphQLRequest,
) -> async_graphql_axum::GraphQLResponse {
    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());

    // Per-request DataLoader (request-scoped cache) to batch nested resolvers.
    let entry_loader = async_graphql::dataloader::DataLoader::new(
        crate::graphql::loaders::EntryLoader {
            repository: repository.clone(),
        },
        tokio::spawn,
    );

    let gql_ctx = GqlContext::from_request(
        repository,
        services,
        auth_header,
        &config.token_index_key,
        config.clone(),
    )
    .await;

    let mut request = req.into_inner();
    let validation = request.parsed_query().and_then(|document| {
        use async_graphql::parser::types::Selection;
        let mut sets = document
            .operations
            .iter()
            .map(|(_, operation)| &operation.node.selection_set.node)
            .chain(
                document
                    .fragments
                    .values()
                    .map(|fragment| &fragment.node.selection_set.node),
            )
            .collect::<Vec<_>>();
        let mut aliases = 0;
        while let Some(set) = sets.pop() {
            for selection in &set.items {
                match &selection.node {
                    Selection::Field(field) => {
                        aliases += usize::from(field.node.alias.is_some());
                        if aliases > 256 {
                            return Err(async_graphql::ServerError::new(
                                "Query exceeds the 256-alias limit",
                                None,
                            ));
                        }
                        sets.push(&field.node.selection_set.node);
                    }
                    Selection::InlineFragment(fragment) => sets.push(&fragment.node.selection_set.node),
                    Selection::FragmentSpread(_) => {}
                }
            }
        }
        Ok(())
    });
    if let Err(error) = validation {
        return async_graphql::Response::from_errors(vec![error]).into();
    }
    let request = if gql_ctx.actor.is_none() {
        request.disable_introspection()
    } else {
        request
    };
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        schema.execute(request.data(gql_ctx).data(entry_loader)),
    )
    .await
    .unwrap_or_else(|_| {
        async_graphql::Response::from_errors(vec![async_graphql::ServerError::new("Request timed out", None)])
    });
    async_graphql_axum::GraphQLResponse::from(response)
}

async fn graphiql_handler() -> impl IntoResponse {
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/api/graphql")
            .finish(),
    )
}

pub fn graphql_routes(production: bool) -> Router {
    let router = if production {
        Router::new().route("/api/graphql", post(graphql_handler))
    } else {
        Router::new().route("/api/graphql", get(graphiql_handler).post(graphql_handler))
    };
    router.layer(Extension(Arc::new(build_schema(production))))
}
