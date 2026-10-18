import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";

type RecoverableTurn = {
  id: string;
  conversationId: string;
  attemptId: string;
  state: string;
  operationsJson?: string | null;
  provisionalText?: string | null;
  updatedAt: string;
};

/** True when operationsJson is a progressive staging checkpoint with at least one op. */
function hasStagedOpsCheckpoint(raw: string | null | undefined): boolean {
  if (typeof raw !== "string" || raw.length < 3) return false;
  try {
    const parsed = JSON.parse(raw) as {
      v?: unknown;
      ops?: unknown;
    };
    return (
      parsed.v === "coreside.ops.checkpoint.v1" &&
      Array.isArray(parsed.ops) &&
      parsed.ops.length > 0
    );
  } catch {
    return false;
  }
}

/**
 * Surfaces interrupted progressive generation that has durable staging on
 * turn_journal (ops checkpoint). Staging is never applied automatically.
 */
export function GenerationRecoveryBanner({
  conversationId,
}: {
  conversationId: string;
}) {
  const [turns, setTurns] = useState<RecoverableTurn[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(
    async (signal?: { cancelled: boolean }) => {
      try {
        const rows = await api.listInterruptedTurns(conversationId, 5);
        if (signal?.cancelled) return;
        setTurns(
          rows.filter(
            (t) =>
              t.state === "interrupted_recoverable" &&
              hasStagedOpsCheckpoint(t.operationsJson),
          ),
        );
        setError(null);
      } catch (e) {
        if (signal?.cancelled) return;
        setError(e instanceof Error ? e.message : "Could not load recovery state");
        setTurns([]);
      }
    },
    [conversationId],
  );

  useEffect(() => {
    const signal = { cancelled: false };
    void refresh(signal);
    return () => {
      signal.cancelled = true;
    };
  }, [refresh]);

  if (!turns.length && !error) return null;

  return (
    <div
      className="generation-recovery"
      role="region"
      aria-label="Interrupted generation recovery"
      aria-live="polite"
      data-testid="generation-recovery"
    >
      {error ? (
        <p className="generation-recovery-error" role="alert">
          {error}
        </p>
      ) : null}
      {turns.map((turn) => (
        <div key={turn.id} className="generation-recovery-card">
          <div className="generation-recovery-copy">
            <strong>Application generation interrupted</strong>
            <p id={`generation-recovery-desc-${turn.id}`}>
              Coreside preserved the last valid preview, but the application has
              not been applied. You can discard the preview or continue chatting
              to retry.
            </p>
          </div>
          <div className="generation-recovery-actions">
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busyId === turn.id}
              aria-busy={busyId === turn.id}
              aria-describedby={`generation-recovery-desc-${turn.id}`}
              onClick={() => {
                setBusyId(turn.id);
                setError(null);
                void (async () => {
                  try {
                    await api.discardInterruptedTurn(turn.id);
                    await refresh();
                  } catch (e) {
                    setError(
                      e instanceof Error ? e.message : "Discard failed",
                    );
                  } finally {
                    setBusyId(null);
                  }
                })();
              }}
            >
              Discard Preview
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
