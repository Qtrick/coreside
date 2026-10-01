import { useMemo, useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import type { ToolState } from "@/types/tool";
import { deriveEvolutionCopy } from "@/lib/application-proposal-status";

export function ToolChangePreview() {
  const pending = useAppStore((s) => s.pendingToolChange);
  const kernelPending = useAppStore((s) => s.pendingKernelProposal);
  const applyPendingToolChange = useAppStore((s) => s.applyPendingToolChange);
  const discardPendingToolChange = useAppStore((s) => s.discardPendingToolChange);
  const [previewState, setPreviewState] = useState<ToolState>({});
  const [busy, setBusy] = useState(false);

  const toolChange = pending?.toolChange;
  const evolution = useMemo(
    () =>
      toolChange
        ? deriveEvolutionCopy(
            [{ type: `tool.${toolChange.action}` }],
            toolChange.changeSummary ?? "",
            toolChange.tool.description ?? toolChange.tool.name,
          )
        : null,
    [toolChange],
  );

  // Kernel proposal is authoritative when present for the same turn.
  if (!pending || (kernelPending && kernelPending.messageId === pending.messageId)) {
    return null;
  }

  if (!toolChange || !evolution) {
    return null;
  }

  const apply = async () => {
    setBusy(true);
    try {
      await applyPendingToolChange();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tool-change-preview" role="region" aria-label="Application change preview">
      <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem", alignItems: "center" }}>
        <span className="proposal-status-badge active">Review changes</span>
        <span className="muted" style={{ fontSize: "0.8rem" }}>Preview ready</span>
      </div>

      <h3 style={{ marginTop: "0.5rem" }}>{evolution.headline}</h3>

      <div style={{ marginTop: "0.5rem" }}>
        <p className="muted" style={{ margin: 0, fontSize: "0.8rem", fontWeight: 600 }}>
          What will change
        </p>
        <p className="muted" style={{ margin: "0.25rem 0 0" }}>
          {evolution.whatWillChange}
        </p>
      </div>

      <div style={{ marginTop: "0.5rem" }}>
        <p className="muted" style={{ margin: 0, fontSize: "0.8rem", fontWeight: 600 }}>
          What will be preserved
        </p>
        <p className="muted" style={{ margin: "0.25rem 0 0" }}>
          {evolution.whatWillBePreserved}
        </p>
      </div>

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
          disabled={busy}
          aria-busy={busy}
          onClick={() => void apply()}
        >
          {busy ? "Building…" : "Apply"}
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() => void discardPendingToolChange()}
        >
          Discard
        </button>
      </div>
    </div>
  );
}
