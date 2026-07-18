-- Exa search usage ledger + research profile / budget settings.
-- Wallpaper templates stub (UI seed may follow in a later migration).

CREATE TABLE IF NOT EXISTS exa_usage_ledger (
  id TEXT PRIMARY KEY NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  month_key TEXT NOT NULL,
  request_id TEXT,
  search_mode TEXT NOT NULL,
  result_count INTEGER NOT NULL DEFAULT 0,
  actual_cost REAL,
  estimated_cost REAL,
  cache_hit INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL,
  conversation_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_exa_usage_month
  ON exa_usage_ledger (month_key, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_exa_usage_conversation
  ON exa_usage_ledger (conversation_id, created_at DESC);

INSERT OR IGNORE INTO settings (key, value) VALUES
  ('searchProfile', 'saver'),
  ('exaMonthlyBudgetUsd', ''),
  ('exaBudgetSoftPercent', '75'),
  ('exaBudgetCriticalPercent', '90'),
  ('exaBudgetHardPercent', '100');

CREATE TABLE IF NOT EXISTS wallpaper_templates (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  definition_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
