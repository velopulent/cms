-- Cross-process search index queue. See the sqlite migration for the rationale.
-- `id` is a UUIDv7 (time-ordered); the single server process drains this queue
-- and applies changes to the Tantivy index.

CREATE TABLE IF NOT EXISTS search_index_queue (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    entry_id VARCHAR(36) NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    op VARCHAR(16) NOT NULL,
    enqueued_at VARCHAR(40) NOT NULL,
    INDEX idx_search_index_queue_entry (entry_id)
);
