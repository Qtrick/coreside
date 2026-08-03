import { useEffect, useState } from "react";
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
  const [backups, setBackups] = useState<Array<{ path: string; byteSize: number }>>(
    [],
  );
  const [selectedBackup, setSelectedBackup] = useState<string>("");
  const [previewNote, setPreviewNote] = useState<string | null>(null);
  const [confirmRestore, setConfirmRestore] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void api
      .listManagedBackups()
      .then((rows) => {
        if (!cancelled) {
          setBackups(rows);
          if (rows[0]) setSelectedBackup(rows[0].path);
        }
      })
      .catch(() => {
        if (!cancelled) setBackups([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const retry = async () => {
    setBusy(true);
    setError(null);
    try {
      const next = await api.retryOpenDatabase();
      if (next.status === "ready") {
        await bootstrap();
        return;
      }
      useAppStore.setState({ bootstrapStatus: next });
      setError(next.message);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reopen local data.");
    } finally {
      setBusy(false);
    }
  };

  const previewSelected = async () => {
    if (!selectedBackup) return;
    setBusy(true);
    setError(null);
    setPreviewNote(null);
    try {
      const preview = await api.previewRestoreBackup(selectedBackup);
      setPreviewNote(
        `Backup from ${preview.createdAt} · schema ${preview.latestMigration ?? "unknown"} · ${preview.integrityOk ? "integrity ok" : "integrity failed"}${
          preview.warnings.length ? ` · ${preview.warnings.join(" ")}` : ""
        }`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not preview backup.");
    } finally {
      setBusy(false);
    }
  };

  const restoreSelected = async () => {
    if (!selectedBackup || !confirmRestore) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.restoreProfileBackup(selectedBackup, true);
      setConfirmRestore(false);
      setPreviewNote(
        `Restored from ${result.restoredFrom}. Restart Coreside to finish loading the restored profile.`,
      );
      // Prefer a clean process after disaster restore (scheduler/event bus already refreshed in Rust).
      await bootstrap();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Restore failed.");
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

      <section
        className="settings-section"
        style={{ marginTop: "1.5rem", maxWidth: "36rem", textAlign: "left" }}
        aria-labelledby="recovery-restore-heading"
      >
        <h3 id="recovery-restore-heading">Restore from backup</h3>
        <p className="muted">
          Choose a managed Coreside backup. Preview does not change your data.
          Restore replaces the broken profile and requires confirmation.
        </p>
        {backups.length === 0 ? (
          <p className="muted">No managed backups found yet.</p>
        ) : (
          <>
            <label htmlFor="recovery-backup-select">Backup</label>
            <select
              id="recovery-backup-select"
              value={selectedBackup}
              disabled={busy}
              onChange={(e) => {
                setSelectedBackup(e.target.value);
                setConfirmRestore(false);
                setPreviewNote(null);
              }}
            >
              {backups.map((b) => (
                <option key={b.path} value={b.path}>
                  {b.path} ({Math.round(b.byteSize / 1024)} KB)
                </option>
              ))}
            </select>
            <div className="button-row" style={{ marginTop: "0.75rem" }}>
              <button
                type="button"
                className="btn btn-secondary"
                disabled={busy || !selectedBackup}
                onClick={() => void previewSelected()}
              >
                Preview
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                disabled={busy || !selectedBackup}
                onClick={() => setConfirmRestore(true)}
              >
                Restore…
              </button>
            </div>
            {previewNote ? (
              <p className="muted" role="status">
                {previewNote}
              </p>
            ) : null}
            {confirmRestore ? (
              <div role="alertdialog" aria-labelledby="confirm-restore-title">
                <p id="confirm-restore-title">
                  Replace the broken profile with this backup? This cannot be
                  undone from Recovery without another backup.
                </p>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={busy}
                    onClick={() => void restoreSelected()}
                  >
                    Confirm restore
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={busy}
                    onClick={() => setConfirmRestore(false)}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            ) : null}
          </>
        )}
      </section>
    </main>
  );
}
