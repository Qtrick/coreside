import { useAppStore } from "@/stores/app-store";

/** Multiwindow / revision conflict banner. */
export function ConflictBanner() {
  const conflict = useAppStore((s) => s.appConflict);
  const clear = useAppStore((s) => s.clearAppConflict);
  const reload = useAppStore((s) => s.reloadActiveSurfaces);

  if (!conflict) return null;

  return (
    <div
      role="alert"
      style={{
        margin: "0.5rem 0.75rem",
        padding: "0.75rem 1rem",
        borderRadius: 10,
        border: "1px solid var(--border)",
        background: "var(--surface-muted, var(--surface))",
      }}
    >
      <strong>Update conflict</strong>
      <p style={{ margin: "0.35rem 0 0" }}>{conflict.message}</p>
      {conflict.conflicts.length > 0 ? (
        <ul className="muted" style={{ margin: "0.35rem 0 0", paddingLeft: "1.1rem" }}>
          {conflict.conflicts.slice(0, 4).map((c) => (
            <li key={c}>{c}</li>
          ))}
        </ul>
      ) : null}
      <div className="button-row" style={{ marginTop: "0.65rem" }}>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => {
            void reload();
            clear();
          }}
        >
          Reload current version
        </button>
        <button type="button" className="btn btn-secondary" onClick={() => clear()}>
          Dismiss
        </button>
      </div>
    </div>
  );
}
