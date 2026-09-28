import { useEffect, useState } from "react";
import { History, Undo2, X } from "lucide-react";
import { interactiveStatusText } from "@/lib/interactive-surface";
import type { InteractiveHistoryEntry, InteractiveView } from "@/types/runtime-v2";

/** Turn, outcome, undo, and read-only history/replay for a rules-engine surface. */
export function InteractiveStatusBar({
  view,
  error,
  pending,
  history,
  replaySeq,
  onUndo,
  onLoadHistory,
  onReplayAt,
  onClearReplay,
}: {
  view: InteractiveView | null;
  error: string | null;
  pending: boolean;
  history?: InteractiveHistoryEntry[];
  replaySeq?: number | null;
  onUndo: () => void;
  onLoadHistory?: () => void | Promise<void>;
  onReplayAt?: (seq: number) => void | Promise<void>;
  onClearReplay?: () => void;
}) {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (open) void onLoadHistory?.();
  }, [open, onLoadHistory]);

  if (!view) return null;
  const status = interactiveStatusText(view);
  const entries = history ?? [];
  const inspecting = replaySeq != null;

  return (
    <div className="interactive-status-wrap">
      <div
        className="interactive-status"
        data-status={view.status}
        data-replay={inspecting ? "true" : undefined}
      >
        <span className="interactive-status-text" role="status" aria-live="polite">
          {pending
            ? "Applying…"
            : inspecting
              ? `Inspecting step ${replaySeq}`
              : status}
        </span>
        {error ? (
          <span className="interactive-status-error" role="alert">
            {error}
          </span>
        ) : null}
        <div className="interactive-status-actions">
          {onLoadHistory ? (
            <button
              type="button"
              className="btn btn-ghost interactive-status-history"
              onClick={() => setOpen((v) => !v)}
              disabled={pending}
              aria-expanded={open}
              aria-label={open ? "Hide history" : "Show history"}
              title="History"
            >
              <History size={14} aria-hidden />
            </button>
          ) : null}
          {inspecting && onClearReplay ? (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={onClearReplay}
              aria-label="Exit history inspection"
              title="Exit inspection"
            >
              <X size={14} aria-hidden />
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-ghost interactive-status-undo"
            onClick={onUndo}
            disabled={pending || view.seq <= 1 || inspecting}
            aria-label="Undo last move"
            title="Undo"
          >
            <Undo2 size={14} aria-hidden />
          </button>
        </div>
      </div>
      {open ? (
        <ol className="interactive-history" aria-label="Action history">
          {entries.length === 0 ? (
            <li className="interactive-history-empty">No recorded actions yet.</li>
          ) : (
            entries.map((entry) => (
              <li key={`${entry.seq}-${entry.eventId}`}>
                <button
                  type="button"
                  className="interactive-history-item"
                  data-active={replaySeq === entry.seq ? "true" : undefined}
                  disabled={!onReplayAt || pending}
                  onClick={() => void onReplayAt?.(entry.seq)}
                >
                  <span className="interactive-history-seq">#{entry.seq}</span>
                  <span className="interactive-history-action">{entry.actionId}</span>
                  <span className="interactive-history-actor">{entry.actor}</span>
                  {entry.checkpoint ? (
                    <span className="interactive-history-checkpoint">checkpoint</span>
                  ) : null}
                </button>
              </li>
            ))
          )}
        </ol>
      ) : null}
    </div>
  );
}
