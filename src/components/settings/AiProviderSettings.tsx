import { useEffect, useState } from "react";
import { CheckCircle2, PlugZap } from "lucide-react";
import { api } from "@/lib/tauri";
import type { ProviderConnection } from "@/types/providers";
import { useAppStore } from "@/stores/app-store";

function sourceCopy(source?: string | null): string | null {
  if (source === "env") return "Using development environment credential";
  if (source === "connection") return null;
  return null;
}

export function AiProviderSettings() {
  const aiStatus = useAppStore((s) => s.aiStatus);
  const testingConnection = useAppStore((s) => s.testingConnection);
  const connectionTestMessage = useAppStore((s) => s.connectionTestMessage);
  const testConnection = useAppStore((s) => s.testConnection);
  const openProviderSetup = useAppStore((s) => s.openProviderSetup);
  const refreshAiStatus = useAppStore((s) => s.refreshAiStatus);

  const [connections, setConnections] = useState<ProviderConnection[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const reload = async () => {
    try {
      const rows = await api.listProviderConnections();
      setConnections(rows);
      setListError(null);
    } catch (err) {
      setListError(
        err instanceof Error ? err.message : "Could not load providers.",
      );
    }
  };

  useEffect(() => {
    void reload();
  }, [aiStatus?.activeConnectionId, aiStatus?.status]);

  const statusClass =
    aiStatus?.status === "ready"
      ? "ready"
      : aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured"
        ? "warn"
        : "";

  const envNote = sourceCopy(aiStatus?.source);

  return (
    <section className="settings-section settings-section-compact" aria-labelledby="ai-providers-heading">
      <h3 id="ai-providers-heading">AI Providers</h3>
      <div className="provider-status-row">
        <span className={`status-pill ${statusClass}`}>
          {aiStatus?.status === "ready" ? (
            <CheckCircle2 size={14} aria-hidden />
          ) : (
            <PlugZap size={14} aria-hidden />
          )}
          {aiStatus?.status === "ready" ? "Connected" : (aiStatus?.status ?? "unknown")}
        </span>
        <div className="provider-status-meta">
          <div>
            <span className="muted">Active provider</span>
            <strong>{aiStatus?.provider ?? "—"}</strong>
          </div>
          <div>
            <span className="muted">Current model</span>
            <strong>{aiStatus?.model ?? "—"}</strong>
          </div>
        </div>
      </div>
      {envNote ? <p className="muted">{envNote}</p> : null}
      {aiStatus?.message && aiStatus.status !== "ready" ? (
        <p className="muted">{aiStatus.message}</p>
      ) : null}

      <div className="button-row">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => openProviderSetup()}
        >
          Manage providers
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => void testConnection()}
          disabled={testingConnection}
        >
          {testingConnection ? "Testing…" : "Test connection"}
        </button>
      </div>
      {connectionTestMessage ? (
        <p role="status" className="muted">
          {connectionTestMessage}
        </p>
      ) : null}

      {listError ? (
        <p className="provider-error" role="alert">
          {listError}
        </p>
      ) : null}

      {connections.length > 0 ? (
        <ul className="provider-connection-list">
          {connections.map((conn) => (
            <li key={conn.id} className="provider-connection-item">
              <div>
                <strong>
                  {conn.label}
                  {conn.isActive ? (
                    <span className="provider-active-tag">Active</span>
                  ) : null}
                </strong>
                <p className="muted">
                  {conn.provider}
                  {conn.modelDefault ? ` · ${conn.modelDefault}` : ""}
                  {conn.lastStatus ? ` · ${conn.lastStatus}` : ""}
                </p>
              </div>
              <div className="button-row">
                {!conn.isActive ? (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={busyId === conn.id}
                    onClick={() => {
                      setBusyId(conn.id);
                      void api
                        .setActiveProviderConnection(conn.id)
                        .then(() => refreshAiStatus())
                        .then(() => reload())
                        .finally(() => setBusyId(null));
                    }}
                  >
                    Use
                  </button>
                ) : null}
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => openProviderSetup(conn)}
                >
                  Edit
                </button>
                {confirmDeleteId === conn.id ? (
                  <>
                    <button
                      type="button"
                      className="btn btn-danger"
                      disabled={busyId === conn.id}
                      onClick={() => {
                        setBusyId(conn.id);
                        void api
                          .deleteProviderConnection(conn.id)
                          .then(() => refreshAiStatus())
                          .then(() => reload())
                          .finally(() => {
                            setBusyId(null);
                            setConfirmDeleteId(null);
                          });
                      }}
                    >
                      Confirm remove
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => setConfirmDeleteId(null)}
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    className="btn btn-danger"
                    onClick={() => setConfirmDeleteId(conn.id)}
                  >
                    Remove
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted" style={{ marginBottom: 0 }}>
          No saved providers yet. Connect one to start chatting.
        </p>
      )}
    </section>
  );
}
