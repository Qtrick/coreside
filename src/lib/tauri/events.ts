import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";

export type AgentTurnEvent =
  | { kind: "action"; conversationId: string; label: string }
  | { kind: "text"; conversationId: string; text: string }
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

/** Subscribe to live agent-turn events. Returns an unsubscribe fn. */
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
