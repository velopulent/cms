use async_graphql::{EmptySubscription, Schema};

use super::admin_mutation::AdminMutationRoot;
use super::admin_query::AdminQueryRoot;
use super::mutation::MutationRoot;
use super::query::QueryRoot;

pub type CmsSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;
pub type AdminCmsSchema = Schema<AdminQueryRoot, AdminMutationRoot, EmptySubscription>;

pub fn build_schema() -> CmsSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .limit_depth(15)
        .limit_complexity(2000)
        .finish()
}

pub fn build_admin_schema() -> AdminCmsSchema {
    Schema::build(AdminQueryRoot, AdminMutationRoot, EmptySubscription)
        .limit_depth(15)
        .limit_complexity(2000)
        .finish()
}
