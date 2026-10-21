/**
 * Agent presence projection for chat UI (dots / status).
 *
 * Authoritative source: Action Log lifecycle statuses on the live turn.
 * Animations must reflect real busy/awaiting/failed state — never invent steps.
 */

import {
  actionTimelineSummary,
  isTerminalActionStatus,
  normalizeActionStatus,
  type AgentActionEntry,
  type ActionEventStatus,
} from "@/lib/action-log";

export type AgentPresencePhase =
  | "idle"
  | "busy"
  | "awaiting"
  | "failed"
  | "cancelled"
  | "done";

export type AgentPresence = {
  phase: AgentPresencePhase;
  tone: "neutral" | "active" | "success" | "error";
  /** Short consumer label for aria-live. */
  label: string;
  busy: boolean;
};

export type AgentPresenceInput = {
  actions: AgentActionEntry[];
  streaming: boolean;
  sending: boolean;
  error?: string | null;
};

function dominantStatus(actions: AgentActionEntry[]): ActionEventStatus | null {
  if (actions.length === 0) return null;
  const statuses = actions.map((a) => normalizeActionStatus(a.status));
  if (statuses.some((s) => s === "running" || s === "pending")) return "running";
  if (statuses.some((s) => s === "awaitingApproval")) return "awaitingApproval";
  if (statuses.some((s) => s === "failed" || s === "blocked")) return "failed";
  if (statuses.every((s) => s === "cancelled")) return "cancelled";
  if (statuses.every((s) => isTerminalActionStatus(s))) return "completed";
  return statuses[statuses.length - 1] ?? null;
}

/** Derive presence for Grok/OpenAI-style indicators from real turn state. */
export function deriveAgentPresence(input: AgentPresenceInput): AgentPresence {
  if (input.error?.trim()) {
    return {
      phase: "failed",
      tone: "error",
      label: "Something went wrong",
      busy: false,
    };
  }

  const summary = actionTimelineSummary(input.actions);
  const status = dominantStatus(input.actions);
  const liveWork = input.sending || input.streaming;

  if (
    status === "awaitingApproval" ||
    (summary.tone === "neutral" && summary.title.toLowerCase().includes("review"))
  ) {
    return {
      phase: "awaiting",
      tone: "neutral",
      label: summary.title,
      busy: false,
    };
  }

  if (status === "failed" || status === "blocked") {
    return {
      phase: "failed",
      tone: "error",
      label: summary.title,
      busy: false,
    };
  }

  if (status === "cancelled") {
    return {
      phase: "cancelled",
      tone: "neutral",
      label: "Stopped",
      busy: false,
    };
  }

  if (liveWork || status === "running" || status === "pending") {
    return {
      phase: "busy",
      tone: "active",
      label: summary.title === "Done" ? "Working…" : summary.title,
      busy: true,
    };
  }

  if (status === "completed" || status === "succeeded") {
    return {
      phase: "done",
      tone: "success",
      label: summary.title,
      busy: false,
    };
  }

  return {
    phase: "idle",
    tone: "neutral",
    label: "",
    busy: false,
  };
}
