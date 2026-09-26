-- Migration 031: Surface event deliveries & declarative handlers
--
-- Provides durable, idempotent event delivery tracking for generated applications
-- and adds declarative event handler specifications to surface subscriptions.

ALTER TABLE surface_subscriptions ADD COLUMN handler_json TEXT NOT NULL DEFAULT '{}';

CREATE TABLE IF NOT EXISTS surface_event_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    event_id TEXT NOT NULL REFERENCES surface_events(id) ON DELETE CASCADE,
    subscription_id TEXT NOT NULL REFERENCES surface_subscriptions(id) ON DELETE CASCADE,
    surface_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    processed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_surface_event_deliveries_sub
    ON surface_event_deliveries(status, subscription_id);

CREATE INDEX IF NOT EXISTS idx_surface_event_deliveries_event
    ON surface_event_deliveries(event_id, subscription_id);

CREATE INDEX IF NOT EXISTS idx_surface_event_deliveries_surface
    ON surface_event_deliveries(surface_id, status);
