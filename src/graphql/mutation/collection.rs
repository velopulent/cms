use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::graphql::context::GqlContext;
use crate::graphql::types::collection::*;

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

        let result = sqlx::query(
            "INSERT INTO collections (id, site_id, name, slug, definition) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(site_id)
        .bind(&input.name)
        .bind(&input.slug)
        .bind(&definition_str)
        .execute(&gql_ctx.pool)
        .await;

        match result {
            Ok(_) => {
                let db_collection = sqlx::query_as::<_, crate::models::collection::Collection>(
                    "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE id = ?",
                )
                .bind(&id)
                .fetch_one(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

                Ok(db_collection_to_gql(db_collection))
            }
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

        let existing = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
        )
        .bind(site_id)
        .bind(&slug)
        .fetch_optional(&gql_ctx.pool)
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
                let old_fields = old_d["fields"].as_array().cloned().unwrap_or_default();
                let new_fields = new_d["fields"].as_array().cloned().unwrap_or_default();

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
                        if let (Some(on), Some(nn)) = (of["name"].as_str(), nf["name"].as_str())
                        {
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

                if !rename_map.is_empty() {
                    let contents = sqlx::query_as::<_, crate::models::content::Content>(
                        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE collection_id = ?",
                    )
                    .bind(&existing.id)
                    .fetch_all(&gql_ctx.pool)
                    .await;

                    if let Ok(items) = contents {
                        for content in &items {
                            if let Ok(mut data) =
                                serde_json::from_str::<serde_json::Value>(&content.data)
                            {
                                if let Some(obj) = data.as_object_mut() {
                                    let mut renamed = serde_json::Map::new();
                                    for (key, value) in obj.iter() {
                                        let new_key = rename_map.get(key).cloned().unwrap_or_else(|| key.clone());
                                        renamed.insert(new_key, value.clone());
                                    }
                                    let new_data_str = serde_json::to_string(&serde_json::Value::Object(renamed))
                                        .unwrap_or_else(|_| content.data.clone());

                                    let _ = sqlx::query(
                                        "UPDATE content SET data = ?, updated_at = datetime('now') WHERE id = ?",
                                    )
                                    .bind(&new_data_str)
                                    .bind(&content.id)
                                    .execute(&gql_ctx.pool)
                                    .await;
                                }
                            }
                        }
                    }
                }
            }
        }

        sqlx::query(
            "UPDATE collections SET name = ?, slug = ?, definition = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&name)
        .bind(&new_slug)
        .bind(&definition_str)
        .bind(&existing.id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        let db_collection = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE id = ?",
        )
        .bind(&existing.id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_collection_to_gql(db_collection))
    }

    pub async fn delete_collection(&self, ctx: &Context<'_>, slug: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let _ = sqlx::query("DELETE FROM collections WHERE site_id = ? AND slug = ?")
            .bind(site_id)
            .bind(&slug)
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }
}
