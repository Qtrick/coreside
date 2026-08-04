import { useCallback, useEffect, useId, useRef, useState } from "react";
import { X } from "lucide-react";
import { ModalPortal } from "@/components/ui/ModalPortal";
import { api } from "@/lib/tauri";
import { useAppStore } from "@/stores/app-store";

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

type TransactionRow = {
  id: string;
  summary: string;
  status: string;
  createdAt: string;
  opsCount: number;
};

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

function formatTime(iso: string): string {
  if (!iso) return "—";
  const d = Date.parse(iso);
  if (Number.isNaN(d)) return iso;
  return new Date(d).toLocaleString();
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
  const [diagnostics, setDiagnostics] = useState<unknown[]>([]);
  const [selectedTxnId, setSelectedTxnId] = useState<string | null>(null);
  const [snapshotDetail, setSnapshotDetail] = useState<string | null>(null);
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
          const rows = parseTransactions(
            await api.listTransactions(convId, 50),
          );
          if (!isCurrent()) return;
          setTransactions(rows);
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
    setDiagnostics([]);
    setSelectedTxnId(null);
    setSnapshotDetail(null);
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

  const selectedTxn = transactions.find((t) => t.id === selectedTxnId) ?? null;

  return (
    <>
      <button
        type="button"
        className="btn btn-secondary btn-compact"
        aria-label="Open conversation history"
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen(true)}
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
                      setSelectedTxnId(null);
                      setSnapshotDetail(null);
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
                          </li>
                        ))}
                      </ul>
                    )}
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
                    <p className="muted conversation-history-note">
                      Read-only transaction list. Operations are not re-applied.
                    </p>
                    <div className="conversation-history-replay-controls">
                      <button
                        type="button"
                        className="btn btn-secondary btn-compact"
                        disabled
                        aria-label="Step previous transaction (read-only, unavailable)"
                      >
                        Prev
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary btn-compact"
                        disabled
                        aria-label="Step next transaction (read-only, unavailable)"
                      >
                        Next
                      </button>
                      <span className="muted">Step player unavailable (read-only)</span>
                    </div>
                    {transactions.length === 0 ? (
                      <p className="muted">No transactions yet.</p>
                    ) : (
                      <ul className="conversation-history-list">
                        {transactions.map((t) => (
                          <li key={t.id}>
                            <button
                              type="button"
                              className={`conversation-history-txn${selectedTxnId === t.id ? " is-selected" : ""}`}
                              aria-label={`Transaction ${t.summary}`}
                              aria-expanded={selectedTxnId === t.id}
                              onClick={() =>
                                setSelectedTxnId((cur) =>
                                  cur === t.id ? null : t.id,
                                )
                              }
                            >
                              <strong>{t.summary}</strong>
                              <span className="muted">
                                {formatTime(t.createdAt)} · {t.opsCount} ops ·{" "}
                                {t.status}
                              </span>
                            </button>
                          </li>
                        ))}
                      </ul>
                    )}
                    {selectedTxn ? (
                      <pre
                        className="conversation-history-detail"
                        aria-label="Transaction summary"
                      >
                        {JSON.stringify(
                          {
                            id: selectedTxn.id,
                            time: selectedTxn.createdAt,
                            status: selectedTxn.status,
                            opsCount: selectedTxn.opsCount,
                            summary: selectedTxn.summary,
                            readOnly: true,
                          },
                          null,
                          2,
                        )}
                      </pre>
                    ) : null}
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
