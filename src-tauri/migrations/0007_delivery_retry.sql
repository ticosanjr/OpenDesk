-- Durable delivery retries survive an application restart.
ALTER TABLE integration_deliveries ADD COLUMN next_retry_at TEXT;
ALTER TABLE integration_deliveries ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 5;
ALTER TABLE integration_deliveries ADD COLUMN payload_json TEXT;
