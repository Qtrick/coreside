/**
 * Conversation history / replay helpers (non-component exports).
 * Kept separate from ConversationHistory.tsx for React Fast Refresh.
 *
 * HARD RULE: Replay never reinvokes the provider, never re-applies
 * transactions, and never resubmits forms. Events are display-only.
 */

export type ReplayEvent = {
  id: string;
  kind: string;
  summary: string;
  createdAt: string;
  status: string;
  opsCount: number;
};

/** Base delay between auto-play steps at 1x (ms). */
export const REPLAY_STEP_MS = 800;

/** Align with Rust `redact_secrets` shapes (client-side defense-in-depth). */
const SENSITIVE_JSON_KEY =
  /^(authorization|api[_-]?key|access[_-]?token|refresh[_-]?token|secret|password|x-api-key|token)$/i;

/** Client-side defense-in-depth for diagnostic display. */
export function redactSecretsForDisplay(text: string): string {
  return text
    .replace(/\bAIza[0-9A-Za-z\-_]{20,}\b/g, "[REDACTED]")
    .replace(/\bsk-[A-Za-z0-9_-]{8,}\b/g, "[REDACTED]")
    .replace(/\bBearer\s+[A-Za-z0-9._\-+=/~]+/gi, "Bearer [REDACTED]")
    .replace(/(api[_-]?key[=:]\s*)([^\s&"']+)/gi, "$1[REDACTED]")
    .replace(/(key[=:]\s*)([A-Za-z0-9\-_]{16,})/gi, "$1[REDACTED]");
}

export function redactDiagnosticJson(value: unknown): string {
  const walk = (v: unknown): unknown => {
    if (typeof v === "string") return redactSecretsForDisplay(v);
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === "object") {
      const out: Record<string, unknown> = {};
      for (const [k, child] of Object.entries(v as Record<string, unknown>)) {
        out[k] = SENSITIVE_JSON_KEY.test(k) ? "[REDACTED]" : walk(child);
      }
      return out;
    }
    return v;
  };
  try {
    return redactSecretsForDisplay(JSON.stringify(walk(value), null, 2));
  } catch {
    return redactSecretsForDisplay(String(value));
  }
}

export type TransactionRow = {
  id: string;
  summary: string;
  status: string;
  createdAt: string;
  opsCount: number;
};

export type TimelineRow = {
  id: string;
  conversationId: string;
  turnId: string;
  sequence: number;
  kind: string;
  redactedPayload?: unknown;
  createdAt: string;
};

const TIMELINE_KIND_LABELS: Record<string, string> = {
  user_request: "User request",
  provider_start: "Provider started",
  text_checkpoint: "Text checkpoint",
  operation_received: "Operation received",
  operation_accepted: "Operation accepted",
  operation_rejected: "Operation rejected",
  preview_update: "Preview update",
  approval: "Approval",
  commit: "Commit",
  failure: "Failure",
  cancellation: "Cancellation",
  completion: "Completion",
};

/**
 * Map transaction list → chronological synthetic events for the player.
 * Honest limitation: not a true provider/form event stream — transaction commits only.
 */
export function transactionsToReplayEvents(
  transactions: TransactionRow[],
): ReplayEvent[] {
  return [...transactions]
    .sort((a, b) => {
      const ta = Date.parse(a.createdAt);
      const tb = Date.parse(b.createdAt);
      if (!Number.isNaN(ta) && !Number.isNaN(tb) && ta !== tb) return ta - tb;
      return a.id.localeCompare(b.id);
    })
    .map((t) => ({
      id: t.id,
      kind: "committed_transaction" as const,
      summary: redactSecretsForDisplay(
        `Committed transaction: ${t.summary}`,
      ),
      createdAt: t.createdAt,
      status: t.status,
      opsCount: t.opsCount,
    }));
}

/** Map redacted timeline DTOs → display-only replay events (never apply). */
export function timelineToReplayEvents(rows: TimelineRow[]): ReplayEvent[] {
  return [...rows]
    .sort((a, b) => {
      const ta = Date.parse(a.createdAt);
      const tb = Date.parse(b.createdAt);
      if (!Number.isNaN(ta) && !Number.isNaN(tb) && ta !== tb) return ta - tb;
      if (a.turnId !== b.turnId) return a.turnId.localeCompare(b.turnId);
      return a.sequence - b.sequence;
    })
    .map((row) => {
      const label = TIMELINE_KIND_LABELS[row.kind] ?? row.kind;
      const payload =
        row.redactedPayload && typeof row.redactedPayload === "object"
          ? (row.redactedPayload as Record<string, unknown>)
          : null;
      const detail =
        typeof payload?.summary === "string"
          ? payload.summary
          : typeof payload?.message === "string"
            ? payload.message
            : typeof payload?.reason === "string"
              ? payload.reason
              : typeof payload?.syncKind === "string"
                ? payload.syncKind
                : "";
      const summary = redactSecretsForDisplay(
        detail ? `${label}: ${detail}` : label,
      );
      return {
        id: row.id,
        kind: row.kind,
        summary,
        createdAt: row.createdAt,
        status: row.kind,
        opsCount: 0,
      };
    });
}
