import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/tauri";
import { listenAutomationsChanged } from "@/lib/tauri/events";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { useAppStore } from "@/stores/app-store";
import {
  describeAutomationTrigger,
  type AutomationProposalTrigger,
} from "@/lib/automation-proposal";

type AutomationRun = {
  id: string;
  automationId: string;
  scheduledAt?: string | null;
  startedAt: string;
  completedAt?: string | null;
  status: string;
  resultSummary?: string | null;
  errorCategory?: string | null;
  createdAt: string;
};

type Automation = {
  id: string;
  name: string;
  enabled: boolean;
  requiresAi: boolean;
  applicationId?: string | null;
  waitingApproval?: boolean;
  permissionReady?: boolean;
  nextRunAt?: string | null;
  lastRunAt?: string | null;
  lastStatus?: string | null;
  consecutiveFailures: number;
  trigger: AutomationProposalTrigger;
};

function formatDuration(startedAt: string, completedAt?: string | null): string | null {
  if (!completedAt) return null;
  const start = new Date(startedAt).getTime();
  const end = new Date(completedAt).getTime();
  if (isNaN(start) || isNaN(end) || end < start) return null;
  const diffMs = end - start;
  if (diffMs < 1000) return `${diffMs}ms`;
  return `${(diffMs / 1000).toFixed(1)}s`;
}

function getStatusBadgeStyle(status: string): { background: string; color: string } {
  switch (status.toLowerCase()) {
    case "success":
      return { background: "rgba(16, 185, 129, 0.16)", color: "#34d399" };
    case "failed":
    case "error":
      return { background: "rgba(239, 68, 68, 0.16)", color: "#f87171" };
    case "running":
      return { background: "rgba(59, 130, 246, 0.16)", color: "#60a5fa" };
    case "interrupted":
      return { background: "rgba(245, 158, 11, 0.16)", color: "#fbbf24" };
    case "awaiting_approval":
      return { background: "rgba(168, 85, 247, 0.16)", color: "#c084fc" };
    case "busy":
    case "skipped":
    default:
      return { background: "rgba(107, 114, 128, 0.16)", color: "var(--text-muted, #9ca3af)" };
  }
}

export function AutomationsPanel({ onBack }: { onBack: () => void }) {
  const [rows, setRows] = useState<Automation[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [expandedHistoryId, setExpandedHistoryId] = useState<string | null>(null);
  const [runsCache, setRunsCache] = useState<Record<string, AutomationRun[]>>({});
  const [loadingRunsId, setLoadingRunsId] = useState<string | null>(null);
  const [copiedRunId, setCopiedRunId] = useState<string | null>(null);

  const reloadSeqRef = useRef(0);
  const runsSeqRef = useRef<Record<string, number>>({});
  const expandedHistoryIdRef = useRef(expandedHistoryId);
  expandedHistoryIdRef.current = expandedHistoryId;

  const navigateToSettings = useAppStore((s) => s.navigateToSettings);

  const reload = async () => {
    const seq = ++reloadSeqRef.current;
    try {
      const raw = await api.listAutomations();
      if (seq !== reloadSeqRef.current) return;
      setRows(
        raw.map((row) => ({
          id: String(row.id ?? ""),
          name: String(row.name ?? "Automation"),
          enabled: Boolean(row.enabled),
          requiresAi: Boolean(row.requiresAi),
          applicationId: (row.applicationId as string | null | undefined) ?? null,
          waitingApproval: Boolean(row.waitingApproval),
          permissionReady: Boolean(row.permissionReady),
          nextRunAt: (row.nextRunAt as string | null | undefined) ?? null,
          lastRunAt: (row.lastRunAt as string | null | undefined) ?? null,
          lastStatus: (row.lastStatus as string | null | undefined) ?? null,
          consecutiveFailures: Number(row.consecutiveFailures ?? 0),
          trigger: (row.trigger as AutomationProposalTrigger) ?? { type: "interval", intervalMinutes: 15 },
        })),
      );
      setError(null);
    } catch (err) {
      if (seq !== reloadSeqRef.current) return;
      setError(err instanceof Error ? err.message : "Could not load automations");
    }
  };

  const loadRuns = async (automationId: string) => {
    const seq = (runsSeqRef.current[automationId] = (runsSeqRef.current[automationId] ?? 0) + 1);
    setLoadingRunsId(automationId);
    try {
      const rawRuns = await api.listAutomationRuns(automationId);
      if (seq !== runsSeqRef.current[automationId]) return;
      const mappedRuns: AutomationRun[] = rawRuns.map((r) => ({
        id: String(r.id ?? ""),
        automationId: String(r.automationId ?? r.automation_id ?? automationId),
        scheduledAt: (r.scheduledAt as string | null | undefined) ?? (r.scheduled_at as string | null | undefined) ?? null,
        startedAt: String(r.startedAt ?? r.started_at ?? ""),
        completedAt: (r.completedAt as string | null | undefined) ?? (r.completed_at as string | null | undefined) ?? null,
        status: String(r.status ?? "unknown"),
        resultSummary: (r.resultSummary as string | null | undefined) ?? (r.result_summary as string | null | undefined) ?? null,
        errorCategory: (r.errorCategory as string | null | undefined) ?? (r.error_category as string | null | undefined) ?? null,
        createdAt: String(r.createdAt ?? r.created_at ?? ""),
      }));
      setRunsCache((prev) => ({
        ...prev,
        [automationId]: mappedRuns,
      }));
    } catch (err) {
      if (seq !== runsSeqRef.current[automationId]) return;
      setError(err instanceof Error ? err.message : "Could not load run history");
    } finally {
      if (seq === runsSeqRef.current[automationId]) {
        setLoadingRunsId(null);
      }
    }
  };

  useEffect(() => {
    void reload();
  }, []);

  // Live synchronization: update panel when backend background runs finish or update
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listenAutomationsChanged(() => {
      void reload();
      const currentExpandedId = expandedHistoryIdRef.current;
      if (currentExpandedId) {
        void loadRuns(currentExpandedId);
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const handleRunNow = async (row: Automation) => {
    setBusyId(row.id);
    setError(null);
    setInfo(null);
    try {
      const outcome = await api.runAutomationNow(row.id);
      if (outcome.status === "success") {
        setInfo(outcome.summary ? `Run completed: ${outcome.summary}` : "Run completed successfully.");
      } else if (outcome.status === "awaiting_approval") {
        setInfo("Run paused: Action requires approval before execution.");
      } else if (outcome.status === "busy") {
        setInfo("Automation is currently already executing.");
      } else if (outcome.status === "skipped") {
        setInfo(outcome.summary || "Run skipped.");
      } else {
        setError(outcome.summary || `Run ended with status: ${outcome.status}`);
      }
      await reload();
      if (expandedHistoryId === row.id) {
        await loadRuns(row.id);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not run automation");
    } finally {
      setBusyId(null);
    }
  };

  const handleToggleEnabled = async (row: Automation) => {
    setBusyId(row.id);
    setError(null);
    setInfo(null);
    try {
      await api.setAutomationEnabled(row.id, !row.enabled);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update automation");
    } finally {
      setBusyId(null);
    }
  };

  const handleDelete = async (automationId: string) => {
    setBusyId(automationId);
    setError(null);
    setInfo(null);
    try {
      await api.deleteAutomation(automationId);
      setConfirmDelete(null);
      if (expandedHistoryId === automationId) {
        setExpandedHistoryId(null);
      }
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not delete automation");
    } finally {
      setBusyId(null);
    }
  };

  const toggleHistory = (automationId: string) => {
    if (expandedHistoryId === automationId) {
      setExpandedHistoryId(null);
    } else {
      setExpandedHistoryId(automationId);
      if (!runsCache[automationId]) {
        void loadRuns(automationId);
      }
    }
  };

  const handleCopySummary = async (runId: string, summary: string) => {
    try {
      await navigator.clipboard.writeText(summary);
      setCopiedRunId(runId);
      setTimeout(() => setCopiedRunId(null), 2000);
    } catch {
      // Ignore clipboard write errors
    }
  };

  return (
    <section className="settings-panel" aria-label="Automations">
      <header className="settings-header">
        <div>
          <h1>Automations</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            Recurring background tasks run while Coreside is open. Inspect recent run history and persistent AI results.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          aria-label="Back to chat"
          onClick={onBack}
        >
          Back to chat
        </button>
      </header>

      <div className="settings-content">
        {error ? (
          <p className="provider-error" role="alert" style={{ marginBottom: "1rem" }}>
            {error}
          </p>
        ) : null}
        {info ? (
          <p
            className="muted"
            role="status"
            style={{
              padding: "0.5rem 0.75rem",
              borderRadius: "4px",
              background: "rgba(16, 185, 129, 0.1)",
              border: "1px solid rgba(16, 185, 129, 0.3)",
              color: "var(--text-primary)",
              marginBottom: "1rem",
            }}
          >
            {info}
          </p>
        ) : null}

        {rows.length === 0 ? (
          <div className="empty-state">
            <h3>{EMPTY_STATES.noAutomations.title}</h3>
            <p>{EMPTY_STATES.noAutomations.body}</p>
            <div className="button-row empty-state-actions">
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => {
                  onBack();
                  const { activeConversationId: activeId, createConversation: create } =
                    useAppStore.getState();
                  const focus = () =>
                    document.getElementById("composer-input")?.focus();
                  if (activeId) {
                    focus();
                  } else {
                    void create().then(focus);
                  }
                }}
              >
                {EMPTY_STATES.noAutomations.primaryCta}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => openHelpAndLearning(navigateToSettings)}
              >
                {EMPTY_STATES.helpLink.label}
              </button>
            </div>
          </div>
        ) : (
          <ul className="provider-connection-list" style={{ gap: "1rem" }}>
            {rows.map((row) => {
              const isBusy = busyId === row.id;
              const isExpanded = expandedHistoryId === row.id;
              const runs = runsCache[row.id] ?? [];
              const isLoadingRuns = loadingRunsId === row.id;

              return (
                <li
                  key={row.id}
                  className="provider-connection-item settings-section"
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "stretch",
                    gap: "0.75rem",
                    padding: "1rem",
                    border: "1px solid var(--border)",
                    borderRadius: "6px",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "flex-start",
                      flexWrap: "wrap",
                      gap: "0.75rem",
                    }}
                  >
                    <div style={{ flex: "1 1 300px" }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", flexWrap: "wrap" }}>
                        <strong style={{ fontSize: "1.05rem" }}>{row.name}</strong>
                        {!row.enabled ? (
                          <span
                            className="provider-active-tag"
                            style={{ background: "rgba(107, 114, 128, 0.16)", color: "var(--text-muted)" }}
                          >
                            Paused
                          </span>
                        ) : (
                          <span
                            className="provider-active-tag"
                            style={{ background: "rgba(16, 185, 129, 0.16)", color: "#34d399" }}
                          >
                            Active
                          </span>
                        )}
                        {row.requiresAi ? (
                          <span
                            className="provider-active-tag"
                            style={{ background: "rgba(168, 85, 247, 0.16)", color: "#c084fc" }}
                          >
                            Uses AI
                          </span>
                        ) : null}
                        {row.waitingApproval ? (
                          <span
                            className="provider-active-tag"
                            style={{ background: "rgba(245, 158, 11, 0.16)", color: "#fbbf24" }}
                          >
                            Waiting approval
                          </span>
                        ) : null}
                        {!row.permissionReady && row.applicationId ? (
                          <span
                            className="provider-active-tag"
                            style={{ background: "rgba(239, 68, 68, 0.16)", color: "#f87171" }}
                          >
                            Not ready
                          </span>
                        ) : null}
                        {row.lastStatus === "running" ? (
                          <span
                            className="provider-active-tag"
                            style={{ background: "rgba(59, 130, 246, 0.16)", color: "#60a5fa" }}
                          >
                            Running
                          </span>
                        ) : null}
                      </div>

                      <p className="muted" style={{ margin: "0.4rem 0 0", fontSize: "0.88rem" }}>
                        <strong>Schedule:</strong> {describeAutomationTrigger(row.trigger)}
                        {row.applicationId ? ` · Scoped to app: ${row.applicationId}` : ""}
                      </p>

                      <p className="muted" style={{ margin: "0.2rem 0 0", fontSize: "0.85rem" }}>
                        {row.nextRunAt ? (
                          <>Next run: {new Date(row.nextRunAt).toLocaleString()}</>
                        ) : (
                          <>Next run: Not scheduled</>
                        )}
                        {row.lastRunAt ? (
                          <> · Last run: {new Date(row.lastRunAt).toLocaleString()}</>
                        ) : (
                          <> · Last run: Never</>
                        )}
                        {row.lastStatus ? (
                          <>
                            {" "}
                            · Last outcome:{" "}
                            <span
                              style={{
                                fontWeight: 600,
                                color: getStatusBadgeStyle(row.lastStatus).color,
                              }}
                            >
                              {row.lastStatus}
                            </span>
                          </>
                        ) : null}
                        {row.consecutiveFailures > 0 ? (
                          <span style={{ color: "#f87171", marginLeft: "0.5rem" }}>
                            ({row.consecutiveFailures} consecutive failure{row.consecutiveFailures > 1 ? "s" : ""})
                          </span>
                        ) : null}
                      </p>
                    </div>

                    <div className="button-row" style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        disabled={isBusy}
                        onClick={() => handleRunNow(row)}
                      >
                        {isBusy ? "Running…" : "Run now"}
                      </button>

                      <button
                        type="button"
                        className="btn btn-secondary"
                        disabled={isBusy}
                        onClick={() => handleToggleEnabled(row)}
                      >
                        {row.enabled ? "Pause" : "Resume"}
                      </button>

                      <button
                        type="button"
                        className="btn btn-secondary"
                        aria-expanded={isExpanded}
                        onClick={() => toggleHistory(row.id)}
                      >
                        {isExpanded ? "Hide runs" : "Run history"}
                      </button>

                      {confirmDelete === row.id ? (
                        <>
                          <button
                            type="button"
                            className="btn btn-danger"
                            disabled={isBusy}
                            onClick={() => handleDelete(row.id)}
                          >
                            Confirm delete
                          </button>
                          <button
                            type="button"
                            className="btn btn-secondary"
                            disabled={isBusy}
                            onClick={() => setConfirmDelete(null)}
                          >
                            Cancel
                          </button>
                        </>
                      ) : (
                        <button
                          type="button"
                          className="btn btn-danger"
                          disabled={isBusy}
                          onClick={() => setConfirmDelete(row.id)}
                        >
                          Delete
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Run History Disclosure */}
                  {isExpanded ? (
                    <div
                      style={{
                        marginTop: "0.5rem",
                        padding: "0.75rem",
                        background: "var(--surface-secondary, rgba(0, 0, 0, 0.2))",
                        borderRadius: "6px",
                        border: "1px solid var(--border)",
                      }}
                      role="region"
                      aria-label={`Run history for ${row.name}`}
                    >
                      <div
                        style={{
                          display: "flex",
                          justifyContent: "space-between",
                          alignItems: "center",
                          marginBottom: "0.5rem",
                        }}
                      >
                        <h4 style={{ margin: 0, fontSize: "0.92rem", fontWeight: 600 }}>Recent Executions</h4>
                        <button
                          type="button"
                          className="btn btn-secondary"
                          style={{ fontSize: "0.78rem", padding: "0.2rem 0.5rem" }}
                          disabled={isLoadingRuns}
                          onClick={() => loadRuns(row.id)}
                        >
                          {isLoadingRuns ? "Refreshing…" : "Refresh history"}
                        </button>
                      </div>

                      {isLoadingRuns && runs.length === 0 ? (
                        <p className="muted" style={{ margin: "0.5rem 0", fontSize: "0.85rem" }}>
                          Loading run history…
                        </p>
                      ) : runs.length === 0 ? (
                        <p className="muted" style={{ margin: "0.5rem 0", fontSize: "0.85rem" }}>
                          No recorded runs yet for this automation. Use <strong>Run now</strong> to trigger an execution.
                        </p>
                      ) : (
                        <ul style={{ listStyle: "none", padding: 0, margin: 0, display: "flex", flexDirection: "column", gap: "0.5rem" }}>
                          {runs.map((run) => {
                            const badgeStyle = getStatusBadgeStyle(run.status);
                            const duration = formatDuration(run.startedAt, run.completedAt);
                            const hasSummary = Boolean(run.resultSummary && run.resultSummary.trim());

                            return (
                              <li
                                key={run.id}
                                style={{
                                  padding: "0.6rem",
                                  borderRadius: "4px",
                                  background: "var(--surface-primary, rgba(255, 255, 255, 0.03))",
                                  border: "1px solid var(--border)",
                                }}
                              >
                                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "0.5rem" }}>
                                  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                                    <span
                                      style={{
                                        display: "inline-block",
                                        padding: "0.15rem 0.45rem",
                                        borderRadius: "999px",
                                        fontSize: "0.72rem",
                                        fontWeight: 600,
                                        background: badgeStyle.background,
                                        color: badgeStyle.color,
                                        textTransform: "capitalize",
                                      }}
                                    >
                                      {run.status}
                                    </span>
                                    <span style={{ fontSize: "0.85rem", color: "var(--text-primary)" }}>
                                      {new Date(run.startedAt).toLocaleString()}
                                    </span>
                                    {duration ? (
                                      <span className="muted" style={{ fontSize: "0.78rem" }}>
                                        ({duration})
                                      </span>
                                    ) : null}
                                  </div>

                                  {hasSummary ? (
                                    <button
                                      type="button"
                                      className="btn btn-secondary"
                                      style={{ fontSize: "0.75rem", padding: "0.15rem 0.45rem" }}
                                      onClick={() => handleCopySummary(run.id, run.resultSummary!)}
                                    >
                                      {copiedRunId === run.id ? "Copied!" : "Copy result"}
                                    </button>
                                  ) : null}
                                </div>

                                {run.errorCategory ? (
                                  <p style={{ margin: "0.35rem 0 0", fontSize: "0.82rem", color: "#f87171" }}>
                                    Error: {run.errorCategory}
                                  </p>
                                ) : null}

                                {hasSummary ? (
                                  <div
                                    style={{
                                      marginTop: "0.45rem",
                                      padding: "0.5rem 0.65rem",
                                      background: "var(--surface-elevated, rgba(0, 0, 0, 0.35))",
                                      borderRadius: "4px",
                                      border: "1px solid var(--border)",
                                      fontSize: "0.84rem",
                                      lineHeight: 1.45,
                                      whiteSpace: "pre-wrap",
                                      wordBreak: "break-word",
                                      color: "var(--text-primary)",
                                    }}
                                  >
                                    {run.resultSummary}
                                  </div>
                                ) : null}
                              </li>
                            );
                          })}
                        </ul>
                      )}
                    </div>
                  ) : null}
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
