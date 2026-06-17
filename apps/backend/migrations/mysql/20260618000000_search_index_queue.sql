-- Cross-process search index queue. See the sqlite migration for the rationale.
-- `id` is a UUIDv7 (time-ordered); the single server process drains this queue
-- and applies changes to the Tantivy index.

-- NOTE: Foreign keys on entry_id/site_id are intentionally omitted so the queue
-- survives entry/site deletion — stale rows are harmless and cleaned up on drain.
CREATE TABLE IF NOT EXISTS search_index_queue (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    entry_id VARCHAR(36) NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    op VARCHAR(16) NOT NULL CHECK (op IN ('index', 'delete')),
    enqueued_at VARCHAR(40) NOT NULL DEFAULT (NOW()),
    INDEX idx_search_index_queue_entry (entry_id),
    INDEX idx_search_index_queue_enqueued (enqueued_at)
);
