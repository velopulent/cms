use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::site::{Site, SiteDetails};
use crate::models::authorization::Action;

pub struct QueryRoot;

/// Every content read lives under `site(id:)`, so a request always names the
/// site it targets and resolvers authorize against that site explicitly.
#[Object]
impl QueryRoot {
    /// Sites the credential can read.
    async fn sites(&self, ctx: &Context<'_>) -> Result<Vec<Site>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_token_scope(Action::SiteRead)?;
        let sites = gql_ctx
            .services
            .site
            .list_sites_for_actor(gql_ctx.require_actor()?)
            .await
            .map_err(|e| crate::graphql::service_error("query.sites", e))?;
        Ok(sites
            .into_iter()
            .filter_map(|site| {
                let text = |key: &str| site.get(key).and_then(|value| value.as_str()).map(str::to_owned);
                Some(Site {
                    id: text("id")?,
                    details: Some(SiteDetails {
                        name: text("name")?,
                        created_at: text("created_at")?,
                        updated_at: text("updated_at")?,
                    }),
                })
            })
            .collect())
    }

    /// Site namespace. Selecting it performs no lookup; each nested field checks
    /// the credential for its own action on this site.
    async fn site(&self, id: String) -> Site {
        Site { id, details: None }
    }
}
