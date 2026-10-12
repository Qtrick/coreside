-- Persist project scope on change proposals so approval apply cannot skip
-- cross-project mutation guards (create_transaction previously used project_id=None).

ALTER TABLE kernel_change_proposals ADD COLUMN project_id TEXT;

UPDATE kernel_change_proposals
SET project_id = (
    SELECT conversations.project_id
    FROM conversations
    WHERE conversations.id = kernel_change_proposals.conversation_id
)
WHERE project_id IS NULL
  AND conversation_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_proposals_project ON kernel_change_proposals(project_id);
