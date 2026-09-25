-- Migration 029: Explicit turn_id binding on agent_request_queue for safe recovery
ALTER TABLE agent_request_queue ADD COLUMN turn_id TEXT REFERENCES turn_journal(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_agent_request_queue_turn ON agent_request_queue(turn_id);
