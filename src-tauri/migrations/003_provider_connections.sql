-- Provider connection metadata (API keys live in OS keychain only)

CREATE TABLE IF NOT EXISTS provider_connections (
  id TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL,
  label TEXT NOT NULL,
  base_url TEXT,
  model_default TEXT,
  keyring_account TEXT NOT NULL UNIQUE,
  is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
  last_status TEXT,
  last_tested_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_connections_one_active
  ON provider_connections (is_active) WHERE is_active = 1;

CREATE INDEX IF NOT EXISTS idx_provider_connections_provider
  ON provider_connections (provider);
