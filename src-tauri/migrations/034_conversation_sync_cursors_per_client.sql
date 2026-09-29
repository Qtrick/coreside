-- Migration 034: Per-client (window) catch-up cursors
--
-- Catch-up watermarks must be scoped to the consuming webview/window, not shared
-- across the conversation. A shared conversation-scoped cursor lets one window
-- advance past events another window has not applied yet.
--
-- Existing rows (migration 033) are remapped to client_id = 'main'.

CREATE TABLE IF NOT EXISTS conversation_sync_cursors_v2 (
    conversation_id TEXT NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    last_sequence INTEGER NOT NULL DEFAULT 0
        CHECK (last_sequence >= 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (conversation_id, client_id),
    CHECK (length(client_id) > 0 AND length(client_id) <= 256)
);

INSERT OR IGNORE INTO conversation_sync_cursors_v2 (
    conversation_id, client_id, last_sequence, updated_at
)
SELECT conversation_id, 'main', last_sequence, updated_at
FROM conversation_sync_cursors;

DROP TABLE IF EXISTS conversation_sync_cursors;

ALTER TABLE conversation_sync_cursors_v2 RENAME TO conversation_sync_cursors;

CREATE INDEX IF NOT EXISTS idx_conversation_sync_cursors_updated
    ON conversation_sync_cursors(updated_at);
