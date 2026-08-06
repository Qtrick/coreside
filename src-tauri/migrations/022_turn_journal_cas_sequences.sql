-- RC3.10: turn journal idempotency scoping + atomic conversation event sequences.

-- Scope idempotency to conversation (global unique key was too coarse and could
-- collide across chats). Keep a unique index on (conversation_id, idempotency_key).
CREATE TABLE IF NOT EXISTS turn_journal_rc310 (
  id TEXT PRIMARY KEY,
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
  UNIQUE(conversation_id, idempotency_key)
);

INSERT OR IGNORE INTO turn_journal_rc310 (
  id, conversation_id, project_id, attempt_id, idempotency_key, state,
  route, provider, model, reservation_id, provisional_text, operations_json,
  error_category, error_message, created_at, updated_at, finalized_at
)
SELECT
  id, conversation_id, project_id, attempt_id, idempotency_key, state,
  route, provider, model, reservation_id, provisional_text, operations_json,
  error_category, error_message, created_at, updated_at, finalized_at
FROM turn_journal;

DROP TABLE turn_journal;
ALTER TABLE turn_journal_rc310 RENAME TO turn_journal;

CREATE INDEX IF NOT EXISTS idx_turn_journal_conversation
  ON turn_journal(conversation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_turn_journal_state
  ON turn_journal(state, updated_at);

-- Per-conversation monotonic sequence counter (replaces MAX(sequence)+1 races).
CREATE TABLE IF NOT EXISTS conversation_event_sequences (
  conversation_id TEXT PRIMARY KEY,
  next_sequence INTEGER NOT NULL DEFAULT 1
);

INSERT OR IGNORE INTO conversation_event_sequences (conversation_id, next_sequence)
SELECT conversation_id, COALESCE(MAX(sequence), 0) + 1
FROM conversation_event_log
GROUP BY conversation_id;
