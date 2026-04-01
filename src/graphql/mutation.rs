use async_graphql::{Context, Object, Result};
use bcrypt::{DEFAULT_COST, hash, verify};
use uuid::Uuid;

use super::context::GqlContext;
use super::types::*;
use crate::middleware::auth::create_token;

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // --- Auth ---

    async fn register(&self, ctx: &Context<'_>, input: RegisterInput) -> Result<AuthResponse> {
        let gql_ctx = ctx.data::<GqlContext>()?;

        if input.username.trim().is_empty() || input.password.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "Username and password are required",
            ));
        }

        let password_hash = hash(&input.password, DEFAULT_COST)
            .map_err(|e| async_graphql::Error::new(format!("Hash error: {}", e)))?;

        let id = Uuid::now_v7().to_string();

        let result =
            sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)")
                .bind(&id)
                .bind(&input.username)
                .bind(&input.email)
                .bind(&password_hash)
                .execute(&gql_ctx.pool)
                .await;

        match result {
            Ok(_) => {
                let token = create_token(id.clone(), &gql_ctx.config.jwt_secret)
                    .map_err(|e| async_graphql::Error::new(format!("Token error: {}", e)))?;

                Ok(AuthResponse {
                    token,
                    user: UserPublic {
                        id,
                        username: input.username,
                        email: input.email,
                    },
                })
            }
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                Err(async_graphql::Error::new(
                    "Username or email already exists",
                ))
            }
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    async fn login(&self, ctx: &Context<'_>, input: LoginInput) -> Result<AuthResponse> {
        let gql_ctx = ctx.data::<GqlContext>()?;

        let user: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT id, username, email, password_hash FROM users WHERE username = ?",
        )
        .bind(&input.username)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        let (id, username, email, password_hash) = user
            .ok_or_else(|| async_graphql::Error::new("Invalid username or password"))?;

        let valid = verify(&input.password, &password_hash).unwrap_or(false);
        if !valid {
            return Err(async_graphql::Error::new("Invalid username or password"));
        }

        let token = create_token(id.clone(), &gql_ctx.config.jwt_secret)
            .map_err(|e| async_graphql::Error::new(format!("Token error: {}", e)))?;

        Ok(AuthResponse {
            token,
            user: UserPublic {
                id,
                username,
                email,
            },
        })
    }

    // --- Sites ---

    async fn create_site(&self, ctx: &Context<'_>, input: CreateSiteInput) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let user_id = gql_ctx.require_jwt()?;

        if input.name.trim().is_empty() {
            return Err(async_graphql::Error::new("Name is required"));
        }

        let site_id = Uuid::now_v7().to_string();
        let member_id = Uuid::now_v7().to_string();

        let storage_provider = input
            .default_storage_provider
            .as_deref()
            .unwrap_or("filesystem");
        if storage_provider != "filesystem" && storage_provider != "s3" {
            return Err(async_graphql::Error::new(
                "Invalid storage provider. Must be 'filesystem' or 's3'",
            ));
        }

        let mut tx = gql_ctx
            .pool
            .begin()
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        sqlx::query(
            "INSERT INTO sites (id, name, default_storage_provider, created_by) VALUES (?, ?, ?, ?)",
        )
        .bind(&site_id)
        .bind(&input.name)
        .bind(storage_provider)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        sqlx::query(
            "INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, 'owner')",
        )
        .bind(&member_id)
        .bind(&site_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        tx.commit()
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        let db_site = sqlx::query_as::<_, crate::models::site::Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
        )
        .bind(&site_id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(Site {
            id: db_site.id,
            name: db_site.name,
            default_storage_provider: db_site.default_storage_provider,
            created_by: db_site.created_by,
            created_at: db_site.created_at,
            updated_at: db_site.updated_at,
        })
    }

    async fn update_site(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateSiteInput,
    ) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&id, "admin").await?;

        let existing = sqlx::query_as::<_, crate::models::site::Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
        .ok_or_else(|| async_graphql::Error::new("Site not found"))?;

        let name = input.name.unwrap_or(existing.name);
        let storage_provider = input
            .default_storage_provider
            .filter(|v| v == "filesystem" || v == "s3")
            .unwrap_or(existing.default_storage_provider);

        sqlx::query(
            "UPDATE sites SET name = ?, default_storage_provider = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&name)
        .bind(&storage_provider)
        .bind(&id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        let db_site = sqlx::query_as::<_, crate::models::site::Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(Site {
            id: db_site.id,
            name: db_site.name,
            default_storage_provider: db_site.default_storage_provider,
            created_by: db_site.created_by,
            created_at: db_site.created_at,
            updated_at: db_site.updated_at,
        })
    }

    async fn delete_site(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&id, "owner").await?;

        sqlx::query("DELETE FROM sites WHERE id = ?")
            .bind(&id)
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    // --- Members ---

    async fn invite_member(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        input: InviteMemberInput,
    ) -> Result<SiteMember> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "admin").await?;

        let valid_roles = ["owner", "admin", "editor", "viewer"];
        if !valid_roles.contains(&input.role.as_str()) {
            return Err(async_graphql::Error::new(
                "Invalid role. Must be owner, admin, editor, or viewer",
            ));
        }

        let user: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(&input.username)
            .fetch_optional(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        let user_id = match user {
            Some((id,)) => id,
            None => return Err(async_graphql::Error::new("User not found")),
        };

        let member_id = Uuid::now_v7().to_string();

        let result =
            sqlx::query("INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, ?)")
                .bind(&member_id)
                .bind(&site_id)
                .bind(&user_id)
                .bind(&input.role)
                .execute(&gql_ctx.pool)
                .await;

        match result {
            Ok(_) => {
                let db_member = sqlx::query_as::<_, crate::models::site::SiteMember>(
                    "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
                     FROM site_members sm JOIN users u ON sm.user_id = u.id WHERE sm.id = ?",
                )
                .bind(&member_id)
                .fetch_one(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

                Ok(SiteMember {
                    id: db_member.id,
                    site_id: db_member.site_id,
                    user_id: db_member.user_id,
                    username: db_member.username,
                    email: db_member.email,
                    role: db_member.role,
                    created_at: db_member.created_at,
                })
            }
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                Err(async_graphql::Error::new(
                    "User is already a member of this site",
                ))
            }
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    async fn update_member_role(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        user_id: String,
        input: UpdateMemberRoleInput,
    ) -> Result<SiteMember> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "owner").await?;

        let valid_roles = ["owner", "admin", "editor", "viewer"];
        if !valid_roles.contains(&input.role.as_str()) {
            return Err(async_graphql::Error::new("Invalid role"));
        }

        let result =
            sqlx::query("UPDATE site_members SET role = ? WHERE site_id = ? AND user_id = ?")
                .bind(&input.role)
                .bind(&site_id)
                .bind(&user_id)
                .execute(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("Member not found"));
        }

        let db_member = sqlx::query_as::<_, crate::models::site::SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
             FROM site_members sm JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = ? AND sm.user_id = ?",
        )
        .bind(&site_id)
        .bind(&user_id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(SiteMember {
            id: db_member.id,
            site_id: db_member.site_id,
            user_id: db_member.user_id,
            username: db_member.username,
            email: db_member.email,
            role: db_member.role,
            created_at: db_member.created_at,
        })
    }

    async fn remove_member(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        user_id: String,
    ) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let auth_user_id = gql_ctx.require_site_access(&site_id, "admin").await?;

        if auth_user_id == user_id {
            return Err(async_graphql::Error::new(
                "Cannot remove yourself from the site",
            ));
        }

        let result =
            sqlx::query("DELETE FROM site_members WHERE site_id = ? AND user_id = ?")
                .bind(&site_id)
                .bind(&user_id)
                .execute(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("Member not found"));
        }

        Ok(true)
    }

    // --- API Keys ---

    async fn create_api_key(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        input: CreateApiKeyInput,
    ) -> Result<ApiKeyResponse> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "admin").await?;

        if input.name.trim().is_empty() {
            return Err(async_graphql::Error::new("Name is required"));
        }

        let random_chars = Uuid::new_v4().to_string().replace('-', "");
        let segment_a: String = random_chars.chars().take(8).collect();
        let segment_b: String = random_chars.chars().skip(8).take(24).collect();
        let raw_key = format!("cms_{}_{}", segment_a, segment_b);

        let prefix: String = raw_key.chars().take(16).collect();

        let key_hash = hash(&raw_key, DEFAULT_COST)
            .map_err(|e| async_graphql::Error::new(format!("Hash error: {}", e)))?;

        let id = Uuid::now_v7().to_string();

        sqlx::query(
            "INSERT INTO api_keys (id, site_id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&site_id)
        .bind(&input.name)
        .bind(&key_hash)
        .bind(&prefix)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        Ok(ApiKeyResponse {
            id,
            site_id,
            name: input.name,
            key: raw_key,
            key_prefix: prefix,
            permissions: "read".to_string(),
            created_at: now,
        })
    }

    async fn delete_api_key(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        key_id: String,
    ) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "admin").await?;

        let result = sqlx::query("DELETE FROM api_keys WHERE id = ? AND site_id = ?")
            .bind(&key_id)
            .bind(&site_id)
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("API key not found"));
        }

        Ok(true)
    }

    // --- Collections ---

    async fn create_collection(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        input: CreateCollectionInput,
    ) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let definition_str = input.definition.to_string();
        let id = Uuid::now_v7().to_string();

        let result = sqlx::query(
            "INSERT INTO collections (id, site_id, name, slug, definition) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&site_id)
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

    async fn update_collection(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        slug: String,
        input: UpdateCollectionInput,
    ) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let existing = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
        )
        .bind(&site_id)
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

        // Handle field renames in content data
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
                    if used_old[i] {
                        continue;
                    }
                    for (j, nf) in new_fields.iter().enumerate() {
                        if used_new[j] {
                            continue;
                        }
                        if of["name"] != nf["name"]
                            && of["type"] == nf["type"]
                            && of.get("required") == nf.get("required")
                            && of.get("options") == nf.get("options")
                        {
                            if let (Some(on), Some(nn)) =
                                (of["name"].as_str(), nf["name"].as_str())
                            {
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
                                        let new_key = rename_map
                                            .get(key)
                                            .cloned()
                                            .unwrap_or_else(|| key.clone());
                                        renamed.insert(new_key, value.clone());
                                    }
                                    let new_data = serde_json::Value::Object(renamed);
                                    let new_data_str = serde_json::to_string(&new_data)
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

    async fn delete_collection(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        slug: String,
    ) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        sqlx::query("DELETE FROM collections WHERE site_id = ? AND slug = ?")
            .bind(&site_id)
            .bind(&slug)
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    // --- Content ---

    async fn create_content(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        input: CreateContentInput,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let data_str = input.data.to_string();
        let id = Uuid::now_v7().to_string();

        let result = sqlx::query(
            "INSERT INTO content (id, site_id, collection_id, data, slug) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&site_id)
        .bind(&input.collection_id)
        .bind(&data_str)
        .bind(&input.slug)
        .execute(&gql_ctx.pool)
        .await;

        match result {
            Ok(_) => {
                let db_content = sqlx::query_as::<_, crate::models::content::Content>(
                    "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
                )
                .bind(&id)
                .fetch_one(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

                Ok(db_content_to_gql(db_content))
            }
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                Err(async_graphql::Error::new(
                    "Content with this slug already exists for this collection",
                ))
            }
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    async fn update_content(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
        input: UpdateContentInput,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let existing = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(&site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
        .ok_or_else(|| async_graphql::Error::new("Content not found"))?;

        let resolved_data = match input.data {
            Some(d) => d.0,
            None => serde_json::from_str(&existing.data).unwrap_or(serde_json::Value::Null),
        };
        let data_str = resolved_data.to_string();
        let slug = input.slug.unwrap_or(existing.slug);
        let status = input.status.unwrap_or(existing.status);

        let result = sqlx::query(
            "UPDATE content SET data = ?, slug = ?, status = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&data_str)
        .bind(&slug)
        .bind(&status)
        .bind(&id)
        .execute(&gql_ctx.pool)
        .await;

        match result {
            Ok(_) => {
                let db_content = sqlx::query_as::<_, crate::models::content::Content>(
                    "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
                )
                .bind(&id)
                .fetch_one(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

                Ok(db_content_to_gql(db_content))
            }
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                Err(async_graphql::Error::new(
                    "Content with this slug already exists for this collection",
                ))
            }
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    async fn delete_content(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
    ) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        sqlx::query("DELETE FROM content WHERE id = ? AND site_id = ?")
            .bind(&id)
            .bind(&site_id)
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    async fn publish_content(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let result = sqlx::query(
            "UPDATE content SET status = 'published', published_at = datetime('now'), updated_at = datetime('now') WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(&site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("Content not found"));
        }

        let db_content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }

    async fn unpublish_content(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let result = sqlx::query(
            "UPDATE content SET status = 'draft', updated_at = datetime('now') WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(&site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("Content not found"));
        }

        let db_content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }

    // --- Files ---

    async fn delete_file(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
    ) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let result = sqlx::query(
            "UPDATE files SET deleted_at = datetime('now') WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
        )
        .bind(&id)
        .bind(&site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("File not found"));
        }

        Ok(true)
    }

    async fn restore_file(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
    ) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        let result = sqlx::query(
            "UPDATE files SET deleted_at = NULL WHERE id = ? AND site_id = ? AND deleted_at IS NOT NULL",
        )
        .bind(&id)
        .bind(&site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("File not found or not deleted"));
        }

        Ok(true)
    }

    async fn batch_delete_files(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        ids: Vec<String>,
    ) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        if ids.is_empty() {
            return Err(async_graphql::Error::new("No file IDs provided"));
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "UPDATE files SET deleted_at = datetime('now') WHERE site_id = ? AND id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(&site_id);
        for id in &ids {
            q = q.bind(id);
        }

        let result = q
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result.rows_affected() as i64)
    }

    async fn batch_restore_files(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        ids: Vec<String>,
    ) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "editor").await?;

        if ids.is_empty() {
            return Err(async_graphql::Error::new("No file IDs provided"));
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "UPDATE files SET deleted_at = NULL WHERE site_id = ? AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(&site_id);
        for id in &ids {
            q = q.bind(id);
        }

        let result = q
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result.rows_affected() as i64)
    }
}
