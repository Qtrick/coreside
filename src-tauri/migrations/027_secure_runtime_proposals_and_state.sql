-- Migration 027: Secure Runtime Proposals, State Revisions, and Checkpoints
-- Fixes proposal error tracking, adds durable state revisions to surface_state and surface_versions,
-- and introduces durable turn-boundary checkpoints for exact branching and replay.

ALTER TABLE kernel_change_proposals ADD COLUMN error TEXT;

ALTER TABLE surface_state ADD COLUMN state_revision INTEGER NOT NULL DEFAULT 1;

ALTER TABLE surface_versions ADD COLUMN state_json TEXT;
ALTER TABLE surface_versions ADD COLUMN state_revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE surface_versions ADD COLUMN turn_id TEXT;
ALTER TABLE surface_versions ADD COLUMN message_id TEXT;

CREATE TABLE IF NOT EXISTS conversation_checkpoints (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL,
    turn_id TEXT,
    checkpoint_hash TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (conversation_id, message_id)
);

CREATE INDEX IF NOT EXISTS idx_checkpoints_conv_msg ON conversation_checkpoints(conversation_id, message_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_hash ON conversation_checkpoints(checkpoint_hash);
