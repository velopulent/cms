use std::fmt;

use async_graphql::{ComplexObject, InputValueError, InputValueResult, Scalar, ScalarType, SimpleObject, Value, InputObject};

/// A JSON scalar type for GraphQL.
///
/// Wraps `serde_json::Value` to work around Rust orphan rules.
#[derive(Debug, Clone, PartialEq)]
pub struct Json(pub serde_json::Value);

#[Scalar(name = "JSON")]
impl ScalarType for Json {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => {
                let v: serde_json::Value = serde_json::from_str(&s)
                    .map_err(|e| InputValueError::custom(format!("Invalid JSON: {}", e)))?;
                Ok(Json(v))
            }
            Value::Null => Ok(Json(serde_json::Value::Null)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Json(serde_json::Value::Number(i.into())))
                } else if let Some(f) = n.as_f64() {
                    Ok(Json(
                        serde_json::Number::from_f64(f)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    ))
                } else {
                    Ok(Json(serde_json::Value::Null))
                }
            }
            Value::Boolean(b) => Ok(Json(serde_json::Value::Bool(b))),
            Value::Enum(s) => Ok(Json(serde_json::Value::String(s.to_string()))),
            _ => {
                // For complex values (lists, objects), serialize via serde_json
                let json_val = serde_json::to_value(&value)
                    .map_err(|e| InputValueError::custom(e.to_string()))?;
                Ok(Json(json_val))
            }
        }
    }

    fn to_value(&self) -> Value {
        serde_json::from_value(self.0.clone()).unwrap_or(Value::Null)
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<serde_json::Value> for Json {
    fn from(v: serde_json::Value) -> Self {
        Json(v)
    }
}

impl From<Json> for serde_json::Value {
    fn from(j: Json) -> Self {
        j.0
    }
}

// --- Output types ---

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub default_storage_provider: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[ComplexObject]
impl Site {
    async fn members(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<SiteMember>> {
        let gql_ctx = ctx.data::<super::context::GqlContext>()?;
        gql_ctx.require_site_access(&self.id, "viewer").await?;

        let members = sqlx::query_as::<_, crate::models::site::SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
             FROM site_members sm
             JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = ?
             ORDER BY sm.role DESC, u.username",
        )
        .bind(&self.id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(members
            .into_iter()
            .map(|m| SiteMember {
                id: m.id,
                site_id: m.site_id,
                user_id: m.user_id,
                username: m.username,
                email: m.email,
                role: m.role,
                created_at: m.created_at,
            })
            .collect())
    }

    async fn collections(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<Collection>> {
        let gql_ctx = ctx.data::<super::context::GqlContext>()?;
        gql_ctx.require_site_access(&self.id, "viewer").await?;

        let db_collections = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? ORDER BY name",
        )
        .bind(&self.id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_collections.into_iter().map(db_collection_to_gql).collect())
    }
}

pub fn db_collection_to_gql(c: crate::models::collection::Collection) -> Collection {
    let definition =
        serde_json::from_str(&c.definition).unwrap_or(serde_json::Value::Object(Default::default()));
    Collection {
        id: c.id,
        site_id: c.site_id,
        name: c.name,
        slug: c.slug,
        definition: Json(definition),
        created_at: c.created_at,
        updated_at: c.updated_at,
    }
}

pub fn db_content_to_gql(c: crate::models::content::Content) -> Content {
    let data = serde_json::from_str(&c.data).unwrap_or(serde_json::Value::Null);
    Content {
        id: c.id,
        site_id: c.site_id,
        collection_id: c.collection_id,
        data: Json(data),
        slug: c.slug,
        status: c.status,
        created_at: c.created_at,
        updated_at: c.updated_at,
        published_at: c.published_at,
    }
}

pub fn db_file_to_gql(f: crate::models::file::File, gql_ctx: &super::context::GqlContext) -> File {
    let url = match f.storage_provider.as_str() {
        "s3" => gql_ctx
            .storage
            .s3
            .as_ref()
            .map(|s| s.url(&f.storage_key))
            .unwrap_or_else(|| format!("/api/files/{}", f.id)),
        _ => format!("/api/files/{}", f.id),
    };
    let thumbnail_url = f
        .thumbnail_key
        .as_ref()
        .map(|_| format!("/api/files/{}/thumbnail", f.id));

    File {
        id: f.id,
        site_id: f.site_id,
        filename: f.filename,
        original_name: f.original_name,
        mime_type: f.mime_type,
        size: f.size,
        url,
        thumbnail_url,
        width: f.width,
        height: f.height,
        created_by: f.created_by,
        created_at: f.created_at,
    }
}

#[derive(SimpleObject)]
pub struct SiteWithRole {
    pub id: String,
    pub name: String,
    pub default_storage_provider: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub role: String,
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Collection {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: Json,
    pub created_at: String,
    pub updated_at: String,
}

#[ComplexObject]
impl Collection {
    async fn content(
        &self,
        ctx: &async_graphql::Context<'_>,
        status: Option<String>,
    ) -> async_graphql::Result<Vec<Content>> {
        let gql_ctx = ctx.data::<super::context::GqlContext>()?;

        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
             FROM content
             WHERE collection_id = ?",
        );
        let mut bindings: Vec<String> = vec![self.id.clone()];

        if let Some(s) = status {
            query.push_str(" AND status = ?");
            bindings.push(s);
        } else if matches!(
            gql_ctx.auth,
            Some(super::context::GqlAuth::ApiKey { .. })
        ) {
            query.push_str(" AND status = 'published'");
        }

        query.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query_as::<_, crate::models::content::Content>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let items = q
            .fetch_all(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(items.into_iter().map(db_content_to_gql).collect())
    }
}

#[derive(SimpleObject)]
pub struct Content {
    pub id: String,
    pub site_id: String,
    pub collection_id: String,
    pub data: Json,
    pub slug: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct File {
    pub id: String,
    pub site_id: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_by: String,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct SiteMember {
    pub id: String,
    pub site_id: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct ApiKey {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub key_prefix: String,
    pub permissions: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct ApiKeyResponse {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub permissions: String,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct UserPublic {
    pub id: String,
    pub username: String,
    pub email: String,
}

#[derive(SimpleObject)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

#[derive(SimpleObject)]
pub struct FileReference {
    pub content_id: String,
    pub collection_name: String,
    pub field_name: String,
}

// --- Input types ---

#[derive(InputObject)]
pub struct CreateSiteInput {
    pub name: String,
    pub default_storage_provider: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateSiteInput {
    pub name: Option<String>,
    pub default_storage_provider: Option<String>,
}

#[derive(InputObject)]
pub struct CreateCollectionInput {
    pub name: String,
    pub slug: String,
    pub definition: Json,
}

#[derive(InputObject)]
pub struct UpdateCollectionInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub definition: Option<Json>,
}

#[derive(InputObject)]
pub struct CreateContentInput {
    pub collection_id: String,
    pub data: Json,
    pub slug: String,
}

#[derive(InputObject)]
pub struct UpdateContentInput {
    pub data: Option<Json>,
    pub slug: Option<String>,
    pub status: Option<String>,
}

#[derive(InputObject)]
pub struct InviteMemberInput {
    pub username: String,
    pub role: String,
}

#[derive(InputObject)]
pub struct UpdateMemberRoleInput {
    pub role: String,
}

#[derive(InputObject)]
pub struct CreateApiKeyInput {
    pub name: String,
}

#[derive(InputObject)]
pub struct RegisterInput {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(InputObject)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}
