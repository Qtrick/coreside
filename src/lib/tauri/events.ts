import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";

export type AgentTurnEvent =
  | { kind: "action"; conversationId: string; label: string }
  | {
      kind: "text";
      conversationId: string;
      /** Cumulative assistant text checkpoint (reconnect / catch-up). */
      text: string;
      turnId?: string | null;
      sequence?: number | null;
      /** New chunk since the previous checkpoint; `text` remains cumulative. */
      delta?: string | null;
    }
  | { kind: "error"; conversationId: string; message: string }
  | {
      kind: "operation";
      conversationId: string;
      operationId: string;
      status: string;
    }
  | {
      kind: "sync";
      conversationId?: string | null;
      surfaceIds: string[];
      revision?: number | null;
      syncKind: string;
    }
  | {
      kind: "conflict";
      conversationId?: string | null;
      message: string;
      conflicts: string[];
    };

const agentTurnHandlers = new Set<(event: AgentTurnEvent) => void>();
let agentTurnUnlisten: (() => void) | null = null;

/**
 * Subscribe to process-wide `agent-turn` events. Returns an unsubscribe fn.
 *
 * The global bus must not carry private assistant text. Interactive
 * `send_message` streams deliver text/action/error/operation on a Tauri
 * Channel (authoritative). This listener remains for Sync/Conflict multi-window
 * refresh — and as a temporary degradation path for queue-drain Action/Error/
 * Operation when no Channel is present.
 *
 * Tool windows must ignore text/action/error/operation (eavesdropping denial).
 * Client-side filtering is defense in depth, not authorization.
 */
export async function listenAgentTurn(
  handler: (event: AgentTurnEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  agentTurnHandlers.add(handler);
  if (!agentTurnUnlisten) {
    agentTurnUnlisten = await listen<AgentTurnEvent>("agent-turn", (event) => {
      for (const h of agentTurnHandlers) {
        h(event.payload);
      }
    });
  }
  return () => {
    agentTurnHandlers.delete(handler);
    if (agentTurnHandlers.size === 0 && agentTurnUnlisten) {
      agentTurnUnlisten();
      agentTurnUnlisten = null;
    }
  };
}

/**
 * Approval / grant state changed somewhere in the trusted core. Every window
 * subscribes so a decision, revocation, or away-parked request is reflected
 * everywhere without polling.
 */
export async function listenApprovalsChanged(
  handler: () => void,
): Promise<() => void> {
  if (!isTauriRuntime()) return () => undefined;
  return listen("runtime-approvals-changed", () => handler());
}

/** Queue mutation kinds emitted by Rust after SQLite authoritative writes. */
export type QueueChangeKind =
  | "itemAdded"
  | "itemActivated"
  | "itemCancelled"
  | "itemCompleted"
  | "queueSnapshot";

export type QueueChangedEvent = {
  kind: QueueChangeKind;
  conversationId: string;
  itemId?: string | null;
};

/** Defense-in-depth filter: queue bus is still process-wide (P1 to scope). */
export function isQueueEventForConversation(
  event: QueueChangedEvent,
  conversationId: string,
): boolean {
  // Empty ids must never match — avoids refreshing/leaking across an unset chat.
  if (!conversationId || !event.conversationId) return false;
  return event.conversationId === conversationId;
}

/**
 * Subscribe to `agent-queue-changed`. Payload includes conversationId; callers
 * must filter. Not conversation-scoped Channel delivery yet.
 */
export async function listenQueueChanged(
  handler: (event: QueueChangedEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<QueueChangedEvent>("agent-queue-changed", (event) => {
    handler(event.payload);
  });
}
