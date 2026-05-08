CREATE TABLE IF NOT EXISTS site_webhooks (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    label VARCHAR(255) NOT NULL,
    url VARCHAR(2048) NOT NULL,
    headers_encrypted TEXT NOT NULL,
    created_by VARCHAR(36) NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS site_webhook_deliveries (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    webhook_id VARCHAR(36) NOT NULL,
    status VARCHAR(20) NOT NULL,
    status_code INTEGER,
    response_body TEXT,
    duration_ms INTEGER,
    triggered_by VARCHAR(36) NOT NULL,
    triggered_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (webhook_id) REFERENCES site_webhooks(id) ON DELETE CASCADE,
    FOREIGN KEY (triggered_by) REFERENCES users(id),
    CONSTRAINT chk_delivery_status CHECK (status IN ('success', 'failed'))
);

CREATE INDEX idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);