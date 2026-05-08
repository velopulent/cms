CREATE TABLE IF NOT EXISTS site_webhooks (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    url TEXT NOT NULL,
    headers_encrypted TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS site_webhook_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    webhook_id TEXT NOT NULL REFERENCES site_webhooks(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('success', 'failed')),
    status_code INTEGER,
    response_body TEXT,
    duration_ms INTEGER,
    triggered_by TEXT NOT NULL REFERENCES users(id),
    triggered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX IF NOT EXISTS idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);