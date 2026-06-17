use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::models::site::{Site, SiteMember, SiteWithRole};
use crate::repository::error::RepositoryError;
use crate::repository::traits::SiteRepository;

pub struct MysqlSiteRepository {
    pool: MySqlPool,
}

impl MysqlSiteRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SiteRepository for MysqlSiteRepository {
    async fn list_all(&self) -> Result<Vec<Site>, RepositoryError> {
        let result = sqlx::query_as::<_, Site>(
            "SELECT id, name, storage_provider, created_by, created_at, updated_at FROM sites ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list_for_user(&self, user_id: &str) -> Result<Vec<SiteWithRole>, RepositoryError> {
        let result = sqlx::query_as::<_, SiteWithRole>(
            "SELECT s.id, s.name, s.storage_provider, s.created_by, s.created_at, s.updated_at, sm.role
             FROM sites s
             JOIN site_members sm ON s.id = sm.site_id
             WHERE sm.user_id = ?
             ORDER BY s.name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Site>, RepositoryError> {
        let result = sqlx::query_as::<_, Site>(
            "SELECT id, name, storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
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
        // The creator is an instance operator; site authority comes from their instance
        // role, not a site_members row, so no membership is inserted here.
        sqlx::query("INSERT INTO sites (id, name, storage_provider, created_by) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(name)
            .bind(storage_provider)
            .bind(created_by)
            .execute(&self.pool)
            .await?;

        self.get_by_id(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn update(&self, id: &str, name: &str) -> Result<Site, RepositoryError> {
        sqlx::query("UPDATE sites SET name = ?, updated_at = NOW() WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_by_id(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM sites WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn list_members(&self, site_id: &str) -> Result<Vec<SiteMember>, RepositoryError> {
        let result = sqlx::query_as::<_, SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
             FROM site_members sm
             JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = ?
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
        sqlx::query("INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(site_id)
            .bind(user_id)
            .bind(role)
            .execute(&self.pool)
            .await?;

        let result = sqlx::query_as::<_, SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
             FROM site_members sm JOIN users u ON sm.user_id = u.id WHERE sm.id = ?",
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
        let result = sqlx::query("UPDATE site_members SET role = ? WHERE site_id = ? AND user_id = ?")
            .bind(role)
            .bind(site_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        let member = sqlx::query_as::<_, SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
             FROM site_members sm JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = ? AND sm.user_id = ?",
        )
        .bind(site_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Some(member))
    }

    async fn remove_member(&self, site_id: &str, user_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM site_members WHERE site_id = ? AND user_id = ?")
            .bind(site_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}
