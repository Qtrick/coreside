import type { AppView } from "./types";

/**
 * True when the app is already showing this chat in chat view
 * (not settings, automations, projects, media, or project detail).
 * Requires both `view.conversationId` and `activeConversationId` to match
 * so a desynced null view id cannot no-op navigation.
 */
export function isCurrentChat(
  view: AppView,
  activeConversationId: string | null,
  id: string,
): boolean {
  if (view.kind !== "chat") return false;
  return view.conversationId === id && activeConversationId === id;
}

/** Alias used by navigation — same predicate as {@link isCurrentChat}. */
export function shouldNavigateToChatNoOp(
  view: AppView,
  activeConversationId: string | null,
  id: string,
): boolean {
  return isCurrentChat(view, activeConversationId, id);
}

export function messagesBelongToChat(
  messages: Array<{ conversationId: string }>,
  conversationId: string,
): boolean {
  if (messages.length === 0) return true;
  return messages.every((m) => m.conversationId === conversationId);
}

export function shouldLoadChatMessages(
  activeConversationId: string | null,
  targetId: string,
  messages: Array<{ conversationId: string }>,
): boolean {
  if (activeConversationId !== targetId) return true;
  return !messagesBelongToChat(messages, targetId);
}
