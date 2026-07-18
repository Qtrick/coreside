-- Action Log Base Setting (default off) + persisted sanitized events.

INSERT OR IGNORE INTO settings (key, value) VALUES ('actionLogEnabled', 'false');

CREATE TABLE IF NOT EXISTS action_events (
  id TEXT PRIMARY KEY NOT NULL,
  request_id TEXT NOT NULL,
  conversation_id TEXT,
  event_type TEXT NOT NULL,
  label TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'completed',
  metadata_json TEXT,
  sequence INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_action_events_request
  ON action_events (request_id, sequence);

CREATE INDEX IF NOT EXISTS idx_action_events_conversation
  ON action_events (conversation_id, created_at);
