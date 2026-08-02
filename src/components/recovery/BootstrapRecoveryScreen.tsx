import { useState } from "react";
import { api } from "@/lib/tauri";
import type { BootstrapStatus } from "@/types/bootstrap";
import { useAppStore } from "@/stores/app-store";

/**
 * Protected startup shell when the profile database cannot open.
 * Does not depend on generated surfaces.
 */
export function BootstrapRecoveryScreen({
  status,
}: {
  status: Extract<BootstrapStatus, { status: "recoveryRequired" }>;
}) {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const retry = async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await api.retryOpenDatabase();
      if (next.status === "ready") {
        await bootstrap();
        return;
      }
      // Rust returns an updated recovery status (not an exception) when reopen fails.
      useAppStore.setState({ bootstrapStatus: next });
      setError(next.message);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reopen local data.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <main
      className="empty-state bootstrap-recovery"
      style={{ minHeight: "100%" }}
      aria-labelledby="bootstrap-recovery-title"
      aria-busy={busy}
    >
      <h2 id="bootstrap-recovery-title">Coreside needs Recovery</h2>
      <p>{status.message}</p>
      <p className="muted" role="status">
        Reason: {status.reasonCode}
        {status.usingShellDatabase
          ? " · Temporary recovery shell is active (your profile data was not opened)."
          : null}
      </p>
      {error ? (
        <p className="muted" role="alert">
          {error}
        </p>
      ) : null}
      <div className="button-row" style={{ marginTop: "1rem" }}>
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy}
          aria-busy={busy}
          onClick={() => void retry()}
        >
          {busy ? "Retrying…" : "Try again"}
        </button>
      </div>
      <p className="muted" style={{ marginTop: "1.25rem", maxWidth: "32rem" }}>
        Your original database file is preserved when possible. Local backup
        snapshots are available from Data &amp; Storage after Recovery succeeds;
        full restore is not yet available in this build. Contact support with a
        redacted diagnostic export if you need help.
      </p>
    </main>
  );
}
