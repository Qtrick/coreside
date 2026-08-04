import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";

export type AgentTurnEvent =
  | { kind: "action"; conversationId: string; label: string }
  | {
      kind: "text";
      conversationId: string;
      /**
       * Cumulative checkpoint. Omitted on ordinary delta-primary events;
       * present every 32 sequences, on first event, and on non-prefix replaces.
       */
      text?: string | null;
      turnId?: string | null;
      sequence?: number | null;
      /** New chunk since the previous event; preferred ordinary carrier. */
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
      kind: "previewSurface";
      conversationId: string;
      turnId: string;
      toolId?: string | null;
      surfaceId: string;
      applicationId?: string | null;
      definitionJson: unknown;
      stateJson: unknown;
      revision: number;
      sequence: number;
    }
  | {
      kind: "sync";
      conversationId?: string | null;
      surfaceIds: string[];
      toolIds?: string[];
      applicationId?: string | null;
      revision?: number | null;
      syncKind: string;
    }
  | {
      kind: "conflict";
      conversationId?: string | null;
      message: string;
      conflicts: string[];
    };

/** Active UI scope used to filter process-wide Sync / Conflict events. */
export type AgentTurnSyncMatchContext = {
  activeConversationId: string | null;
  activeToolId: string | null;
  /** Optional explicit surface ids (defaults derived from activeToolId). */
  activeSurfaceIds?: string[];
};

/**
 * Whether a Sync event should reload surfaces in this window.
 *
 * - conversationId set and different from active → skip (Application A must not
 *   reload Application B / chat B).
 * - When surfaceIds / toolIds are present, require overlap with the active tool
 *   or active surfaces (tool windows match by tool even without an active chat).
 * - Unscoped Sync (no conversation / surface / tool target) is refused.
 */
export function shouldApplyAgentTurnSync(
  event: {
    conversationId?: string | null;
    surfaceIds?: string[];
    toolIds?: string[];
    applicationId?: string | null;
  },
  ctx: AgentTurnSyncMatchContext,
): boolean {
  const eventConv = (event.conversationId ?? "").trim();
  const activeConv = (ctx.activeConversationId ?? "").trim();
  const activeTool = (ctx.activeToolId ?? "").trim();
  const surfaceIds = (event.surfaceIds ?? []).map((s) => s.trim()).filter(Boolean);
  const toolIds = (event.toolIds ?? []).map((s) => s.trim()).filter(Boolean);
  const applicationId = (event.applicationId ?? "").trim();

  if (eventConv && activeConv && eventConv !== activeConv) {
    return false;
  }

  const activeSurfaces = new Set(
    (ctx.activeSurfaceIds ?? [])
      .map((s) => s.trim())
      .filter(Boolean),
  );
  if (activeTool) {
    activeSurfaces.add(activeTool);
    activeSurfaces.add(`surf-${activeTool}`);
  }

  const hasTargeting = surfaceIds.length > 0 || toolIds.length > 0 || Boolean(applicationId);
  if (hasTargeting) {
    if (activeTool && toolIds.includes(activeTool)) {
      return true;
    }
    if (activeTool && applicationId && applicationId === activeTool) {
      return true;
    }
    if (surfaceIds.some((id) => activeSurfaces.has(id))) {
      return true;
    }
    // Targeted sync for another tool/surface — do not reload this window.
    // Chat with matching conversation but no open tool: reload is a no-op today;
    // still accept so future surface refresh can run for the matching chat.
    if (!activeTool && eventConv && eventConv === activeConv) {
      return true;
    }
    return false;
  }

  if (eventConv) {
    return eventConv === activeConv;
  }

  // Unscoped Sync must not reload arbitrary windows.
  return false;
}

/**
 * Conflict banners are conversation-scoped. Empty ids never match.
 */
export function shouldShowAppConflict(
  conflict: { conversationId?: string | null },
  activeConversationId: string | null,
): boolean {
  const eventConv = (conflict.conversationId ?? "").trim();
  const activeConv = (activeConversationId ?? "").trim();
  if (!eventConv || !activeConv) return false;
  return eventConv === activeConv;
}

const agentTurnHandlers = new Set<(event: AgentTurnEvent) => void>();
let agentTurnUnlisten: (() => void) | null = null;

/**
 * Subscribe to process-wide `agent-turn` events. Returns an unsubscribe fn.
 *
 * The global bus must not carry private assistant text. Interactive
 * `send_message` streams deliver text/action/error/operation on a Tauri
 * Channel (authoritative). Sync/Conflict prefer `subscribeConversationSync`;
 * this listener remains as defense-in-depth for residual global emits when
 * no scoped subscribers delivered (see docs/SCOPED_TURN_STREAMING.md).
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

/** Defense-in-depth filter for conversation-scoped queue Channels. */
export function isQueueEventForConversation(
  event: QueueChangedEvent,
  conversationId: string,
): boolean {
  // Empty ids must never match — avoids refreshing/leaking across an unset chat.
  if (!conversationId || !event.conversationId) return false;
  return event.conversationId === conversationId;
}

/**
 * Subscribe to conversation-scoped queue mutations via Tauri Channel.
 * Does not use the process-wide event bus.
 * Keep the returned unlisten (or Channel) alive while the UI needs events.
 */
export async function subscribeConversationQueue(
  conversationId: string,
  handler: (event: QueueChangedEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime() || !conversationId) return () => undefined;
  const { Channel, invoke } = await import("@tauri-apps/api/core");
  let alive = true;
  const channel = new Channel<QueueChangedEvent>();
  channel.onmessage = (event) => {
    if (!alive) return;
    if (!isQueueEventForConversation(event, conversationId)) return;
    handler(event);
  };
  await invoke("subscribe_conversation_queue", {
    conversationId,
    onEvent: channel,
  });
  return () => {
    alive = false;
    channel.onmessage = () => undefined;
  };
}

/** Defense-in-depth filter for conversation-scoped Sync Channels. */
export function isSyncEventForConversation(
  event: AgentTurnEvent,
  conversationId: string,
): boolean {
  if (!conversationId) return false;
  if (event.kind !== "sync" && event.kind !== "conflict") return false;
  const eventConv = (event.conversationId ?? "").trim();
  if (!eventConv) return false;
  return eventConv === conversationId;
}

/**
 * Subscribe to conversation-scoped Sync/Conflict via Tauri Channel.
 * Primary path for surface reload + conflict banners (replaces global agent-turn
 * when Rust has at least one successful scoped delivery).
 */
export async function subscribeConversationSync(
  conversationId: string,
  handler: (event: AgentTurnEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime() || !conversationId) return () => undefined;
  const { Channel, invoke } = await import("@tauri-apps/api/core");
  let alive = true;
  const channel = new Channel<AgentTurnEvent>();
  channel.onmessage = (event) => {
    if (!alive) return;
    if (!isSyncEventForConversation(event, conversationId)) return;
    handler(event);
  };
  await invoke("subscribe_conversation_sync", {
    conversationId,
    onEvent: channel,
  });
  return () => {
    alive = false;
    channel.onmessage = () => undefined;
  };
}
