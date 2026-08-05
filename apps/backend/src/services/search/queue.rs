//! Durable queue of pending search-index updates (`search_index_queue` table).
//!
//! Content writes call [`SearchQueue::enqueue`] after committing; the server's
//! [`indexer`](super::indexer) drains the queue. All producers run in the server
//! process (`vcms mcp stdio` is an HTTP proxy), so every enqueue also rings the
//! in-process `Notify` and the indexer needs no polling. The queue lives in the
//! database purely for durability: rows enqueued before a crash are drained on
//! the next startup.

use std::sync::Arc;

use sqlx::FromRow;
use tokio::sync::Notify;

use super::SearchError;
use crate::database::backend::DatabaseBackend;
use crate::database::pool::DbPool;

/// Advisory op labels stored on the queue row (the consumer re-derives the real
/// action from the database, so these are for observability only).
pub const OP_INDEX: &str = "index";
pub const OP_DELETE: &str = "delete";

/// A pending queue entry. Only `id` (for deletion after processing) and
/// `entry_id` (to look up current state) are needed by the consumer.
#[derive(Debug, Clone, FromRow)]
pub struct QueueRow {
    pub id: String,
    pub entry_id: String,
}

/// Handle for enqueuing index updates and draining them.
pub struct SearchQueue {
    pool: DbPool,
    /// Wakes the indexer immediately on an enqueue (all producers are in-process).
    notify: Arc<Notify>,
}

fn dberr(e: sqlx::Error) -> SearchError {
    SearchError::Db(e.to_string())
}

/// Rewrite `?` placeholders to `$1..$n` for Postgres; leave as-is otherwise.
fn q(backend: DatabaseBackend, sql: &str) -> String {
    if backend != DatabaseBackend::Postgres {
        return sql.to_string();
    }
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = 1;
    for ch in sql.chars() {
        if ch == '?' {
            out.push('$');
            out.push_str(&n.to_string());
            n += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

impl SearchQueue {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            notify: Arc::new(Notify::new()),
        }
    }

    /// A handle the indexer waits on to react to local enqueues without polling.
    pub fn notify_handle(&self) -> Arc<Notify> {
        self.notify.clone()
    }

    /// Record a pending index update for `entry_id` and wake the local indexer.
    pub async fn enqueue(&self, entry_id: &str, site_id: &str, op: &str) -> Result<(), SearchError> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = crate::services::backup::now_iso();
        let sql = q(
            self.pool.backend(),
            "INSERT INTO search_index_queue (id, entry_id, site_id, op, enqueued_at) VALUES (?, ?, ?, ?, ?)",
        );
        match &self.pool {
            DbPool::Sqlite(p) => {
                sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(&id)
                    .bind(entry_id)
                    .bind(site_id)
                    .bind(op)
                    .bind(&now)
                    .execute(p)
                    .await
                    .map_err(dberr)?;
            }
            DbPool::Postgres(p) => {
                sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(&id)
                    .bind(entry_id)
                    .bind(site_id)
                    .bind(op)
                    .bind(&now)
                    .execute(p)
                    .await
                    .map_err(dberr)?;
            }
        }
        self.notify.notify_one();
        Ok(())
    }

    /// Read up to `limit` pending rows in chronological (UUIDv7) order.
    pub async fn dequeue_batch(&self, limit: i64) -> Result<Vec<QueueRow>, SearchError> {
        let sql = q(
            self.pool.backend(),
            "SELECT id, entry_id FROM search_index_queue ORDER BY id ASC LIMIT ?",
        );
        let rows = match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as::<_, QueueRow>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(limit)
                .fetch_all(p)
                .await
                .map_err(dberr)?,
            DbPool::Postgres(p) => sqlx::query_as::<_, QueueRow>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(limit)
                .fetch_all(p)
                .await
                .map_err(dberr)?,
        };
        Ok(rows)
    }

    /// Delete the processed rows by id.
    pub async fn delete_ids(&self, ids: &[String]) -> Result<(), SearchError> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; ids.len()].join(", ");
        let sql = q(
            self.pool.backend(),
            &format!("DELETE FROM search_index_queue WHERE id IN ({placeholders})"),
        );
        match &self.pool {
            DbPool::Sqlite(p) => {
                let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
                for id in ids {
                    query = query.bind(id);
                }
                query.execute(p).await.map_err(dberr)?;
            }
            DbPool::Postgres(p) => {
                let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
                for id in ids {
                    query = query.bind(id);
                }
                query.execute(p).await.map_err(dberr)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{OP_DELETE, OP_INDEX, SearchQueue};
    use crate::database::init_db;

    /// The `search_index_queue.op` column carries a DB `CHECK (op IN ('index',
    /// 'delete'))`. Producer constants must stay inside that set: a drift (e.g. an
    /// `"upsert"` value) makes every enqueue INSERT fail, and because enqueue is
    /// best-effort the failure is swallowed — silently dropping all index updates.
    /// This guards the constants against the constraint directly.
    #[tokio::test]
    async fn enqueue_accepts_known_ops_and_rejects_unknown() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db_path = dir.path().join("queue.db");
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let pool = init_db(&url).await.expect("migrated database");
        let queue = SearchQueue::new(pool);

        queue
            .enqueue("entry-1", "site-1", OP_INDEX)
            .await
            .expect("OP_INDEX must satisfy the op CHECK constraint");
        queue
            .enqueue("entry-1", "site-1", OP_DELETE)
            .await
            .expect("OP_DELETE must satisfy the op CHECK constraint");

        assert!(
            queue.enqueue("entry-1", "site-1", "upsert").await.is_err(),
            "an op outside the CHECK set must be rejected, not silently accepted"
        );
    }
}
