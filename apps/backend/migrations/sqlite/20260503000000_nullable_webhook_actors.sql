-- Make created_by and triggered_by nullable in webhook tables
-- Previously these referenced users(id) with NOT NULL, but API key operations
-- don't have a user context, causing FK violations.

-- site_webhooks: recreate table without FK/NOT NULL on created_by
ALTER TABLE site_webhooks RENAME TO site_webhooks_old;

CREATE TABLE site_webhooks (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    url TEXT NOT NULL,
    headers_encrypted TEXT NOT NULL,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO site_webhooks (id, site_id, label, url, headers_encrypted, created_by, created_at, updated_at)
    SELECT id, site_id, label, url, headers_encrypted, created_by, created_at, updated_at
    FROM site_webhooks_old;

DROP TABLE site_webhooks_old;

-- site_webhook_deliveries: recreate table without FK/NOT NULL on triggered_by
ALTER TABLE site_webhook_deliveries RENAME TO site_webhook_deliveries_old;

CREATE TABLE site_webhook_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    webhook_id TEXT NOT NULL REFERENCES site_webhooks(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('success', 'failed')),
    status_code INTEGER,
    response_body TEXT,
    duration_ms INTEGER,
    triggered_by TEXT,
    triggered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO site_webhook_deliveries (id, webhook_id, status, status_code, response_body, duration_ms, triggered_by, triggered_at)
    SELECT id, webhook_id, status, status_code, response_body, duration_ms, triggered_by, triggered_at
    FROM site_webhook_deliveries_old;

DROP TABLE site_webhook_deliveries_old;

CREATE INDEX IF NOT EXISTS idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX IF NOT EXISTS idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);
