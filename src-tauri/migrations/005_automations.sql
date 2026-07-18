-- Automations, run history, and workspace background presets.

CREATE TABLE IF NOT EXISTS automations (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL,
  owner_tool_id TEXT,
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  trigger_type TEXT NOT NULL,
  trigger_json TEXT NOT NULL,
  action_type TEXT NOT NULL,
  action_json TEXT NOT NULL,
  requires_ai INTEGER NOT NULL DEFAULT 0 CHECK (requires_ai IN (0, 1)),
  provider_connection_id TEXT,
  missed_run_policy TEXT NOT NULL DEFAULT 'run_once',
  next_run_at TEXT,
  last_run_at TEXT,
  last_status TEXT,
  consecutive_failures INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_automations_next_run
  ON automations (enabled, next_run_at);

CREATE TABLE IF NOT EXISTS automation_runs (
  id TEXT PRIMARY KEY NOT NULL,
  automation_id TEXT NOT NULL,
  scheduled_at TEXT,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  status TEXT NOT NULL,
  result_summary TEXT,
  error_category TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (automation_id) REFERENCES automations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_automation_runs_automation
  ON automation_runs (automation_id, created_at DESC);

CREATE TABLE IF NOT EXISTS workspace_backgrounds (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  definition_json TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_workspace_backgrounds_workspace
  ON workspace_backgrounds (workspace_id, sort_order);

CREATE TABLE IF NOT EXISTS export_history (
  id TEXT PRIMARY KEY NOT NULL,
  tool_id TEXT NOT NULL,
  format TEXT NOT NULL,
  filename TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
