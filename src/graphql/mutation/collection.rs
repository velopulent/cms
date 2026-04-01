use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::graphql::context::GqlContext;
use crate::graphql::types::collection::*;
use crate::repository::collection as collection_repo;

pub struct CollectionMutation;

#[Object]
impl CollectionMutation {
    pub async fn create_collection(
        &self,
        ctx: &Context<'_>,
        input: CreateCollectionInput,
    ) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let definition_str = input.definition.to_string();
        let id = Uuid::now_v7().to_string();

        match collection_repo::create(
            &gql_ctx.pool,
            &id,
            site_id,
            &input.name,
            &input.slug,
            &definition_str,
        )
        .await
        {
            Ok(db_collection) => Ok(db_collection_to_gql(db_collection)),
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                Err(async_graphql::Error::new(
                    "Collection with this name or slug already exists",
                ))
            }
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    pub async fn update_collection(
        &self,
        ctx: &Context<'_>,
        slug: String,
        input: UpdateCollectionInput,
    ) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let existing = collection_repo::get_by_slug(&gql_ctx.pool, site_id, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Collection not found"))?;

        let name = input.name.unwrap_or(existing.name);
        let new_slug = input.slug.unwrap_or(existing.slug);
        let definition_str = input
            .definition
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| existing.definition.clone());

        if let Some(ref new_def_json) = input.definition {
            let new_def_value = &new_def_json.0;
            let old_def: Option<serde_json::Value> =
                serde_json::from_str(&existing.definition).ok();
            let new_def: Option<serde_json::Value> =
                serde_json::from_value(new_def_value.clone()).ok();

            if let (Some(old_d), Some(new_d)) = (old_def, new_def) {
                let rename_map = compute_field_rename_map(&old_d, &new_d);

                if !rename_map.is_empty() {
                    if let Ok(items) =
                        collection_repo::get_content_for_migration(&gql_ctx.pool, &existing.id)
                            .await
                    {
                        collection_repo::migrate_content_field_renames(
                            &gql_ctx.pool,
                            &items,
                            &rename_map,
                        )
                        .await;
                    }
                }
            }
        }

        let db_collection =
            collection_repo::update(&gql_ctx.pool, &existing.id, &name, &new_slug, &definition_str)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_collection_to_gql(db_collection))
    }

    pub async fn delete_collection(&self, ctx: &Context<'_>, slug: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        collection_repo::delete(&gql_ctx.pool, site_id, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }
}

fn compute_field_rename_map(
    old_def: &serde_json::Value,
    new_def: &serde_json::Value,
) -> std::collections::HashMap<String, String> {
    let old_fields = old_def["fields"].as_array().cloned().unwrap_or_default();
    let new_fields = new_def["fields"].as_array().cloned().unwrap_or_default();

    let mut rename_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut used_old = vec![false; old_fields.len()];
    let mut used_new = vec![false; new_fields.len()];

    for i in 0..old_fields.len().min(new_fields.len()) {
        let of = &old_fields[i];
        let nf = &new_fields[i];
        if of["name"] != nf["name"]
            && of["type"] == nf["type"]
            && of.get("required") == nf.get("required")
            && of.get("options") == nf.get("options")
        {
            if let (Some(on), Some(nn)) = (of["name"].as_str(), nf["name"].as_str()) {
                rename_map.insert(on.to_string(), nn.to_string());
                used_old[i] = true;
                used_new[i] = true;
            }
        }
    }

    for (i, of) in old_fields.iter().enumerate() {
        if used_old[i] { continue; }
        for (j, nf) in new_fields.iter().enumerate() {
            if used_new[j] { continue; }
            if of["name"] != nf["name"]
                && of["type"] == nf["type"]
                && of.get("required") == nf.get("required")
                && of.get("options") == nf.get("options")
            {
                if let (Some(on), Some(nn)) = (of["name"].as_str(), nf["name"].as_str()) {
                    rename_map.insert(on.to_string(), nn.to_string());
                    used_old[i] = true;
                    used_new[j] = true;
                }
                break;
            }
        }
    }

    rename_map
}
