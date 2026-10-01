-- Freeze conversation/project identity on runtime_approvals so Approve-once
-- replay can reconstruct the same call_hash (which binds conversationId).
-- Personal-tool surfaces often have NULL conversation_id while the original
-- invoke used the active chat — surface lookup alone is not enough.
ALTER TABLE runtime_approvals ADD COLUMN conversation_id TEXT;
ALTER TABLE runtime_approvals ADD COLUMN project_id TEXT;
