CREATE TABLE IF NOT EXISTS site_webhooks (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label VARCHAR(255) NOT NULL,
    url VARCHAR(2048) NOT NULL,
    headers_encrypted TEXT NOT NULL,
    created_by VARCHAR(36) NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS site_webhook_deliveries (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    webhook_id VARCHAR(36) NOT NULL REFERENCES site_webhooks(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL CHECK(status IN ('success', 'failed')),
    status_code INTEGER,
    response_body TEXT,
    duration_ms INTEGER,
    triggered_by VARCHAR(36) NOT NULL REFERENCES users(id),
    triggered_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX IF NOT EXISTS idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);