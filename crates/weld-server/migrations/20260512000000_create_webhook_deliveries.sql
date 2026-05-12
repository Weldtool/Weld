CREATE TABLE github_webhook_deliveries (
    delivery_id  TEXT        PRIMARY KEY,
    event        TEXT        NOT NULL,
    received_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    error        TEXT
);
