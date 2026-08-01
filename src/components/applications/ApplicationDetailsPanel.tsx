import { X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ModalPortal } from "@/components/ui/ModalPortal";
import { api } from "@/lib/tauri";
import type {
  ApplicationVersion,
  AuditEvent,
  BuildFailure,
  ManifestRecord,
  RuntimeGrant,
} from "@/types/application-kernel";

type ApplicationDetailsPanelProps = {
  applicationId: string;
  open: boolean;
  onClose: () => void;
  fallbackName?: string;
};

export function ApplicationDetailsPanel({
  applicationId,
  open,
  onClose,
  fallbackName,
}: ApplicationDetailsPanelProps) {
  const [record, setRecord] = useState<ManifestRecord | null>(null);
  const [summary, setSummary] = useState<Record<string, unknown> | null>(null);
  const [grants, setGrants] = useState<RuntimeGrant[]>([]);
  const [audit, setAudit] = useState<AuditEvent[]>([]);
  const [versions, setVersions] = useState<ApplicationVersion[]>([]);
  const [failures, setFailures] = useState<BuildFailure[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    if (!open || !applicationId) return;
    try {
      const [manifest, appSummary, runtimeGrants, events, vers, fails] =
        await Promise.all([
          api.kernelGetManifest(applicationId),
          api.kernelApplicationSummary(applicationId).catch(() => null),
          api.kernelListRuntimeGrants(applicationId).catch(() => []),
          api.kernelListAuditEvents(applicationId, 12).catch(() => []),
          api.kernelListApplicationVersions(applicationId).catch(() => []),
          api.kernelListBuildFailures(applicationId).catch(() => []),
        ]);
      setRecord(manifest);
      setSummary(appSummary);
      setGrants(runtimeGrants);
      setAudit(events);
      setVersions(vers);
      setFailures(fails);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load application");
    }
  }, [applicationId, open]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Action failed");
    } finally {
      setBusy(false);
    }
  };

  if (!open) return null;

  const name =
    record?.manifest.name ?? fallbackName ?? applicationId;
  const permissions = record?.manifest.permissions ?? [];
  const actionAccess = record?.manifest.applicationActionAccess ?? [];

  return (
    <ModalPortal>
      <div className="provider-modal-root approval-details-root">
        <button
          type="button"
          className="provider-modal-backdrop"
          aria-label="Close application details"
          onClick={onClose}
        />
        <section
          className="provider-modal project-dialog application-details-panel"
          aria-labelledby="application-details-title"
        >
          <header className="provider-modal-header">
            <div>
              <h2 id="application-details-title">{name}</h2>
              <p className="muted">{applicationId}</p>
            </div>
            <button
              type="button"
              className="icon-btn"
              aria-label="Close"
              onClick={onClose}
            >
              <X size={18} />
            </button>
          </header>

          {error ? (
            <p className="provider-error" role="alert">
              {error}
            </p>
          ) : null}

          {!record ? (
            <p className="muted">Loading application details…</p>
          ) : (
            <div className="application-details-body">
              <section className="settings-section settings-section-compact">
                <h3>Status</h3>
                <ul className="muted application-details-list">
                  <li>Health: {record.healthState}</li>
                  <li>Lifecycle: {record.lifecycleState}</li>
                  <li>Version: {record.currentVersion}</li>
                  <li>
                    Last known good:{" "}
                    {record.lastKnownGoodVersion ?? "Not recorded"}
                  </li>
                  <li>Disabled: {record.disabled ? "Yes" : "No"}</li>
                  {summary?.recordCount != null ? (
                    <li>Data records: {String(summary.recordCount)}</li>
                  ) : null}
                </ul>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={busy || record.lastKnownGoodVersion == null}
                    onClick={() =>
                      void run(() => api.kernelRestoreLastKnownGood(applicationId))
                    }
                  >
                    Restore last known good
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={busy}
                    onClick={() =>
                      void run(() =>
                        api.kernelSetApplicationLifecycle(
                          applicationId,
                          record.disabled,
                        ),
                      )
                    }
                  >
                    {record.disabled ? "Enable application" : "Disable application"}
                  </button>
                </div>
              </section>

              <section className="settings-section settings-section-compact">
                <h3>Permissions</h3>
                {permissions.length === 0 ? (
                  <p className="muted">No declared permissions.</p>
                ) : (
                  <ul className="application-details-list">
                    {permissions.map((permission) => (
                      <li key={permission}>
                        <code>{permission}</code>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="settings-section settings-section-compact">
                <h3>Action access</h3>
                {actionAccess.length === 0 ? (
                  <p className="muted">No registered actions declared.</p>
                ) : (
                  <ul className="application-details-list">
                    {actionAccess.map((action) => (
                      <li key={action}>
                        <code>{action}</code>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="settings-section settings-section-compact">
                <h3>Remembered grants</h3>
                {grants.length === 0 ? (
                  <p className="muted">No active runtime grants.</p>
                ) : (
                  <ul className="provider-connection-list">
                    {grants.map((grant) => (
                      <li key={grant.id} className="provider-connection-item">
                        <div>
                          <strong>{grant.actionName}</strong>
                          <p className="muted">
                            {grant.scopeKind} · {grant.duration}
                          </p>
                        </div>
                        <button
                          type="button"
                          className="btn btn-secondary"
                          disabled={busy}
                          onClick={() =>
                            void run(() => api.kernelRevokeRuntimeGrant(grant.id))
                          }
                        >
                          Revoke
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="settings-section settings-section-compact">
                <h3>Recent audit</h3>
                {audit.length === 0 ? (
                  <p className="muted">No audit events yet.</p>
                ) : (
                  <ul className="application-details-list muted">
                    {audit.map((event) => (
                      <li key={event.id}>
                        {event.actionName ?? event.kind} · {event.outcome} ·{" "}
                        {new Date(event.createdAt).toLocaleString()}
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="settings-section settings-section-compact">
                <h3>Versions</h3>
                {versions.length === 0 ? (
                  <p className="muted">No version history.</p>
                ) : (
                  <ul className="application-details-list muted">
                    {versions.map((version) => (
                      <li key={version.version}>
                        v{version.version}
                        {version.isKnownGood ? " · known good" : ""} ·{" "}
                        {version.validationStatus} / {version.testStatus}
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="settings-section settings-section-compact">
                <h3>Build failures</h3>
                {failures.length === 0 ? (
                  <p className="muted">No recent build failures.</p>
                ) : (
                  <>
                    <ul className="application-details-list">
                      {failures.map((failure) => (
                        <li key={failure.id}>
                          <p>{failure.safeMessage}</p>
                          <p className="muted">
                            {new Date(failure.createdAt).toLocaleString()}
                            {failure.retryable ? " · retryable" : ""}
                          </p>
                        </li>
                      ))}
                    </ul>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={busy}
                      onClick={() =>
                        void run(() => api.kernelClearBuildFailure(applicationId))
                      }
                    >
                      Clear failures
                    </button>
                  </>
                )}
              </section>
            </div>
          )}
        </section>
      </div>
    </ModalPortal>
  );
}
