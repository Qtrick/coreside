-- RC3.9: authoritative commit outbox, durable idempotency, turn journal.

CREATE TABLE IF NOT EXISTS commit_event_outbox (
  id TEXT PRIMARY KEY NOT NULL,
  transaction_id TEXT,
  conversation_id TEXT,
  turn_id TEXT,
  attempt_id TEXT,
  sequence INTEGER NOT NULL DEFAULT 0,
  effect_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL,
  delivered_at TEXT,
  delivery_attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_commit_outbox_pending
  ON commit_event_outbox(status, created_at);

CREATE TABLE IF NOT EXISTS apply_idempotency_outcomes (
  scope_key TEXT PRIMARY KEY NOT NULL,
  profile_id TEXT,
  conversation_id TEXT,
  turn_id TEXT,
  transaction_id TEXT,
  outcome TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_apply_idempotency_txn
  ON apply_idempotency_outcomes(transaction_id);

-- Durable atomic turn state machine (provider work stays outside SQLite txn).
CREATE TABLE IF NOT EXISTS turn_journal (
  id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  project_id TEXT,
  attempt_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  state TEXT NOT NULL,
  route TEXT,
  provider TEXT,
  model TEXT,
  reservation_id TEXT,
  provisional_text TEXT,
  operations_json TEXT,
  error_category TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  finalized_at TEXT,
  UNIQUE(idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_turn_journal_conversation
  ON turn_journal(conversation_id, created_at);

CREATE INDEX IF NOT EXISTS idx_turn_journal_state
  ON turn_journal(state, updated_at);

-- Conversation-scoped event cursor for direct + queued delivery.
CREATE TABLE IF NOT EXISTS conversation_event_log (
  id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  turn_id TEXT,
  attempt_id TEXT,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(conversation_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_conversation_event_log_resume
  ON conversation_event_log(conversation_id, sequence);
