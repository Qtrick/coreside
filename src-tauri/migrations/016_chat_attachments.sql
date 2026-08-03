-- First-class chat attachment lifecycle (opaque IDs, states, no absolute paths).

CREATE TABLE IF NOT EXISTS chat_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT,
    message_id TEXT,
    draft_id TEXT,
    storage_key TEXT NOT NULL UNIQUE,
    original_filename TEXT NOT NULL,
    display_name TEXT NOT NULL,
    detected_mime TEXT NOT NULL,
    detected_format TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    state TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    backup_eligible INTEGER NOT NULL DEFAULT 0,
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_chat_attachments_conversation
    ON chat_attachments(conversation_id, state);
CREATE INDEX IF NOT EXISTS idx_chat_attachments_message
    ON chat_attachments(message_id);
CREATE INDEX IF NOT EXISTS idx_chat_attachments_draft
    ON chat_attachments(draft_id);
CREATE INDEX IF NOT EXISTS idx_chat_attachments_state_expires
    ON chat_attachments(state, expires_at);
CREATE INDEX IF NOT EXISTS idx_chat_attachments_hash
    ON chat_attachments(content_hash);
