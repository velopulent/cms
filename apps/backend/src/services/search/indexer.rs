//! The single-consumer search indexer.
//!
//! Spawned once by the running server (it owns the Tantivy writer). It drains the
//! [`SearchQueue`] and applies the changes to the index. The server is the only
//! process that writes content (`vcms mcp stdio` is an HTTP proxy to it), and
//! every enqueue rings the queue's in-process `Notify`, so the indexer is purely
//! event-driven: one startup drain for rows a previous process left behind
//! (crash recovery), then sleep until notified.

use std::collections::HashSet;
use std::sync::Arc;

use super::SearchError;
use super::SearchService;
use super::queue::SearchQueue;
use crate::repository::Repository;

/// Max rows processed per drain iteration.
const BATCH_SIZE: i64 = 256;

/// Retries after a failed drain before giving up until the next enqueue.
const MAX_DRAIN_RETRIES: u32 = 5;

/// Run the indexer loop forever. Owns the writer-side `SearchService`.
pub async fn run(search: Arc<SearchService>, queue: Arc<SearchQueue>, repository: Arc<Repository>) {
    let notify = queue.notify_handle();

    // Startup drain: pick up rows committed before a crash/restart. `Notify`
    // stores a permit, so an enqueue racing this drain is never lost — it just
    // wakes the loop below for an empty (cheap) drain in the worst case.
    drain_with_retry(&search, &queue, &repository).await;

    loop {
        notify.notified().await;
        drain_with_retry(&search, &queue, &repository).await;
    }
}

/// Drain, retrying transient failures with exponential backoff so queued rows
/// don't sit stuck until the next enqueue. After [`MAX_DRAIN_RETRIES`] the rows
/// stay in the durable queue for the next notification or restart.
async fn drain_with_retry(search: &SearchService, queue: &SearchQueue, repository: &Repository) {
    let mut delay = std::time::Duration::from_millis(500);
    for attempt in 0..=MAX_DRAIN_RETRIES {
        match drain(search, queue, repository).await {
            Ok(()) => return,
            Err(e) if attempt == MAX_DRAIN_RETRIES => {
                tracing::error!("Search indexer drain failed after {MAX_DRAIN_RETRIES} retries: {e}");
            }
            Err(e) => {
                tracing::warn!("Search indexer drain failed (attempt {}): {e}; retrying", attempt + 1);
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(std::time::Duration::from_secs(10));
            }
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
