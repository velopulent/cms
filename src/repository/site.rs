use sqlx::SqlitePool;

use crate::models::site::{Site, SiteMember, SiteWithRole};

// --- Sites ---

pub async fn list_for_user(
    pool: &SqlitePool,
    user_id: &str,
) -> Result<Vec<SiteWithRole>, sqlx::Error> {
    sqlx::query_as::<_, SiteWithRole>(
        "SELECT s.id, s.name, s.default_storage_provider, s.created_by, s.created_at, s.updated_at, sm.role
         FROM sites s
         JOIN site_members sm ON s.id = sm.site_id
         WHERE sm.user_id = ?
         ORDER BY s.name",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Site>, sqlx::Error> {
    sqlx::query_as::<_, Site>(
        "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    storage_provider: &str,
    created_by: &str,
) -> Result<Site, sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO sites (id, name, default_storage_provider, created_by) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(storage_provider)
    .bind(created_by)
    .execute(&mut *tx)
    .await?;

    let member_id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, 'owner')",
    )
    .bind(&member_id)
    .bind(id)
    .bind(created_by)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    get_by_id(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    storage_provider: &str,
) -> Result<Site, sqlx::Error> {
    sqlx::query(
        "UPDATE sites SET name = ?, default_storage_provider = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(name)
    .bind(storage_provider)
    .bind(id)
    .execute(pool)
    .await?;

    get_by_id(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sites WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

// --- Members ---

pub async fn list_members(
    pool: &SqlitePool,
    site_id: &str,
) -> Result<Vec<SiteMember>, sqlx::Error> {
    sqlx::query_as::<_, SiteMember>(
        "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
         FROM site_members sm
         JOIN users u ON sm.user_id = u.id
         WHERE sm.site_id = ?
         ORDER BY sm.role DESC, u.username",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await
}

pub async fn add_member(
    pool: &SqlitePool,
    id: &str,
    site_id: &str,
    user_id: &str,
    role: &str,
) -> Result<SiteMember, sqlx::Error> {
    sqlx::query("INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(site_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await?;

    sqlx::query_as::<_, SiteMember>(
        "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
         FROM site_members sm JOIN users u ON sm.user_id = u.id WHERE sm.id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn update_member_role(
    pool: &SqlitePool,
    site_id: &str,
    user_id: &str,
    role: &str,
) -> Result<Option<SiteMember>, sqlx::Error> {
    let result = sqlx::query("UPDATE site_members SET role = ? WHERE site_id = ? AND user_id = ?")
        .bind(role)
        .bind(site_id)
        .bind(user_id)
        .execute(pool)
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
    .fetch_one(pool)
    .await?;

    Ok(Some(member))
}

pub async fn remove_member(
    pool: &SqlitePool,
    site_id: &str,
    user_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM site_members WHERE site_id = ? AND user_id = ?")
        .bind(site_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
