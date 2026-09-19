-- Migration 028: Deep Runtime V2 Hardening
-- Adds application scope, manifest, routes, and data models to checkpoints.
-- Adds application_id and proposal_id to app_transactions for full provenance.
-- Adds branch provenance and checkpoint linking to chat_branches.

ALTER TABLE conversation_checkpoints ADD COLUMN application_id TEXT;
ALTER TABLE conversation_checkpoints ADD COLUMN manifest_json TEXT;
ALTER TABLE conversation_checkpoints ADD COLUMN routes_json TEXT;
ALTER TABLE conversation_checkpoints ADD COLUMN data_models_json TEXT;

ALTER TABLE app_transactions ADD COLUMN application_id TEXT;
ALTER TABLE app_transactions ADD COLUMN proposal_id TEXT;

ALTER TABLE chat_branches ADD COLUMN checkpoint_id TEXT REFERENCES conversation_checkpoints(id) ON DELETE SET NULL;
ALTER TABLE chat_branches ADD COLUMN provenance_json TEXT;

CREATE INDEX IF NOT EXISTS idx_transactions_app ON app_transactions(application_id);
CREATE INDEX IF NOT EXISTS idx_transactions_proposal ON app_transactions(proposal_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_app ON conversation_checkpoints(application_id);
CREATE INDEX IF NOT EXISTS idx_conversation_event_log_seq ON conversation_event_log(conversation_id, sequence);
