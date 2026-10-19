import { useCallback, useEffect, useId, useRef, useState } from "react";
import { api, listenAgentTurn } from "@/lib/tauri";
import type { AgentTurnEvent } from "@/lib/tauri";
import {
  applyPreviewSurfaceOverlay,
  clearPreviewOverlaysForTurn,
} from "@/lib/preview/surface-overlay";
import { validateToolDefinition } from "@/lib/tool-schema";
import { useAppStore } from "@/stores/app-store";
import {
  RecoveredPreviewHost,
  type RecoveredPreviewView,
} from "./RecoveredPreviewHost";

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

function consumerError(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.trim()) {
    const msg = error.message.trim();
    // Avoid dumping raw Rust/SQLite internals into the banner.
    if (/turn_cas|stale_|DbError|rusqlite|sqlite/i.test(msg)) {
      return fallback;
    }
    return msg;
  }
  return fallback;
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
  const [review, setReview] = useState<RecoveredPreviewView | null>(null);
  const refreshGen = useRef(0);
  const reviewToken = useRef(0);
  const gensEmptyRef = useRef(true);
  const descId = useId();
  gensEmptyRef.current = gens.length === 0;

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
        setError(consumerError(e, "Could not load recovery state"));
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

  // Drop review host when leaving the conversation (staging remains).
  useEffect(() => {
    setReview(null);
    reviewToken.current += 1;
  }, [conversationId]);

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
        // Cancel settlement can race the first list read; one deferred reload
        // picks up interrupted_recoverable after the journal transition commits.
        window.setTimeout(() => {
          if (!disposed) void refresh();
        }, 250);
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    // While no banner is showing, poll briefly so cancel→recoverable races
    // cannot leave the UI empty after the journal has already committed.
    const poll = window.setInterval(() => {
      if (!disposed && gensEmptyRef.current) void refresh();
    }, 1500);
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener("focus", onFocus);
      window.clearInterval(poll);
    };
  }, [conversationId, refresh]);

  const clearReviewForTurn = useCallback(
    (turnId: string) => {
      useAppStore.setState((state) => ({
        previewSurfacesByKey: clearPreviewOverlaysForTurn(
          state.previewSurfacesByKey,
          conversationId,
          turnId,
        ),
      }));
      setReview((current) => (current?.turnId === turnId ? null : current));
    },
    [conversationId],
  );

  const onCloseReview = useCallback(() => {
    if (review) clearReviewForTurn(review.turnId);
    else setReview(null);
  }, [clearReviewForTurn, review]);

  const onReview = async (gen: RecoverableGeneration) => {
    const token = ++reviewToken.current;
    setBusyId(gen.turnId);
    setError(null);
    try {
      const preview = await api.getRecoverableGenerationPreview({
        conversationId,
        turnId: gen.turnId,
        expectedAttemptId: gen.attemptId,
        expectedSequence: gen.sequence,
      });
      if (token !== reviewToken.current) return;

      const surfaces: RecoveredPreviewView["surfaces"] = [];
      for (const surface of preview.surfaces) {
        const validated = validateToolDefinition(surface.definitionJson);
        if (!validated.success) continue;
        const state =
          surface.stateJson &&
          typeof surface.stateJson === "object" &&
          !Array.isArray(surface.stateJson)
            ? (surface.stateJson as Record<string, unknown>)
            : {};
        surfaces.push({
          surfaceId: surface.surfaceId,
          toolId: surface.toolId ?? validated.data.id,
          applicationId: surface.applicationId ?? null,
          tool: validated.data,
          state,
          revision: surface.revision,
          sequence: surface.sequence,
        });
      }
      if (surfaces.length === 0) {
        setError(
          "Could not restore a visible preview from the saved checkpoint. Your staging is still available — try Retry or Discard.",
        );
        setReview(null);
        return;
      }

      // Paint overlays for already-mounted hosts without clearing other turns.
      let next = useAppStore.getState().previewSurfacesByKey;
      next = clearPreviewOverlaysForTurn(next, conversationId, preview.turnId);
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

      const fidelity =
        preview.fidelity === "partial" || preview.warnings.length > 0
          ? "partial"
          : "full";
      setReview({
        turnId: preview.turnId,
        conversationId: preview.conversationId,
        attemptId: preview.attemptId,
        fidelity,
        surfaces,
        warnings: preview.warnings,
      });
      if (fidelity === "partial") {
        setError(
          "Preview restored partially — some staged operations could not be reconstructed. Inspect-only; nothing was applied.",
        );
      }
    } catch (e) {
      if (token !== reviewToken.current) return;
      setError(consumerError(e, "Could not restore preview"));
      setReview(null);
    } finally {
      setBusyId(null);
    }
  };

  const onRetry = async (gen: RecoverableGeneration) => {
    setBusyId(gen.turnId);
    setError(null);
    try {
      // Store-owned path mirrors sendMessage: full AgentTurnEvent handling,
      // getMessages reload, and toolChange/runtimeV2 proposal staging.
      const result = await useAppStore.getState().retryRecoverableGeneration({
        conversationId,
        turnId: gen.turnId,
        expectedAttemptId: gen.attemptId,
        expectedSequence:
          gen.status === "previewAvailable" ? gen.sequence : undefined,
      });
      if (!result.admitted) {
        setError(
          "Retry was not admitted. Your saved preview remains available.",
        );
        await refresh();
        return;
      }
      clearReviewForTurn(gen.turnId);
      await refresh();
    } catch (e) {
      await refresh();
      setError(
        consumerError(
          e,
          "Retry failed. Your saved preview remains available if it was not admitted.",
        ),
      );
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
      clearReviewForTurn(gen.turnId);
      await refresh();
    } catch (e) {
      await refresh();
      setError(consumerError(e, "Discard failed"));
    } finally {
      setBusyId(null);
    }
  };

  if (!gens.length && !error && !review) return null;

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
      {review ? (
        <RecoveredPreviewHost preview={review} onClose={onCloseReview} />
      ) : null}
      {gens.map((gen) => {
        const copy = statusCopy(gen);
        const busy = busyId === gen.turnId;
        const reviewing = review?.turnId === gen.turnId;
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
                  {busy && busyId === gen.turnId
                    ? "Starting…"
                    : "Retry Generation"}
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
