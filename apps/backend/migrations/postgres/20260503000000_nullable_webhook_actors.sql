-- Make created_by and triggered_by nullable in webhook tables

ALTER TABLE site_webhooks DROP CONSTRAINT IF EXISTS site_webhooks_created_by_fkey;
ALTER TABLE site_webhooks ALTER COLUMN created_by DROP NOT NULL;
ALTER TABLE site_webhooks ADD CONSTRAINT site_webhooks_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE site_webhook_deliveries DROP CONSTRAINT IF EXISTS site_webhook_deliveries_triggered_by_fkey;
ALTER TABLE site_webhook_deliveries ALTER COLUMN triggered_by DROP NOT NULL;
ALTER TABLE site_webhook_deliveries ADD CONSTRAINT site_webhook_deliveries_triggered_by_fkey
    FOREIGN KEY (triggered_by) REFERENCES users(id) ON DELETE SET NULL;
