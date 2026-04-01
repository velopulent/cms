use async_graphql::{EmptySubscription, Schema};

use super::mutation::MutationRoot;
use super::query::QueryRoot;

pub type CmsSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema() -> CmsSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .limit_depth(10)
        .limit_complexity(200)
        .finish()
}
