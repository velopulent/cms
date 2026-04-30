use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::models::file::{File, FileReference};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{FileListResult, FileRepository, ListFilesParams};

pub struct SqliteFileRepository {
    pool: SqlitePool,
}

impl SqliteFileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FileRepository for SqliteFileRepository {
    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<File>, RepositoryError> {
        let result = sqlx::query_as::<_, File>(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at
             FROM files WHERE id = ? AND site_id = ?",
        )
        .bind(id)
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_id_any(&self, id: &str) -> Result<Option<File>, RepositoryError> {
        let result = sqlx::query_as::<_, File>(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at
             FROM files WHERE id = ?",
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
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE site_id = ? AND {}",
            deleted_clause,
        );
        let mut count_query = format!("SELECT COUNT(*) FROM files WHERE site_id = ? AND {}", deleted_clause,);

        let mut bindings: Vec<String> = vec![params.site_id.to_string()];

        if let Some(search) = params.search {
            query.push_str(" AND (original_name LIKE ? OR filename LIKE ?)");
            count_query.push_str(" AND (original_name LIKE ? OR filename LIKE ?)");
            let pattern = format!("%{}%", search);
            bindings.push(pattern.clone());
            bindings.push(pattern);
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
                _ => {}
            }
        }

        let count_bindings = bindings.clone();

        let offset = (params.page - 1) * params.per_page;
        query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        bindings.push(params.per_page.to_string());
        bindings.push(offset.to_string());

        let mut count_q = sqlx::query_scalar::<_, i64>(&count_query);
        for b in &count_bindings {
            count_q = count_q.bind(b);
        }
        let total: i64 = count_q.fetch_optional(&self.pool).await.unwrap_or(Some(0)).unwrap_or(0);

        let mut q = sqlx::query_as::<_, File>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let items = q.fetch_all(&self.pool).await?;

        Ok(FileListResult {
            items,
            total,
            page: params.page,
            per_page: params.per_page,
        })
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        filename: &str,
        original_name: &str,
        mime_type: &str,
        size: i64,
        storage_provider: &str,
        storage_key: &str,
        thumbnail_key: Option<&str>,
        width: Option<i32>,
        height: Option<i32>,
        created_by: Option<&str>,
    ) -> Result<File, RepositoryError> {
        sqlx::query(
            "INSERT INTO files (id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        let result = sqlx::query(
            "UPDATE files SET deleted_at = datetime('now') WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(site_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    async fn restore(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query("UPDATE files SET deleted_at = NULL WHERE id = ? AND site_id = ? AND deleted_at IS NOT NULL")
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

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "UPDATE files SET deleted_at = datetime('now') WHERE site_id = ? AND id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(site_id);
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

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "UPDATE files SET deleted_at = NULL WHERE site_id = ? AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(site_id);
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

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE site_id = ? AND id IN ({})",
            placeholders
        );

        let mut q = sqlx::query_as::<_, File>(&query).bind(site_id);
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

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE site_id = ? AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query_as::<_, File>(&query).bind(site_id);
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

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "DELETE FROM files WHERE site_id = ? AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(site_id);
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
             WHERE efr.file_id = ?",
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
             WHERE efr.file_id = ? AND e.site_id = ?",
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
        let provider: Option<String> = sqlx::query_scalar("SELECT storage_provider FROM sites WHERE id = ?")
            .bind(site_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(provider.unwrap_or_else(|| "filesystem".into()))
    }
}
