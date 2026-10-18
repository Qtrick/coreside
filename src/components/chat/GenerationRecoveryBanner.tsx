import { useCallback, useEffect, useId, useRef, useState } from "react";
import { api, listenAgentTurn } from "@/lib/tauri";
import type { AgentTurnEvent } from "@/lib/tauri";
import { applyPreviewSurfaceOverlay } from "@/lib/preview/surface-overlay";
import { useAppStore } from "@/stores/app-store";

export type RecoverableGeneration = {
  turnId: string;
  conversationId: string;
  attemptId: string;
  sequence: number;
  complete: boolean;
  status:
    | "previewAvailable"
    | "previewUnavailable"
    | "checkpointCorrupted"
    | "notRecoverable";
  opCount: number;
  contentHash?: string | null;
  updatedAt: string;
  canRetry: boolean;
  canDiscard: boolean;
  canReview: boolean;
};

function eventBelongsToConversation(
  event: AgentTurnEvent,
  conversationId: string,
): boolean {
  if (!("conversationId" in event)) return false;
  const id = event.conversationId;
  return typeof id === "string" && id === conversationId;
}

function statusCopy(gen: RecoverableGeneration): {
  title: string;
  body: string;
} {
  switch (gen.status) {
    case "previewAvailable":
      return {
        title: "Application generation interrupted",
        body: "Your last saved preview is available. It has not been applied to your application.",
      };
    case "checkpointCorrupted":
      return {
        title: "Saved preview cannot be restored",
        body: "Staging for this turn is damaged or incomplete. You can retry generation or discard it. Nothing was applied.",
      };
    case "previewUnavailable":
    default:
      return {
        title: "Application generation interrupted",
        body: "No recoverable preview was saved for this turn. You can retry generation or discard this notice. Nothing was applied.",
      };
  }
}

/**
 * Surfaces interrupted progressive generation with backend-validated recovery
 * eligibility. Staging is never applied automatically.
 */
export function GenerationRecoveryBanner({
  conversationId,
}: {
  conversationId: string;
}) {
  const [gens, setGens] = useState<RecoverableGeneration[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reviewingId, setReviewingId] = useState<string | null>(null);
  const refreshGen = useRef(0);
  const descId = useId();

  const refresh = useCallback(
    async (signal?: { cancelled: boolean }) => {
      const gen = ++refreshGen.current;
      try {
        const rows = await api.listRecoverableGenerations(conversationId, 5);
        if (signal?.cancelled || gen !== refreshGen.current) return;
        setGens(rows);
        setError(null);
      } catch (e) {
        if (signal?.cancelled || gen !== refreshGen.current) return;
        setError(
          e instanceof Error ? e.message : "Could not load recovery state",
        );
        setGens([]);
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

  // Refresh when this conversation's turn/stream settles so superseded or
  // newly interrupted staging appears without a remount.
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void listenAgentTurn((event) => {
      if (!eventBelongsToConversation(event, conversationId)) return;
      if (
        event.kind === "error" ||
        event.kind === "operation" ||
        event.kind === "sync"
      ) {
        void refresh();
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener("focus", onFocus);
    };
  }, [conversationId, refresh]);

  const clearRecoveredOverlay = useCallback(() => {
    useAppStore.setState((state) => ({
      previewSurfacesByKey: Object.fromEntries(
        Object.entries(state.previewSurfacesByKey).filter(
          ([, overlay]) => overlay.conversationId !== conversationId,
        ),
      ),
    }));
    setReviewingId(null);
  }, [conversationId]);

  const onReview = async (gen: RecoverableGeneration) => {
    setBusyId(gen.turnId);
    setError(null);
    try {
      const preview = await api.getRecoverableGenerationPreview({
        conversationId,
        turnId: gen.turnId,
        expectedAttemptId: gen.attemptId,
        expectedSequence: gen.sequence,
      });
      let next = useAppStore.getState().previewSurfacesByKey;
      // Drop prior overlays for this conversation before painting recovered ones.
      next = Object.fromEntries(
        Object.entries(next).filter(
          ([, overlay]) => overlay.conversationId !== conversationId,
        ),
      );
      for (const surface of preview.surfaces) {
        next = applyPreviewSurfaceOverlay(next, {
          conversationId,
          turnId: preview.turnId,
          toolId: surface.toolId,
          surfaceId: surface.surfaceId,
          applicationId: surface.applicationId,
          definitionJson: surface.definitionJson,
          stateJson: surface.stateJson,
          revision: surface.revision,
          sequence: surface.sequence,
        });
      }
      useAppStore.setState({ previewSurfacesByKey: next });
      setReviewingId(gen.turnId);
      if (preview.warnings.length > 0) {
        setError(
          `Preview restored with warnings: ${preview.warnings.slice(0, 2).join("; ")}`,
        );
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not restore preview");
      setReviewingId(null);
    } finally {
      setBusyId(null);
    }
  };

  const onRetry = async (gen: RecoverableGeneration) => {
    setBusyId(gen.turnId);
    setError(null);
    try {
      const result = await api.retryRecoverableGeneration({
        conversationId,
        turnId: gen.turnId,
        expectedAttemptId: gen.attemptId,
        expectedSequence:
          gen.status === "previewAvailable" ? gen.sequence : undefined,
      });
      // Clear overlay only after staging is retired so a failed CAS keeps Review.
      clearRecoveredOverlay();
      await refresh();
      // New generation attempt — does not resume the prior provider stream.
      await useAppStore.getState().sendMessage(result.originalContent);
    } catch (e) {
      // Refresh first; it clears error on success, so re-apply the action error after.
      await refresh();
      setError(e instanceof Error ? e.message : "Retry failed");
    } finally {
      setBusyId(null);
    }
  };

  const onDiscard = async (gen: RecoverableGeneration) => {
    setBusyId(gen.turnId);
    setError(null);
    try {
      await api.discardInterruptedTurn(
        gen.turnId,
        conversationId,
        gen.attemptId,
        gen.status === "previewAvailable" ? gen.sequence : undefined,
      );
      if (reviewingId === gen.turnId) {
        clearRecoveredOverlay();
      }
      await refresh();
    } catch (e) {
      await refresh();
      setError(e instanceof Error ? e.message : "Discard failed");
    } finally {
      setBusyId(null);
    }
  };

  if (!gens.length && !error) return null;

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
      {gens.map((gen) => {
        const copy = statusCopy(gen);
        const busy = busyId === gen.turnId;
        const reviewing = reviewingId === gen.turnId;
        const desc = `${descId}-${gen.turnId}`;
        return (
          <div
            key={gen.turnId}
            className="generation-recovery-card"
            data-status={gen.status}
            data-reviewing={reviewing ? "true" : "false"}
          >
            <div className="generation-recovery-copy">
              <strong>{copy.title}</strong>
              <p id={desc}>
                {copy.body}
                {reviewing
                  ? " Showing a read-only saved preview — it is not applied yet."
                  : null}
              </p>
            </div>
            <div className="generation-recovery-actions">
              {gen.canReview ? (
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={busy}
                  aria-busy={busy}
                  aria-describedby={desc}
                  onClick={() => void onReview(gen)}
                >
                  {reviewing ? "Refresh Preview" : "Review Preview"}
                </button>
              ) : null}
              {gen.canRetry ? (
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={busy}
                  aria-busy={busy}
                  aria-describedby={desc}
                  title="Starts a new generation attempt with your original request"
                  onClick={() => void onRetry(gen)}
                >
                  Retry Generation
                </button>
              ) : null}
              {gen.canDiscard ? (
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={busy}
                  aria-busy={busy}
                  aria-describedby={desc}
                  onClick={() => void onDiscard(gen)}
                >
                  Discard Preview
                </button>
              ) : null}
            </div>
          </div>
        );
      })}
    </div>
  );
}
