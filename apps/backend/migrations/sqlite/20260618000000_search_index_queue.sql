-- Cross-process search index queue.
--
-- Any process that writes entry content (server, gRPC, MCP HTTP, or a separate
-- `cms mcp stdio` process) enqueues a row here. The single running server owns
-- the Tantivy IndexWriter and is the sole consumer: it drains this queue, applies
-- the changes to the index, and deletes the processed rows. This makes search
-- sync work across processes and survive restarts (the queue is durable), while
-- keeping the embedded single-writer model.
--
-- `id` is a UUIDv7 (time-ordered) so draining in `id` order is chronological.
-- `op` is advisory; the consumer re-derives the action by looking the entry up in
-- the database (present => upsert, absent => delete).

CREATE TABLE IF NOT EXISTS search_index_queue (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL,
    site_id TEXT NOT NULL,
    op TEXT NOT NULL,
    enqueued_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_search_index_queue_entry ON search_index_queue(entry_id);
