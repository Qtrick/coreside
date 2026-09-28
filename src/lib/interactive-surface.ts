import { useCallback, useRef, useState, type MutableRefObject } from "react";
import { api, TauriCommandError } from "@/lib/tauri";
import type { InteractiveDispatchOutcome } from "@/lib/actions";
import type { InteractiveHistoryEntry, InteractiveView } from "@/types/runtime-v2";
import type { ToolState } from "@/types/tool";

/** True when a surface definition carries a Rust rules-engine definition. */
export function hasInteractiveDefinition(definition: unknown): boolean {
  if (!definition || typeof definition !== "object") return false;
  const interactive = (definition as Record<string, unknown>).interactive;
  return Boolean(interactive && typeof interactive === "object");
}


const ONGOING = new Set(["playing", "active", "in_progress", "editing", "planning", "running"]);

function humanize(value: string): string {
  const text = value.replace(/[_-]+/g, " ").trim();
  return text ? text[0].toUpperCase() + text.slice(1) : text;
}

export function interactiveStatusText(view: InteractiveView): string | null {
  if (view.winner) return `${humanize(view.winner)} wins`;
  if (!ONGOING.has(view.status)) return view.message ?? humanize(view.status);
  if (view.waitingFor) return `Waiting for ${humanize(view.waitingFor)}`;
  if (typeof view.state.currentPlayer === "string" && view.actor) {
    return `${humanize(view.actor)} to move`;
  }
  return null;
}

const MODEL_STATE_BUDGET_BYTES = 5000;

/**
 * Bound the public state sent to the model for an AI turn: repeatedly keep the
 * newest half of the largest array until the JSON fits the structured-input budget.
 */
export function compactStateForModel(
  state: Record<string, unknown>,
  budget = MODEL_STATE_BUDGET_BYTES,
): Record<string, unknown> {
  const copy = structuredClone(state) as Record<string, unknown>;
  for (let guard = 0; guard < 64 && JSON.stringify(copy).length > budget; guard += 1) {
    let largest: { holder: Record<string, unknown>; key: string; size: number } | null = null;
    const visit = (holder: Record<string, unknown>, depth: number) => {
      for (const [key, value] of Object.entries(holder)) {
        if (Array.isArray(value) && value.length > 1) {
          const size = JSON.stringify(value).length;
          if (!largest || size > largest.size) largest = { holder, key, size };
        } else if (value && typeof value === "object" && depth < 2) {
          visit(value as Record<string, unknown>, depth + 1);
        }
      }
    };
    visit(copy, 0);
    if (!largest) break;
    const { holder, key } = largest as { holder: Record<string, unknown>; key: string };
    const arr = holder[key] as unknown[];
    holder[key] = arr.slice(Math.floor(arr.length / 2));
  }
  return copy;
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  return "That action could not be applied.";
}

/**
 * Host-side binding for an interactive surface. Rust owns legality and state;
 * this hook only forwards gestures and adopts the committed view.
 */
export function useInteractiveSurface({
  surfaceId,
  stateRevisionRef,
  setState,
  beforeDispatch,
  onAdopt,
}: {
  surfaceId: string;
  stateRevisionRef: MutableRefObject<number>;
  setState: (update: (current: ToolState) => ToolState) => void;
  beforeDispatch?: () => Promise<void>;
  onAdopt?: (view: InteractiveView) => void;
}) {
  const [view, setView] = useState<InteractiveView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [history, setHistory] = useState<InteractiveHistoryEntry[]>([]);
  const [replayState, setReplayState] = useState<Record<string, unknown> | null>(null);
  const [replaySeq, setReplaySeq] = useState<number | null>(null);
  const inFlight = useRef(false);

  const adopt = useCallback(
    (next: InteractiveView) => {
      stateRevisionRef.current = next.stateRevision;
      setView(next);
      setState((current) => ({ ...current, ...next.state }));
      onAdopt?.(next);
    },
    [onAdopt, setState, stateRevisionRef],
  );

  const hydrate = useCallback(async () => {
    const next = await api.interactiveView(surfaceId);
    adopt(next);
    return next;
  }, [adopt, surfaceId]);

  const run = useCallback(
    async (call: () => Promise<InteractiveView>): Promise<InteractiveDispatchOutcome> => {
      if (inFlight.current) {
        return { ok: false, error: "Still applying the previous action." };
      }
      inFlight.current = true;
      setPending(true);
      try {
        await beforeDispatch?.();
        const next = await call();
        adopt(next);
        setError(null);
        return { ok: true, state: next.state };
      } catch (e) {
        const stale = e instanceof TauriCommandError && e.code === "conflict";
        const message = stale ? "This app changed elsewhere, so it was refreshed." : describe(e);
        if (stale) await hydrate().catch(() => undefined);
        setError(message);
        return { ok: false, error: message };
      } finally {
        inFlight.current = false;
        setPending(false);
      }
    },
    [adopt, beforeDispatch, hydrate],
  );

  const dispatch = useCallback(
    (payload: { actionId: string; params: Record<string, unknown> }) =>
      run(() =>
        api.interactiveDispatch({
          surfaceId,
          expectedStateRevision: stateRevisionRef.current,
          eventId: `ia-${crypto.randomUUID()}`,
          actionId: payload.actionId,
          params: payload.params,
        }),
      ),
    [run, stateRevisionRef, surfaceId],
  );

  const undo = useCallback(
    () =>
      run(() =>
        api.interactiveUndo(surfaceId, stateRevisionRef.current, `ia-undo-${crypto.randomUUID()}`),
      ),
    [run, stateRevisionRef, surfaceId],
  );

  const loadHistory = useCallback(async () => {
    const entries = await api.interactiveHistory(surfaceId);
    setHistory(entries);
    return entries;
  }, [surfaceId]);

  /** Read-only reconstruction of public state at `seq`. Does not mutate live state. */
  const replayAt = useCallback(
    async (seq: number) => {
      const snapshot = await api.interactiveReplay(surfaceId, seq);
      setReplaySeq(seq);
      setReplayState(snapshot);
      return snapshot;
    },
    [surfaceId],
  );

  const clearReplay = useCallback(() => {
    setReplaySeq(null);
    setReplayState(null);
  }, []);

  return {
    view,
    error,
    pending,
    history,
    replaySeq,
    replayState,
    hydrate,
    dispatch,
    undo,
    loadHistory,
    replayAt,
    clearReplay,
  };
}
