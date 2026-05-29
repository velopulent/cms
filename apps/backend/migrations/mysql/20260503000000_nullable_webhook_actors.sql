-- Make created_by and triggered_by nullable in webhook tables

ALTER TABLE site_webhooks DROP FOREIGN KEY site_webhooks_ibfk_2;
ALTER TABLE site_webhooks MODIFY COLUMN created_by VARCHAR(36) NULL;
ALTER TABLE site_webhooks ADD CONSTRAINT site_webhooks_ibfk_2
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE site_webhook_deliveries DROP FOREIGN KEY site_webhook_deliveries_ibfk_2;
ALTER TABLE site_webhook_deliveries MODIFY COLUMN triggered_by VARCHAR(36) NULL;
ALTER TABLE site_webhook_deliveries ADD CONSTRAINT site_webhook_deliveries_ibfk_2
    FOREIGN KEY (triggered_by) REFERENCES users(id) ON DELETE SET NULL;
