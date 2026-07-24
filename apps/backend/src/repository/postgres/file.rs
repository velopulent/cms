use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::file::{File, FileReference};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{FileListResult, FileRepository, ListFilesParams, NewFile};

pub struct PostgresFileRepository {
    pool: PgPool,
}

impl PostgresFileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FileRepository for PostgresFileRepository {
    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<File>, RepositoryError> {
        let result = sqlx::query_as::<_, File>(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at::text as deleted_at, created_by, created_at::text as created_at
             FROM files WHERE id = $1 AND site_id = $2",
        )
        .bind(id)
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_id_any(&self, id: &str) -> Result<Option<File>, RepositoryError> {
        let result = sqlx::query_as::<_, File>(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at::text as deleted_at, created_by, created_at::text as created_at
             FROM files WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list(&self, params: ListFilesParams<'_>) -> Result<FileListResult, RepositoryError> {
        let deleted_clause = if params.trashed {
            "deleted_at IS NOT NULL"
        } else {
            "deleted_at IS NULL"
        };

        let mut query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at::text as deleted_at, created_by, created_at::text as created_at FROM files WHERE site_id = $1 AND {}",
            deleted_clause,
        );
        let mut count_query = format!("SELECT COUNT(*) FROM files WHERE site_id = $1 AND {}", deleted_clause,);

        let mut bindings: Vec<String> = vec![params.site_id.to_string()];
        let mut param_index = 2;

        if let Some(search) = params.search {
            query.push_str(&format!(
                " AND (original_name LIKE ${} OR filename LIKE ${})",
                param_index,
                param_index + 1
            ));
            count_query.push_str(&format!(
                " AND (original_name LIKE ${} OR filename LIKE ${})",
                param_index,
                param_index + 1
            ));
            let pattern = format!("%{}%", search);
            bindings.push(pattern.clone());
            bindings.push(pattern);
            param_index += 2;
        }

        if let Some(type_filter) = params.file_type {
            match type_filter {
                "image" => {
                    query.push_str(" AND mime_type LIKE 'image/%'");
                    count_query.push_str(" AND mime_type LIKE 'image/%'");
                }
                "video" => {
                    query.push_str(" AND mime_type LIKE 'video/%'");
                    count_query.push_str(" AND mime_type LIKE 'video/%'");
                }
                "document" => {
                    let clause = " AND (mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/%' OR mime_type LIKE 'text/%')";
                    query.push_str(clause);
                    count_query.push_str(clause);
                }
                mime => {
                    query.push_str(&format!(" AND mime_type = ${}", param_index));
                    count_query.push_str(&format!(" AND mime_type = ${}", param_index));
                    bindings.push(mime.to_string());
                    param_index += 1;
                }
            }
        }

        let count_bindings = bindings.clone();

        let offset = (params.page - 1) * params.per_page;
        let per_page = params.per_page;
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_index,
            param_index + 1
        ));

        let mut count_q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_query.as_str()));
        for b in &count_bindings {
            count_q = count_q.bind(b);
        }
        let total: i64 = count_q.fetch_optional(&self.pool).await.unwrap_or(Some(0)).unwrap_or(0);

        let mut q = sqlx::query_as::<_, File>(sqlx::AssertSqlSafe(query.as_str()));
        for b in &bindings {
            q = q.bind(b);
        }
        q = q.bind(per_page).bind(offset);

        let items = q.fetch_all(&self.pool).await?;

        Ok(FileListResult {
            items,
            total,
            page: params.page,
            per_page: params.per_page,
        })
    }

    async fn create(&self, file: NewFile<'_>) -> Result<File, RepositoryError> {
        let NewFile {
            id,
            site_id,
            filename,
            original_name,
            mime_type,
            size,
            storage_provider,
            storage_key,
            thumbnail_key,
            width,
            height,
            created_by,
        } = file;
        sqlx::query(
            "INSERT INTO files (id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(id)
        .bind(site_id)
        .bind(filename)
        .bind(original_name)
        .bind(mime_type)
        .bind(size)
        .bind(storage_provider)
        .bind(storage_key)
        .bind(thumbnail_key)
        .bind(width)
        .bind(height)
        .bind(created_by)
        .execute(&self.pool)
        .await?;

        self.get_by_id_any(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn soft_delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query("UPDATE files SET deleted_at = NOW() WHERE id = $1 AND site_id = $2 AND deleted_at IS NULL")
                .bind(id)
                .bind(site_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected())
    }

    async fn restore(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query("UPDATE files SET deleted_at = NULL WHERE id = $1 AND site_id = $2 AND deleted_at IS NOT NULL")
                .bind(id)
                .bind(site_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected())
    }

    async fn batch_soft_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("${}", i + 2)).collect();
        let query = format!(
            "UPDATE files SET deleted_at = NOW() WHERE site_id = $1 AND id IN ({}) AND deleted_at IS NULL",
            placeholders.join(",")
        );

        let mut q = sqlx::query(sqlx::AssertSqlSafe(query.as_str())).bind(site_id);
        for id in ids {
            q = q.bind(id);
        }

        let result = q.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    async fn batch_restore(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("${}", i + 2)).collect();
        let query = format!(
            "UPDATE files SET deleted_at = NULL WHERE site_id = $1 AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders.join(",")
        );

        let mut q = sqlx::query(sqlx::AssertSqlSafe(query.as_str())).bind(site_id);
        for id in ids {
            q = q.bind(id);
        }

        let result = q.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    async fn get_by_ids(&self, site_id: &str, ids: &[String]) -> Result<Vec<File>, RepositoryError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("${}", i + 2)).collect();
        let query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at::text as deleted_at, created_by, created_at::text as created_at FROM files WHERE site_id = $1 AND id IN ({})",
            placeholders.join(",")
        );

        let mut q = sqlx::query_as::<_, File>(sqlx::AssertSqlSafe(query.as_str())).bind(site_id);
        for id in ids {
            q = q.bind(id);
        }

        let result = q.fetch_all(&self.pool).await?;
        Ok(result)
    }

    async fn get_deleted_by_ids(&self, site_id: &str, ids: &[String]) -> Result<Vec<File>, RepositoryError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("${}", i + 2)).collect();
        let query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at::text as deleted_at, created_by, created_at::text as created_at FROM files WHERE site_id = $1 AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders.join(",")
        );

        let mut q = sqlx::query_as::<_, File>(sqlx::AssertSqlSafe(query.as_str())).bind(site_id);
        for id in ids {
            q = q.bind(id);
        }

        let result = q.fetch_all(&self.pool).await?;
        Ok(result)
    }

    async fn batch_permanent_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("${}", i + 2)).collect();
        let query = format!(
            "DELETE FROM files WHERE site_id = $1 AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders.join(",")
        );

        let mut q = sqlx::query(sqlx::AssertSqlSafe(query.as_str())).bind(site_id);
        for id in ids {
            q = q.bind(id);
        }

        let result = q.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    async fn get_references(&self, file_id: &str) -> Result<Vec<FileReference>, RepositoryError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT DISTINCT e.id, col.name FROM entry_file_references efr
             JOIN entries e ON efr.entry_id = e.id
             JOIN collections col ON e.collection_id = col.id
             WHERE efr.file_id = $1",
        )
        .bind(file_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(entry_id, collection_name)| FileReference {
                entry_id,
                collection_name,
                field_name: String::new(),
            })
            .collect())
    }

    async fn get_references_for_site(
        &self,
        file_id: &str,
        site_id: &str,
    ) -> Result<Vec<FileReference>, RepositoryError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT DISTINCT e.id, col.name FROM entry_file_references efr
             JOIN entries e ON efr.entry_id = e.id
             JOIN collections col ON e.collection_id = col.id
             WHERE efr.file_id = $1 AND e.site_id = $2",
        )
        .bind(file_id)
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(entry_id, collection_name)| FileReference {
                entry_id,
                collection_name,
                field_name: String::new(),
            })
            .collect())
    }

    async fn get_storage_provider(&self, site_id: &str) -> Result<String, RepositoryError> {
        let provider: Option<String> =
            sqlx::query_scalar("SELECT COALESCE(storage_profile_id, storage_provider) FROM sites WHERE id = $1")
                .bind(site_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(provider.unwrap_or_else(|| "filesystem".into()))
    }

    async fn set_thumbnail_meta(
        &self,
        id: &str,
        thumbnail_key: &str,
        width: Option<i32>,
        height: Option<i32>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE files SET thumbnail_key = $1, width = $2, height = $3 WHERE id = $4")
            .bind(thumbnail_key)
            .bind(width)
            .bind(height)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
