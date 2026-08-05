-- Context ledger privacy: consumption tracking + branch index.
-- Legacy rows remain readable; None-project queries only match project_id IS NULL.

ALTER TABLE context_ledger_entries ADD COLUMN consumed_at TEXT;
ALTER TABLE context_ledger_entries ADD COLUMN consumed_by_message_id TEXT;

CREATE INDEX IF NOT EXISTS idx_context_ledger_branch
    ON context_ledger_entries(conversation_id, branch_id, created_at);
