-- Migration 033: Durable per-conversation catch-up cursors
--
-- Clients advance last_sequence only after successfully applying a catch-up
-- page. Monotonic: never moves backwards. Cascade-deletes with the conversation.

CREATE TABLE IF NOT EXISTS conversation_sync_cursors (
    conversation_id TEXT PRIMARY KEY NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    last_sequence INTEGER NOT NULL DEFAULT 0
        CHECK (last_sequence >= 0),
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversation_sync_cursors_updated
    ON conversation_sync_cursors(updated_at);
