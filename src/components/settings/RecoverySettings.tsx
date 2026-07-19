import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { ManifestRecord, RecoveryState } from "@/types/application-kernel";

/**
 * Protected Recovery Mode controls.
 * Must not depend on generated surfaces or capability packs.
 */
export function RecoverySettings() {
  const [state, setState] = useState<RecoveryState | null>(null);
  const [apps, setApps] = useState<ManifestRecord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      const [r, m] = await Promise.all([
        api.kernelGetRecoveryState(),
        api.kernelListManifests().catch(() => [] as ManifestRecord[]),
      ]);
      setState(r);
      setApps(m);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not load recovery state");
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Recovery action failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="settings-section" aria-labelledby="recovery-heading">
      <h3 id="recovery-heading">Recovery</h3>
      <p>
        Recovery Mode is protected. It can disable user-created surfaces and
        restore last working application versions without deleting chats,
        projects, or credentials.
      </p>
      {error ? (
        <p className="muted" role="alert">
          {error}
        </p>
      ) : null}
      {state ? (
        <ul className="muted" style={{ marginTop: "0.5rem" }}>
          <li>Recovery Mode: {state.recoveryMode ? "On" : "Off"}</li>
          <li>
            User surfaces disabled: {state.disableUserSurfaces ? "Yes" : "No"}
          </li>
          <li>
            Custom layouts disabled: {state.disableCustomLayouts ? "Yes" : "No"}
          </li>
          <li>
            Capability packs disabled:{" "}
            {state.disableCapabilityPacks ? "Yes" : "No"}
          </li>
        </ul>
      ) : (
        <p className="muted">Loading…</p>
      )}
      <div className="button-row" style={{ marginTop: "0.75rem", flexWrap: "wrap" }}>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() => void run(() => api.kernelSetRecoveryMode(true))}
        >
          Enter Recovery Mode
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() =>
            void run(() =>
              api.kernelSetRecoveryFlags({
                disableUserSurfaces: true,
                disableCustomLayouts: true,
                disableCapabilityPacks: false,
              }),
            )
          }
        >
          Disable user surfaces
        </button>
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy}
          onClick={() => void run(() => api.kernelClearRecovery())}
        >
          Exit Recovery Mode
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() => void run(() => api.kernelGarbageCollect())}
        >
          Clean temporary resources
        </button>
      </div>

      {apps.length > 0 ? (
        <div style={{ marginTop: "1rem" }}>
          <h4 style={{ marginBottom: "0.5rem" }}>Applications</h4>
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {apps.map((app) => (
              <li
                key={app.applicationId}
                style={{
                  display: "flex",
                  gap: "0.75rem",
                  alignItems: "center",
                  marginBottom: "0.5rem",
                  flexWrap: "wrap",
                }}
              >
                <span>
                  <strong>{app.manifest.name}</strong>{" "}
                  <span className="muted">
                    v{app.currentVersion} · {app.healthState} ·{" "}
                    {app.lifecycleState}
                    {app.lastKnownGoodVersion != null
                      ? ` · LKG v${app.lastKnownGoodVersion}`
                      : ""}
                  </span>
                </span>
                {app.lastKnownGoodVersion != null ? (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={busy}
                    onClick={() =>
                      void run(() =>
                        api.kernelRestoreLastKnownGood(app.applicationId),
                      )
                    }
                  >
                    Restore last working version
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
