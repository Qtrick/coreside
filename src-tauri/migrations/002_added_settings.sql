-- Added Settings: tool-owned preferences (never under core.*)

CREATE TABLE IF NOT EXISTS added_settings (
    id TEXT PRIMARY KEY NOT NULL,
    owner_tool_id TEXT,
    label TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    setting_type TEXT NOT NULL DEFAULT 'string',
    default_value TEXT NOT NULL DEFAULT 'null',
    current_value TEXT NOT NULL DEFAULT 'null',
    constraints TEXT NOT NULL DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (owner_tool_id) REFERENCES tools(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_added_settings_owner
    ON added_settings(owner_tool_id);
