-- Cross-process search index queue. See the sqlite migration for the rationale.
-- `id` is a UUIDv7 (time-ordered); the single server process drains this queue
-- and applies changes to the Tantivy index.

-- NOTE: Foreign keys on entry_id/site_id are intentionally omitted so the queue
-- survives entry/site deletion — stale rows are harmless and cleaned up on drain.
CREATE TABLE IF NOT EXISTS search_index_queue (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL,
    site_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK(op IN ('index', 'delete')),
    enqueued_at TEXT NOT NULL DEFAULT (NOW()::text)
);

CREATE INDEX IF NOT EXISTS idx_search_index_queue_entry ON search_index_queue(entry_id);
CREATE INDEX IF NOT EXISTS idx_search_index_queue_enqueued ON search_index_queue(enqueued_at);
