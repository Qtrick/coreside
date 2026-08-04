-- Redacted turn timeline for read-only replay (RC3.4)

CREATE TABLE IF NOT EXISTS turn_timeline_events (
  id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  turn_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN (
    'user_request',
    'provider_start',
    'text_checkpoint',
    'operation_received',
    'operation_accepted',
    'operation_rejected',
    'preview_update',
    'approval',
    'commit',
    'failure',
    'cancellation',
    'completion'
  )),
  redacted_payload_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_turn_timeline_conv_turn
  ON turn_timeline_events(conversation_id, turn_id, sequence);

CREATE INDEX IF NOT EXISTS idx_turn_timeline_conversation
  ON turn_timeline_events(conversation_id, created_at);
