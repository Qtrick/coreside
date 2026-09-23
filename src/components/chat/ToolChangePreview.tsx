import { useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import type { ToolState } from "@/types/tool";

export function ToolChangePreview() {
  const pending = useAppStore((s) => s.pendingToolChange);
  const applyPendingToolChange = useAppStore((s) => s.applyPendingToolChange);
  const discardPendingToolChange = useAppStore((s) => s.discardPendingToolChange);
  const [previewState, setPreviewState] = useState<ToolState>({});

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

      {toolChange.tool ? (
        <div
          className="tool-change-visual-preview"
          style={{
            marginTop: "0.75rem",
            padding: "0.75rem",
            borderRadius: "var(--radius-md)",
            border: "1px dashed var(--border)",
            background: "var(--core-card-overlay)",
            maxHeight: "360px",
            overflowY: "auto",
          }}
        >
          <ToolRenderer
            tool={toolChange.tool}
            state={previewState}
            mode="preview"
            onStateChange={(nextState) => setPreviewState(nextState)}
          />
        </div>
      ) : null}

      <div className="preview-actions" style={{ marginTop: "0.75rem" }}>
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
