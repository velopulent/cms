use async_graphql::SimpleObject;

#[derive(SimpleObject)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub default_storage_provider: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}
