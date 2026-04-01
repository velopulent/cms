use async_graphql::{EmptySubscription, Schema};

use super::mutation::MutationRoot;
use super::query::QueryRoot;

pub type CmsSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema() -> CmsSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .limit_depth(15)
        .limit_complexity(2000)
        .finish()
}
