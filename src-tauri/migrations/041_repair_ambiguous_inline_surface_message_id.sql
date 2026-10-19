-- Repair ambiguous bindings introduced by 040_backfill_inline_surface_message_id.
--
-- Migration 040 attached unbound chat_inline surfaces to the latest assistant
-- message in the conversation. That is unambiguous only when a single assistant
-- message exists. When multiple assistants exist and the surface was created
-- before the bound message, inventing ownership is unsafe — clear the binding
-- (fail-closed orphan) rather than keep a fabricated relationship.
--
-- Do not delete surfaces. Do not invent a different message_id.

UPDATE surfaces
SET message_id = NULL
WHERE placement = 'chat_inline'
  AND message_id IS NOT NULL
  AND conversation_id IS NOT NULL
  AND (
    SELECT COUNT(*)
    FROM messages m
    WHERE m.conversation_id = surfaces.conversation_id
      AND m.role = 'assistant'
  ) > 1
  AND EXISTS (
    SELECT 1
    FROM messages bound
    WHERE bound.id = surfaces.message_id
      AND bound.conversation_id = surfaces.conversation_id
      AND bound.role = 'assistant'
      AND datetime(surfaces.created_at) < datetime(bound.created_at)
  );
