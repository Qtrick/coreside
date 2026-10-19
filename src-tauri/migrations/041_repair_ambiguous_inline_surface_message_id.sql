-- Repair ambiguous bindings introduced by 040_backfill_inline_surface_message_id.
--
-- Migration 040 attached unbound chat_inline surfaces to the latest assistant
-- message in the conversation. That invents ownership when an older assistant
-- message sits between the surface's creation time and the bound message —
-- clear those bindings (fail-closed orphan).
--
-- Legitimate runtime bindings create the surface seconds before the assistant
-- message that owns them. Do NOT clear those: require a different assistant
-- message with created_at strictly between the surface and the bound message.
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
      AND EXISTS (
        SELECT 1
        FROM messages intervening
        WHERE intervening.conversation_id = surfaces.conversation_id
          AND intervening.role = 'assistant'
          AND intervening.id != bound.id
          AND datetime(intervening.created_at) > datetime(surfaces.created_at)
          AND datetime(intervening.created_at) < datetime(bound.created_at)
      )
  );
