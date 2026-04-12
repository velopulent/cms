use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::site::{Site, SiteMember, SiteWithRole};
use crate::repository::error::RepositoryError;
use crate::repository::traits::SiteRepository;

pub struct PostgresSiteRepository {
    pool: PgPool,
}

impl PostgresSiteRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SiteRepository for PostgresSiteRepository {
    async fn list_all(&self) -> Result<Vec<Site>, RepositoryError> {
        let result = sqlx::query_as::<_, Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at::text as created_at, updated_at::text as updated_at FROM sites ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list_for_user(&self, user_id: &str) -> Result<Vec<SiteWithRole>, RepositoryError> {
        let result = sqlx::query_as::<_, SiteWithRole>(
            "SELECT s.id, s.name, s.default_storage_provider, s.created_by, s.created_at::text as created_at, s.updated_at::text as updated_at, sm.role
             FROM sites s
             JOIN site_members sm ON s.id = sm.site_id
             WHERE sm.user_id = $1
             ORDER BY s.name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Site>, RepositoryError> {
        let result = sqlx::query_as::<_, Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at::text as created_at, updated_at::text as updated_at FROM sites WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn create(
        &self,
        id: &str,
        name: &str,
        storage_provider: &str,
        created_by: &str,
    ) -> Result<Site, RepositoryError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("INSERT INTO sites (id, name, default_storage_provider, created_by) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(name)
            .bind(storage_provider)
            .bind(created_by)
            .execute(&mut *tx)
            .await?;

        let member_id = uuid::Uuid::now_v7().to_string();
        sqlx::query("INSERT INTO site_members (id, site_id, user_id, role) VALUES ($1, $2, $3, 'owner')")
            .bind(&member_id)
            .bind(id)
            .bind(created_by)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        self.get_by_id(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn update(&self, id: &str, name: &str, storage_provider: &str) -> Result<Site, RepositoryError> {
        sqlx::query("UPDATE sites SET name = $1, default_storage_provider = $2, updated_at = NOW() WHERE id = $3")
            .bind(name)
            .bind(storage_provider)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_by_id(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM sites WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn list_members(&self, site_id: &str) -> Result<Vec<SiteMember>, RepositoryError> {
        let result = sqlx::query_as::<_, SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at::text as created_at
             FROM site_members sm
             JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = $1
             ORDER BY sm.role DESC, u.username",
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn add_member(
        &self,
        id: &str,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<SiteMember, RepositoryError> {
        sqlx::query("INSERT INTO site_members (id, site_id, user_id, role) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(site_id)
            .bind(user_id)
            .bind(role)
            .execute(&self.pool)
            .await?;

        let result = sqlx::query_as::<_, SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at::text as created_at
             FROM site_members sm JOIN users u ON sm.user_id = u.id WHERE sm.id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    async fn update_member_role(
        &self,
        site_id: &str,
        user_id: &str,
        role: &str,
    ) -> Result<Option<SiteMember>, RepositoryError> {
        let result = sqlx::query("UPDATE site_members SET role = $1 WHERE site_id = $2 AND user_id = $3")
            .bind(role)
            .bind(site_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        let member = sqlx::query_as::<_, SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at::text as created_at
             FROM site_members sm JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = $1 AND sm.user_id = $2",
        )
        .bind(site_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Some(member))
    }

    async fn remove_member(&self, site_id: &str, user_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM site_members WHERE site_id = $1 AND user_id = $2")
            .bind(site_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
