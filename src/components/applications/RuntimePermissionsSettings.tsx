import { useCallback, useEffect, useState } from "react";
import { ApprovalCard } from "@/components/applications/ApprovalCard";
import { api } from "@/lib/tauri";
import type {
  ApprovalRequest,
  ManifestRecord,
  RuntimeGrant,
} from "@/types/application-kernel";

export function RuntimePermissionsSettings() {
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);
  const [grants, setGrants] = useState<RuntimeGrant[]>([]);
  const [appNames, setAppNames] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [pending, runtimeGrants, manifests] = await Promise.all([
        api.kernelListPendingApprovals(),
        api.kernelListRuntimeGrants(),
        api.kernelListManifests().catch(() => [] as ManifestRecord[]),
      ]);
      setApprovals(pending);
      setGrants(runtimeGrants);
      const names: Record<string, string> = {};
      for (const record of manifests) {
        names[record.applicationId] = record.manifest.name;
      }
      setAppNames(names);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load permissions");
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const revoke = async (grantId: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.kernelRevokeRuntimeGrant(grantId);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not revoke grant");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="settings-section" aria-labelledby="runtime-permissions-heading">
      <h3 id="runtime-permissions-heading">Runtime permissions</h3>
      <p>
        Review pending action approvals and remembered grants for generated
        applications. Destructive and critical actions always require your
        confirmation.
      </p>

      {error ? (
        <p className="provider-error" role="alert">
          {error}
        </p>
      ) : null}

      <h4 className="settings-subheading">Pending approvals</h4>
      {approvals.length === 0 ? (
        <p className="muted">No pending approvals.</p>
      ) : (
        <div className="approval-settings-stack">
          {approvals.map((approval) => (
            <ApprovalCard
              key={approval.id}
              approval={approval}
              applicationName={
                approval.applicationId
                  ? appNames[approval.applicationId] ?? null
                  : null
              }
              onResolved={() => void reload()}
            />
          ))}
        </div>
      )}

      <h4 className="settings-subheading" style={{ marginTop: "1rem" }}>
        Remembered grants
      </h4>
      {grants.length === 0 ? (
        <p className="muted">No active remembered grants.</p>
      ) : (
        <ul className="provider-connection-list">
          {grants.map((grant) => (
            <li key={grant.id} className="provider-connection-item">
              <div>
                <strong>{grant.actionName}</strong>
                <p className="muted">
                  {grant.applicationId
                    ? appNames[grant.applicationId] ?? grant.applicationId
                    : "Global"}{" "}
                  · {grant.scopeKind} · {grant.duration}
                </p>
              </div>
              <button
                type="button"
                className="btn btn-secondary"
                disabled={busy}
                onClick={() => void revoke(grant.id)}
              >
                Revoke
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
