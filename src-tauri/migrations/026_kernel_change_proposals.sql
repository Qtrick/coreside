-- Migration 026: Persisted Kernel Change Proposals
-- Binds user approvals cryptographically and logically to an exact frozen operation set.
-- Prevents replay, payload tampering, and race conditions.

CREATE TABLE IF NOT EXISTS kernel_change_proposals (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    message_id TEXT,
    turn_id TEXT,
    application_id TEXT,
    targets_json TEXT NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL,
    impact_summary TEXT,
    risk TEXT NOT NULL DEFAULT 'medium',
    exact_operations_json TEXT NOT NULL,
    exact_operations_hash TEXT NOT NULL,
    base_revisions_json TEXT NOT NULL DEFAULT '{}',
    source_type TEXT NOT NULL DEFAULT 'agent',
    model TEXT,
    provider TEXT,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, approved, applied, discarded, expired, stale
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    decided_at TEXT,
    applied_at TEXT,
    discard_reason TEXT,
    transaction_id TEXT REFERENCES app_transactions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_proposals_conversation ON kernel_change_proposals(conversation_id);
CREATE INDEX IF NOT EXISTS idx_proposals_status ON kernel_change_proposals(status);
CREATE INDEX IF NOT EXISTS idx_proposals_hash ON kernel_change_proposals(exact_operations_hash);

-- Persist trusted source provenance on scheduled patches (prevents privilege escalation during flush)
ALTER TABLE patch_scheduler_items ADD COLUMN source_type TEXT NOT NULL DEFAULT 'agent';

