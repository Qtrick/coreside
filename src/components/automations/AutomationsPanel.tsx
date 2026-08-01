import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";

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
  trigger: { type: string; intervalMinutes?: number; time?: string };
};

export function AutomationsPanel({ onBack }: { onBack: () => void }) {
  const [rows, setRows] = useState<Automation[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const reload = async () => {
    try {
      const raw = await api.listAutomations();
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
          trigger: (row.trigger as Automation["trigger"]) ?? { type: "interval" },
        })),
      );
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load automations");
    }
  };

  useEffect(() => {
    void reload();
  }, []);

  return (
    <section className="settings-panel" aria-label="Automations">
      <header className="settings-header">
        <div>
          <h1>Automations</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            Recurring changes run while Coreside is open. Missed intervals catch
            up at most once.
          </p>
        </div>
        <button type="button" className="btn btn-secondary" onClick={onBack}>
          Back to chat
        </button>
      </header>
      <div className="settings-content">
        {error ? (
          <p className="provider-error" role="alert">
            {error}
          </p>
        ) : null}
        {rows.length === 0 ? (
          <div className="settings-section">
            <p>
              Recurring changes and routines you create with Coreside will appear
              here.
            </p>
          </div>
        ) : (
          <ul className="provider-connection-list">
            {rows.map((row) => (
              <li key={row.id} className="provider-connection-item settings-section">
                <div>
                  <strong>
                    {row.name}
                    {!row.enabled ? (
                      <span className="provider-active-tag">Paused</span>
                    ) : null}
                    {row.requiresAi ? (
                      <span className="provider-active-tag">Uses AI credits</span>
                    ) : null}
                    {row.waitingApproval ? (
                      <span className="provider-active-tag">Waiting approval</span>
                    ) : null}
                    {!row.permissionReady && row.applicationId ? (
                      <span className="provider-active-tag">Not ready</span>
                    ) : null}
                  </strong>
                  <p className="muted">
                    {row.trigger.type}
                    {row.applicationId ? ` · app ${row.applicationId}` : ""}
                    {row.trigger.intervalMinutes
                      ? ` · every ${row.trigger.intervalMinutes} min`
                      : ""}
                    {row.nextRunAt
                      ? ` · next ${new Date(row.nextRunAt).toLocaleString()}`
                      : ""}
                    {row.lastStatus ? ` · last ${row.lastStatus}` : ""}
                  </p>
                </div>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={busyId === row.id}
                    onClick={() => {
                      setBusyId(row.id);
                      void api
                        .runAutomationNow(row.id)
                        .then(() => reload())
                        .finally(() => setBusyId(null));
                    }}
                  >
                    Run now
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => {
                      setBusyId(row.id);
                      void api
                        .setAutomationEnabled(row.id, !row.enabled)
                        .then(() => reload())
                        .finally(() => setBusyId(null));
                    }}
                  >
                    {row.enabled ? "Pause" : "Resume"}
                  </button>
                  {confirmDelete === row.id ? (
                    <>
                      <button
                        type="button"
                        className="btn btn-danger"
                        onClick={() => {
                          void api.deleteAutomation(row.id).then(() => reload());
                          setConfirmDelete(null);
                        }}
                      >
                        Confirm delete
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => setConfirmDelete(null)}
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-danger"
                      onClick={() => setConfirmDelete(row.id)}
                    >
                      Delete
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
