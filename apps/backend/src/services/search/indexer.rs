//! The single-consumer search indexer.
//!
//! Spawned once by the running server (it owns the Tantivy writer). It drains the
//! [`SearchQueue`] — populated by content writes from *any* process — and applies
//! the changes to the index. It wakes immediately on a local enqueue (via the
//! queue's `Notify`) and also polls on an interval to pick up enqueues from other
//! processes (e.g. `vcms mcp stdio`).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use super::SearchError;
use super::SearchService;
use super::queue::SearchQueue;
use crate::repository::Repository;

/// Fallback poll interval for enqueues made by other processes (which can't ring
/// this process's in-memory `Notify`).
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Max rows processed per drain iteration.
const BATCH_SIZE: i64 = 256;

/// Run the indexer loop forever. Owns the writer-side `SearchService`.
pub async fn run(search: Arc<SearchService>, queue: Arc<SearchQueue>, repository: Arc<Repository>) {
    let notify = queue.notify_handle();
    loop {
        tokio::select! {
            _ = notify.notified() => {}
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }
        if let Err(e) = drain(&search, &queue, &repository).await {
            tracing::error!("Search indexer drain failed: {}", e);
        }
    }
}

/// Drain the queue in batches until empty. Each entry is reindexed once at its
/// current database state: present in the DB ⇒ upsert the document, absent ⇒ delete
/// it. This makes the queue's `op` advisory and naturally heals create-then-delete
/// sequences.
async fn drain(search: &SearchService, queue: &SearchQueue, repository: &Repository) -> Result<(), SearchError> {
    loop {
        let batch = queue.dequeue_batch(BATCH_SIZE).await?;
        if batch.is_empty() {
            break;
        }

        // Collapse duplicate entry_ids, keeping each entry's latest occurrence.
        let mut seen = HashSet::new();
        let mut unique: Vec<&str> = Vec::new();
        for row in batch.iter().rev() {
            if seen.insert(row.entry_id.as_str()) {
                unique.push(row.entry_id.as_str());
            }
        }

        for entry_id in unique {
            match repository
                .entry
                .get_by_id_any_site(entry_id)
                .await
                .map_err(|e| SearchError::Repository(e.to_string()))?
            {
                Some(entry) => search.index_doc(&entry)?,
                None => search.delete_doc(entry_id)?,
            }
        }
        search.commit()?;

        let ids: Vec<String> = batch.iter().map(|r| r.id.clone()).collect();
        queue.delete_ids(&ids).await?;

        if (batch.len() as i64) < BATCH_SIZE {
            break;
        }
    }
    Ok(())
}
