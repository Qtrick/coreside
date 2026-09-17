import { useCallback, useEffect, useId, useRef, useState } from "react";
import { X } from "lucide-react";
import { ModalPortal } from "@/components/ui/ModalPortal";
import { api } from "@/lib/tauri";
import { useAppStore } from "@/stores/app-store";
import type { BranchDiffRecord } from "@/types/runtime-v2";
import {
  REPLAY_STEP_MS,
  redactDiagnosticJson,
  timelineToReplayEvents,
  transactionsToReplayEvents,
  type ReplayEvent,
  type TimelineRow,
  type TransactionRow,
} from "./conversation-history-utils";

type HistoryTab = "branches" | "snapshots" | "replay" | "inspector";

type BranchRow = {
  id: string;
  branchName: string;
  newConversationId: string;
  createdAt: string;
};

type SnapshotRow = {
  id: string;
  description: string;
  createdAt: string;
};

function asString(v: unknown, fallback = ""): string {
  return typeof v === "string" ? v : fallback;
}

function parseBranches(raw: Record<string, unknown>[]): BranchRow[] {
  const rows: BranchRow[] = [];
  for (const row of raw) {
    const id = asString(row.id);
    const newConversationId = asString(row.newConversationId);
    if (!id || !newConversationId) continue;
    rows.push({
      id,
      branchName: asString(row.branchName, "Branch"),
      newConversationId,
      createdAt: asString(row.createdAt),
    });
  }
  return rows;
}

function parseSnapshots(raw: Record<string, unknown>[]): SnapshotRow[] {
  const rows: SnapshotRow[] = [];
  for (const row of raw) {
    const id = asString(row.id);
    if (!id) continue;
    rows.push({
      id,
      description: asString(row.description) || "(no description)",
      createdAt: asString(row.createdAt),
    });
  }
  return rows;
}

function parseTransactions(raw: Record<string, unknown>[]): TransactionRow[] {
  const rows: TransactionRow[] = [];
  for (const row of raw) {
    const id = asString(row.id);
    if (!id) continue;
    // Read-only replay: keep opsCount only — never retain operation payloads in UI state.
    const opsCount = Array.isArray(row.operations) ? row.operations.length : 0;
    rows.push({
      id,
      summary: asString(row.summary) || "(no summary)",
      status: asString(row.status, "unknown"),
      createdAt: asString(row.createdAt),
      opsCount,
    });
  }
  return rows;
}

function parseTimelineRows(raw: Record<string, unknown>[]): TimelineRow[] {
  const rows: TimelineRow[] = [];
  for (const row of raw) {
    const id = asString(row.id);
    const conversationId = asString(row.conversationId);
    const turnId = asString(row.turnId);
    const kind = asString(row.kind);
    if (!id || !conversationId || !turnId || !kind) continue;
    const sequence =
      typeof row.sequence === "number" && Number.isFinite(row.sequence)
        ? row.sequence
        : 0;
    rows.push({
      id,
      conversationId,
      turnId,
      sequence,
      kind,
      redactedPayload: row.redactedPayload,
      createdAt: asString(row.createdAt),
    });
  }
  return rows;
}

function formatTime(iso: string): string {
  if (!iso) return "—";
  const d = Date.parse(iso);
  if (Number.isNaN(d)) return iso;
  return new Date(d).toLocaleString();
}

/**
 * Read-only paced stepper over redacted synthetic events.
 *
 * HARD RULE (UI + code): Replay never reinvokes the provider, never
 * re-applies transactions (`applyOperations` / `undoTransaction`), and
 * never resubmits forms. Play only advances a local cursor via setTimeout.
 */
export function ReplayPlayer({ events }: { events: ReplayEvent[] }) {
  // null = live (not inspecting a historical event)
  const [cursor, setCursor] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<1 | 2>(1);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current != null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const returnToLive = useCallback(() => {
    clearTimer();
    setPlaying(false);
    setCursor(null);
  }, [clearTimer]);

  useEffect(() => () => clearTimer(), [clearTimer]);

  useEffect(() => {
    if (!playing || events.length === 0) {
      clearTimer();
      return;
    }
    clearTimer();
    timerRef.current = setTimeout(() => {
      setCursor((cur) => {
        const next = cur == null ? 0 : cur + 1;
        if (next >= events.length) {
          setPlaying(false);
          return events.length - 1;
        }
        return next;
      });
    }, REPLAY_STEP_MS / speed);
    return clearTimer;
  }, [playing, cursor, speed, events.length, clearTimer]);

  const stepPrev = () => {
    if (events.length === 0) return;
    setPlaying(false);
    setCursor((cur) => {
      if (cur == null) return events.length - 1;
      return Math.max(0, cur - 1);
    });
  };

  const stepNext = () => {
    if (events.length === 0) return;
    setPlaying(false);
    setCursor((cur) => {
      if (cur == null) return 0;
      return Math.min(events.length - 1, cur + 1);
    });
  };

  const togglePlay = () => {
    if (events.length === 0) return;
    if (playing) {
      setPlaying(false);
      return;
    }
    setCursor((cur) => (cur == null ? 0 : cur));
    setPlaying(true);
  };

  const current = cursor != null ? events[cursor] ?? null : null;
  const positionLabel =
    cursor == null
      ? "Live"
      : `${cursor + 1} / ${events.length}`;
  const usesTimeline = events.some((e) => e.kind !== "committed_transaction");

  return (
    <div className="replay-player" aria-label="Read-only replay player">
      <p className="muted conversation-history-note">
        Read-only replay. Never reinvokes the provider, never re-applies
        transactions, never resubmits forms.
        {usesTimeline
          ? " Showing redacted turn timeline events when available."
          : " Events are synthetic from the transaction list (Committed transaction) when no timeline exists."}
      </p>
      <div className="conversation-history-replay-controls">
        <button
          type="button"
          className="btn btn-secondary btn-compact"
          disabled={events.length === 0}
          aria-label="Previous replay event"
          onClick={stepPrev}
        >
          Previous
        </button>
        <button
          type="button"
          className="btn btn-secondary btn-compact"
          disabled={events.length === 0}
          aria-label="Next replay event"
          onClick={stepNext}
        >
          Next
        </button>
        <button
          type="button"
          className="btn btn-secondary btn-compact"
          disabled={events.length === 0}
          aria-label={playing ? "Pause replay" : "Play replay"}
          onClick={togglePlay}
        >
          {playing ? "Pause" : "Play"}
        </button>
        <button
          type="button"
          className="btn btn-ghost btn-compact"
          aria-label={`Replay speed ${speed}x`}
          aria-pressed={speed === 2}
          disabled={events.length === 0}
          onClick={() => setSpeed((s) => (s === 1 ? 2 : 1))}
        >
          {speed}x
        </button>
        <button
          type="button"
          className="btn btn-secondary btn-compact"
          aria-label="Return to live"
          disabled={cursor == null && !playing}
          onClick={returnToLive}
        >
          Return to live
        </button>
        <span className="muted" aria-live="polite">
          {positionLabel}
        </span>
      </div>

      {events.length === 0 ? (
        <p className="muted">No transactions yet.</p>
      ) : (
        <ul className="conversation-history-list" aria-label="Replay events">
          {events.map((ev, i) => (
            <li key={ev.id}>
              <button
                type="button"
                className={`conversation-history-txn${cursor === i ? " is-selected" : ""}`}
                aria-label={`Replay event ${ev.summary}`}
                aria-current={cursor === i ? "step" : undefined}
                onClick={() => {
                  setPlaying(false);
                  setCursor(i);
                }}
              >
                <strong>{ev.summary}</strong>
                <span className="muted">
                  {formatTime(ev.createdAt)} · {ev.opsCount} ops · {ev.status}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}

      <div
        className="conversation-history-detail replay-player-current"
        aria-label="Current replay event summary"
      >
        {current ? (
          <>
            <strong>{current.summary}</strong>
            <div className="muted">
              {formatTime(current.createdAt)} · {current.opsCount} ops ·{" "}
              {current.status} · read-only
            </div>
          </>
        ) : (
          <span className="muted">Live — select an event or press Play</span>
        )}
      </div>
    </div>
  );
}

export function ConversationHistory({
  conversationId,
}: {
  conversationId: string;
}) {
  const titleId = useId();
  const developerMode = useAppStore((s) => s.developerMode);
  const messages = useAppStore((s) => s.messages);
  const projectId = useAppStore((s) => {
    const conv = s.conversations.find((c) => c.id === conversationId);
    return conv?.projectId ?? null;
  });
  const refreshConversations = useAppStore((s) => s.refreshConversations);
  const navigateToChat = useAppStore((s) => s.navigateToChat);

  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<HistoryTab>("branches");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [branches, setBranches] = useState<BranchRow[]>([]);
  const [snapshots, setSnapshots] = useState<SnapshotRow[]>([]);
  const [transactions, setTransactions] = useState<TransactionRow[]>([]);
  const [timelineRows, setTimelineRows] = useState<TimelineRow[]>([]);
  const [diagnostics, setDiagnostics] = useState<unknown[]>([]);
  const [snapshotDetail, setSnapshotDetail] = useState<string | null>(null);
  const [diffDetail, setDiffDetail] = useState<BranchDiffRecord | null>(null);
  const [branchName, setBranchName] = useState("");
  const [snapshotDesc, setSnapshotDesc] = useState("");
  const loadGenRef = useRef(0);
  const conversationIdRef = useRef(conversationId);
  const openRef = useRef(open);
  conversationIdRef.current = conversationId;
  openRef.current = open;

  const tabs: { id: HistoryTab; label: string }[] = [
    { id: "branches", label: "Branches" },
    { id: "snapshots", label: "Snapshots" },
    { id: "replay", label: "Replay" },
    ...(developerMode ? [{ id: "inspector" as const, label: "Inspector" }] : []),
  ];

  const loadTab = useCallback(
    async (next: HistoryTab) => {
      const gen = ++loadGenRef.current;
      const convId = conversationId;
      setError(null);
      const isCurrent = () =>
        loadGenRef.current === gen &&
        conversationIdRef.current === convId &&
        openRef.current;
      try {
        if (next === "branches") {
          const rows = parseBranches(await api.listBranches(convId));
          if (!isCurrent()) return;
          setBranches(rows);
        } else if (next === "snapshots") {
          const rows = parseSnapshots(await api.listSnapshots(convId));
          if (!isCurrent()) return;
          setSnapshots(rows);
        } else if (next === "replay") {
          const [timelineRaw, txnRaw] = await Promise.all([
            api.listTurnTimeline(convId, null, 200).catch(() => []),
            api.listTransactions(convId, 50),
          ]);
          if (!isCurrent()) return;
          const timeline = parseTimelineRows(timelineRaw);
          setTimelineRows(timeline);
          setTransactions(parseTransactions(txnRaw));
        } else if (next === "inspector") {
          if (!developerMode) return;
          const rows = await api.listDiagnostics(convId, 20);
          if (!isCurrent()) return;
          setDiagnostics(rows);
        }
      } catch (err) {
        if (!isCurrent()) return;
        setError(err instanceof Error ? err.message : "Failed to load history");
      }
    },
    [conversationId, developerMode],
  );

  useEffect(() => {
    if (!open) return;
    void loadTab(tab);
  }, [open, tab, loadTab]);

  // Drop prior conversation rows immediately on switch (avoid stale flash).
  useEffect(() => {
    setBranches([]);
    setSnapshots([]);
    setTransactions([]);
    setTimelineRows([]);
    setDiagnostics([]);
    setSnapshotDetail(null);
    setDiffDetail(null);
    setError(null);
    setBranchName("");
    setSnapshotDesc("");
  }, [conversationId]);

  useEffect(() => {
    if (!developerMode && tab === "inspector") setTab("branches");
  }, [developerMode, tab]);

  // Drop diagnostics from React state when Inspector is not visible.
  useEffect(() => {
    if (!open || tab !== "inspector" || !developerMode) {
      setDiagnostics([]);
    }
  }, [open, tab, developerMode]);

  const createBranch = async () => {
    const sourceMessageId = messages[messages.length - 1]?.id;
    if (!sourceMessageId) {
      setError("Add a message before creating a branch.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const name = branchName.trim() || `Branch ${new Date().toLocaleString()}`;
      await api.branchConversation({
        sourceConversationId: conversationId,
        sourceMessageId,
        branchName: name,
      });
      setBranchName("");
      await refreshConversations();
      await loadTab("branches");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create branch");
    } finally {
      setBusy(false);
    }
  };

  const viewBranchDiff = async (branchId: string) => {
    setBusy(true);
    setError(null);
    try {
      const diff = await api.diffBranch(branchId);
      setDiffDetail(diff);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not compute branch diff");
    } finally {
      setBusy(false);
    }
  };

  const createSnap = async () => {
    setBusy(true);
    setError(null);
    try {
      const description =
        snapshotDesc.trim() || `Snapshot ${new Date().toLocaleString()}`;
      await api.createSnapshot(conversationId, projectId, description);
      setSnapshotDesc("");
      await loadTab("snapshots");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create snapshot");
    } finally {
      setBusy(false);
    }
  };

  const openSnapshot = async (snapshotId: string) => {
    setBusy(true);
    setError(null);
    setSnapshotDetail(null);
    try {
      const snap = await api.getSnapshot(snapshotId);
      const payload = snap.payload;
      const summary = {
        id: snap.id,
        description: snap.description,
        createdAt: snap.createdAt,
        readOnly: true,
        messageCount:
          payload &&
          typeof payload === "object" &&
          Array.isArray((payload as { messages?: unknown }).messages)
            ? (payload as { messages: unknown[] }).messages.length
            : undefined,
        transactionCount:
          payload &&
          typeof payload === "object" &&
          Array.isArray((payload as { transactions?: unknown }).transactions)
            ? (payload as { transactions: unknown[] }).transactions.length
            : undefined,
      };
      setSnapshotDetail(JSON.stringify(summary, null, 2));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load snapshot");
    } finally {
      setBusy(false);
    }
  };

  const replayEvents =
    timelineRows.length > 0
      ? timelineToReplayEvents(timelineRows)
      : transactionsToReplayEvents(transactions);

  return (
    <>
      <button
        type="button"
        className="btn btn-secondary btn-compact"
        aria-label="Open conversation history"
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => {
          setOpen(true);
          window.dispatchEvent(new CustomEvent("coreside:history-opened"));
        }}
      >
        History
      </button>

      {open ? (
        <ModalPortal onClose={() => !busy && setOpen(false)}>
          <div className="provider-modal-root" role="presentation">
            <button
              type="button"
              className="provider-modal-backdrop"
              aria-label="Close history"
              onClick={() => !busy && setOpen(false)}
            />
            <div
              className="provider-modal project-dialog conversation-history-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby={titleId}
            >
              <div className="provider-modal-header">
                <div>
                  <h2 id={titleId}>History</h2>
                  <p className="muted">Branches, snapshots, and read-only replay</p>
                </div>
                <button
                  type="button"
                  className="btn-icon"
                  onClick={() => setOpen(false)}
                  disabled={busy}
                  aria-label="Close history"
                >
                  <X size={16} />
                </button>
              </div>

              <div
                className="conversation-history-tabs"
                role="tablist"
                aria-label="History sections"
              >
                {tabs.map((t) => (
                  <button
                    key={t.id}
                    type="button"
                    role="tab"
                    className={`btn btn-ghost btn-compact${tab === t.id ? " is-active" : ""}`}
                    aria-selected={tab === t.id}
                    onClick={() => {
                      setSnapshotDetail(null);
                      setDiffDetail(null);
                      setTab(t.id);
                    }}
                  >
                    {t.label}
                  </button>
                ))}
              </div>

              <div className="conversation-history-body">
                {error ? (
                  <p className="form-error" role="alert">
                    {error}
                  </p>
                ) : null}

                {tab === "branches" ? (
                  <section aria-label="Branches">
                    <div className="conversation-history-create">
                      <label className="conversation-history-field">
                        <span className="muted">Branch name</span>
                        <input
                          value={branchName}
                          onChange={(e) => setBranchName(e.target.value)}
                          placeholder="Optional name"
                          disabled={busy}
                          aria-label="Branch name"
                        />
                      </label>
                      <button
                        type="button"
                        className="btn btn-primary btn-compact"
                        disabled={busy}
                        aria-label="Create branch from latest message"
                        onClick={() => void createBranch()}
                      >
                        Create branch
                      </button>
                    </div>
                    {branches.length === 0 ? (
                      <p className="muted">No branches yet.</p>
                    ) : (
                      <ul className="conversation-history-list">
                        {branches.map((b) => (
                          <li key={b.id} className="conversation-history-item">
                            <div className="conversation-history-item-body">
                              <strong>{b.branchName || "Branch"}</strong>
                              <span className="muted">
                                {formatTime(b.createdAt)}
                              </span>
                            </div>
                            <div style={{ display: "flex", gap: "0.35rem" }}>
                              <button
                                type="button"
                                className="btn btn-ghost btn-compact"
                                aria-label={`Compare branch ${b.branchName}`}
                                disabled={busy}
                                onClick={() => void viewBranchDiff(b.id)}
                              >
                                Compare
                              </button>
                              <button
                                type="button"
                                className="btn btn-secondary btn-compact"
                                aria-label={`Open branch ${b.branchName}`}
                                onClick={() => {
                                  setOpen(false);
                                  void navigateToChat(b.newConversationId);
                                }}
                              >
                                Open
                              </button>
                            </div>
                          </li>
                        ))}
                      </ul>
                    )}
                    {diffDetail ? (
                      <div
                        className="conversation-history-detail"
                        aria-label="Branch comparison summary"
                      >
                        <div
                          style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "center",
                            marginBottom: "0.4rem",
                          }}
                        >
                          <strong>Branch Comparison</strong>
                          <button
                            type="button"
                            className="btn btn-ghost btn-compact"
                            onClick={() => setDiffDetail(null)}
                            aria-label="Close branch comparison"
                          >
                            Close
                          </button>
                        </div>
                        <p className="muted" style={{ margin: "0 0 0.4rem" }}>
                          Branch messages: {diffDetail.branchMessageCount} (+{diffDetail.uniqueBranchMessages} unique)
                          {" · "}
                          Source messages: {diffDetail.sourceMessageCount} (+{diffDetail.uniqueSourceMessages} unique)
                        </p>
                        {diffDetail.surfaces.length > 0 ? (
                          <div>
                            <strong>Surfaces:</strong>
                            <ul style={{ margin: "0.25rem 0 0", paddingLeft: "1.2rem" }}>
                              {diffDetail.surfaces.map((s) => (
                                <li key={s.surfaceId}>
                                  <strong>{s.name}</strong> ({s.status})
                                  {s.changedComponents.length > 0
                                    ? ` — changed components: ${s.changedComponents.join(", ")}`
                                    : ""}
                                </li>
                              ))}
                            </ul>
                          </div>
                        ) : (
                          <p className="muted" style={{ margin: 0 }}>
                            No surface differences detected.
                          </p>
                        )}
                      </div>
                    ) : null}
                  </section>
                ) : null}

                {tab === "snapshots" ? (
                  <section aria-label="Snapshots">
                    <div className="conversation-history-create">
                      <label className="conversation-history-field">
                        <span className="muted">Description</span>
                        <input
                          value={snapshotDesc}
                          onChange={(e) => setSnapshotDesc(e.target.value)}
                          placeholder="Optional description"
                          disabled={busy}
                          aria-label="Snapshot description"
                        />
                      </label>
                      <button
                        type="button"
                        className="btn btn-primary btn-compact"
                        disabled={busy}
                        aria-label="Create snapshot"
                        onClick={() => void createSnap()}
                      >
                        Create snapshot
                      </button>
                    </div>
                    {snapshots.length === 0 ? (
                      <p className="muted">No snapshots yet.</p>
                    ) : (
                      <ul className="conversation-history-list">
                        {snapshots.map((s) => (
                          <li key={s.id} className="conversation-history-item">
                            <div className="conversation-history-item-body">
                              <strong>{s.description}</strong>
                              <span className="muted">
                                {formatTime(s.createdAt)}
                              </span>
                            </div>
                            <button
                              type="button"
                              className="btn btn-secondary btn-compact"
                              aria-label={`View snapshot ${s.description}`}
                              disabled={busy}
                              onClick={() => void openSnapshot(s.id)}
                            >
                              View
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
                    {snapshotDetail ? (
                      <pre
                        className="conversation-history-detail"
                        aria-label="Snapshot summary"
                      >
                        {snapshotDetail}
                      </pre>
                    ) : null}
                  </section>
                ) : null}

                {tab === "replay" ? (
                  <section aria-label="Replay">
                    <ReplayPlayer
                      key={conversationId}
                      events={replayEvents}
                    />
                  </section>
                ) : null}

                {tab === "inspector" ? (
                  <section aria-label="Developer inspector">
                    <p className="muted conversation-history-note">
                      Developer diagnostics with client-side secret redaction.
                    </p>
                    {diagnostics.length === 0 ? (
                      <p className="muted">No diagnostics stored.</p>
                    ) : (
                      <ul className="conversation-history-list">
                        {diagnostics.map((d, i) => (
                          <li key={i}>
                            <pre
                              className="conversation-history-detail"
                              aria-label={`Diagnostic entry ${i + 1}`}
                            >
                              {redactDiagnosticJson(d)}
                            </pre>
                          </li>
                        ))}
                      </ul>
                    )}
                  </section>
                ) : null}
              </div>
            </div>
          </div>
        </ModalPortal>
      ) : null}
    </>
  );
}
