use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;
use crate::graphql::types::collection::{SingletonGraphql, singleton_to_gql};
use crate::graphql::types::json::Json;

pub struct SingletonMutation;

#[Object]
impl SingletonMutation {
    pub async fn update_singleton(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        slug: String,
        data: Json,
        change_summary: Option<String>,
        expected_version: Option<String>,
    ) -> Result<SingletonGraphql> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::ContentWrite)
            .await?;
        let value = gql_ctx
            .services
            .singleton
            .update_singleton(
                &site_id,
                &slug,
                &data.0,
                gql_ctx.user_id(),
                change_summary.as_deref(),
                expected_version.as_deref(),
            )
            .await
            .map_err(|error| crate::graphql::service_error("mutation.update_singleton", error))?;
        Ok(singleton_to_gql(value))
    }
}
