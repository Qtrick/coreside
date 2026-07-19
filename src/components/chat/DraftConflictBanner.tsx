import { useAppStore } from "@/stores/app-store";

export function DraftConflictBanner() {
  const conflict = useAppStore((s) => s.surfaceDraftConflict);
  const resolveDraftConflict = useAppStore((s) => s.resolveSurfaceDraftConflict);
  const clearDraftConflict = useAppStore((s) => s.clearSurfaceDraftConflict);

  if (!conflict) return null;

  return (
    <div className="draft-conflict-banner" role="alert">
      <p>
        Agent changes conflict with your draft on component{" "}
        <strong>{conflict.componentId}</strong>.
      </p>
      <div className="draft-conflict-actions">
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => void resolveDraftConflict("keep")}
        >
          Keep mine
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void resolveDraftConflict("apply")}
        >
          Apply agent
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={() => clearDraftConflict()}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
