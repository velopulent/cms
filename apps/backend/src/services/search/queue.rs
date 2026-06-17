//! Durable, cross-process queue of pending search-index updates
//! (`search_index_queue` table).
//!
//! Producers (any process that writes entry content) call [`SearchQueue::enqueue`]
//! after a write. The single writer-owning server drains the queue via the
//! [`indexer`](super::indexer). Because the queue lives in the database it works
//! across processes (e.g. a separate `cms mcp stdio`) and survives restarts.

use std::sync::Arc;

use sqlx::FromRow;
use tokio::sync::Notify;

use super::SearchError;
use crate::database::backend::DatabaseBackend;
use crate::database::pool::DbPool;

/// Advisory op labels stored on the queue row (the consumer re-derives the real
/// action from the database, so these are for observability only).
pub const OP_UPSERT: &str = "upsert";
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
    /// Wakes the in-process indexer immediately on a local enqueue (cross-process
    /// enqueues are picked up by the indexer's poll fallback instead).
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
                sqlx::query(&sql)
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
                sqlx::query(&sql)
                    .bind(&id)
                    .bind(entry_id)
                    .bind(site_id)
                    .bind(op)
                    .bind(&now)
                    .execute(p)
                    .await
                    .map_err(dberr)?;
            }
            DbPool::MySql(p) => {
                sqlx::query(&sql)
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
            DbPool::Sqlite(p) => sqlx::query_as::<_, QueueRow>(&sql)
                .bind(limit)
                .fetch_all(p)
                .await
                .map_err(dberr)?,
            DbPool::Postgres(p) => sqlx::query_as::<_, QueueRow>(&sql)
                .bind(limit)
                .fetch_all(p)
                .await
                .map_err(dberr)?,
            DbPool::MySql(p) => sqlx::query_as::<_, QueueRow>(&sql)
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
                let mut query = sqlx::query(&sql);
                for id in ids {
                    query = query.bind(id);
                }
                query.execute(p).await.map_err(dberr)?;
            }
            DbPool::Postgres(p) => {
                let mut query = sqlx::query(&sql);
                for id in ids {
                    query = query.bind(id);
                }
                query.execute(p).await.map_err(dberr)?;
            }
            DbPool::MySql(p) => {
                let mut query = sqlx::query(&sql);
                for id in ids {
                    query = query.bind(id);
                }
                query.execute(p).await.map_err(dberr)?;
            }
        }
        Ok(())
    }
}
