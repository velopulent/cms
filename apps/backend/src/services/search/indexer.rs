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

/// Run the indexer loop forever. Owns the writer-side `SearchService`.
pub async fn run(search: Arc<SearchService>, queue: Arc<SearchQueue>, repository: Arc<Repository>) {
    let notify = queue.notify_handle();

    // Startup drain: pick up rows committed before a crash/restart. `Notify`
    // stores a permit, so an enqueue racing this drain is never lost — it just
    // wakes the loop below for an empty (cheap) drain in the worst case.
    if let Err(e) = drain(&search, &queue, &repository).await {
        tracing::error!("Search indexer startup drain failed: {}", e);
    }

    loop {
        notify.notified().await;
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
