import { useAppStore } from "@/stores/app-store";

export function ToolChangePreview() {
  const pending = useAppStore((s) => s.pendingToolChange);
  const applyPendingToolChange = useAppStore((s) => s.applyPendingToolChange);
  const discardPendingToolChange = useAppStore((s) => s.discardPendingToolChange);

  if (!pending) return null;

  const { toolChange } = pending;
  const actionLabel =
    toolChange.action === "create"
      ? "Create tool"
      : toolChange.action === "update"
        ? "Update tool"
        : "Replace tool";

  return (
    <div className="tool-change-preview" role="region" aria-label="Tool change preview">
      <h3>
        {actionLabel}: {toolChange.tool.name}
      </h3>
      <p className="muted" style={{ margin: 0 }}>
        {toolChange.changeSummary || toolChange.tool.description || "Review this change before applying."}
      </p>
      <div className="preview-actions">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void applyPendingToolChange()}
        >
          Apply
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => void discardPendingToolChange()}
        >
          Discard
        </button>
      </div>
    </div>
  );
}
