-- Bind legacy chat-inline surfaces that never received message_id so they remain
-- visible under an assistant bubble after fail-closed ownership filtering.
-- Prefer the latest assistant message in the same conversation.

UPDATE surfaces
SET message_id = (
  SELECT m.id
  FROM messages m
  WHERE m.conversation_id = surfaces.conversation_id
    AND m.role = 'assistant'
  ORDER BY m.created_at DESC
  LIMIT 1
)
WHERE placement = 'chat_inline'
  AND message_id IS NULL
  AND conversation_id IS NOT NULL
  AND EXISTS (
    SELECT 1
    FROM messages m2
    WHERE m2.conversation_id = surfaces.conversation_id
      AND m2.role = 'assistant'
  );
