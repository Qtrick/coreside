/**
 * Minimal frontend turn registry for Channel-scoped streaming (RC3.3 Phase 4).
 *
 * Accumulates per-turn live state so navigation away from a chat does not drop
 * Channel text/action/error. `streamingText` in app-store remains a derived
 * view for the active conversation (MessageList compatibility).
 */

export type TurnLiveStatus = "streaming" | "completed" | "failed" | "cancelled";

export type TurnLiveState = {
  turnId: string;
  conversationId: string;
  status: TurnLiveStatus;
  text: string;
  lastSequence: number;
  actions: string[];
  error: string | null;
  startedAt: number;
};

export type TextDeltaInput = {
  text?: string | null;
  delta?: string | null;
  sequence?: number | null;
};

export function createTurnLiveState(
  turnId: string,
  conversationId: string,
  startedAt: number = Date.now(),
): TurnLiveState {
  return {
    turnId,
    conversationId,
    status: "streaming",
    text: "",
    lastSequence: 0,
    actions: [],
    error: null,
    startedAt,
  };
}

/**
 * Apply a Channel text event to turn live state.
 *
 * - Soft-reject duplicate or older sequences (return state unchanged).
 * - When `text` is a string, treat it as the authoritative checkpoint.
 *   Never append `delta` on top of an existing body when `text` is also
 *   supplied — Rust's non-prefix replace path sends the full body as `delta`,
 *   which would duplicate if appended.
 * - Ordinary events may be delta-primary (no `text`); append `delta` when
 *   `sequence === lastSequence + 1`, or when sequence is absent.
 * - Gaps (sequence > lastSequence + 1) with only a delta are soft-rejected;
 *   a `text` checkpoint still applies when present.
 */
export function applyTextDelta(
  state: TurnLiveState,
  event: TextDeltaInput,
): TurnLiveState {
  const seq =
    typeof event.sequence === "number" && Number.isFinite(event.sequence)
      ? event.sequence
      : null;

  // Soft reject: duplicate or older sequence.
  if (seq !== null && seq <= state.lastSequence) {
    return state;
  }

  const delta = typeof event.delta === "string" ? event.delta : null;
  // Only string checkpoints count — null/undefined means delta-primary.
  const text = typeof event.text === "string" ? event.text : null;

  // Authoritative checkpoint when present (periodic / first / non-prefix).
  if (text !== null) {
    return {
      ...state,
      text,
      lastSequence: seq ?? state.lastSequence,
    };
  }

  if (delta !== null && seq !== null && seq === state.lastSequence + 1) {
    return {
      ...state,
      text: state.text + delta,
      lastSequence: seq,
    };
  }

  // No sequence: best-effort append when only delta is available.
  if (delta !== null && seq === null) {
    return {
      ...state,
      text: state.text + delta,
    };
  }

  return state;
}

/** Provisional id until the first Channel text event supplies a real turnId. */
export function pendingTurnId(conversationId: string): string {
  return `pending:${conversationId}`;
}
