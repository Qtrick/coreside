import { ExternalLink, History, X } from "lucide-react";
import { useCallback } from "react";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { useAppStore } from "@/stores/app-store";

export function ToolCanvas() {
  const {
    activeTool,
    toolState,
    updateToolState,
    closeToolCanvas,
    openToolWindow,
    undoTool,
    sendMessage,
  } = useAppStore();

  const onStateChange = useCallback(
    (state: Record<string, unknown>) => {
      void updateToolState(state, true);
    },
    [updateToolState],
  );

  const onSubmitToAgent = useCallback(
    (payload: {
      toolId: string;
      eventName: string;
      componentId?: string;
      values: Record<string, unknown>;
    }) => {
      const summary = [
        `Tool interaction: ${payload.eventName}`,
        `Tool: ${payload.toolId}`,
        payload.componentId ? `Component: ${payload.componentId}` : null,
        `Values: ${JSON.stringify(payload.values)}`,
      ]
        .filter(Boolean)
        .join("\n");
      void sendMessage(summary);
    },
    [sendMessage],
  );

  if (!activeTool) {
    return (
      <section className="tool-canvas" aria-label="Tool canvas">
        <div className="empty-state">
          <h3>No active tool</h3>
          <p>Ask the agent to create a tool, or select one from the sidebar.</p>
        </div>
      </section>
    );
  }

  return (
    <section className="tool-canvas" aria-label={`${activeTool.name} canvas`}>
      <header className="tool-canvas-header">
        <div>
          <h2>{activeTool.name}</h2>
          <div className="tool-meta">
            <span>{activeTool.description || "Personal tool"}</span>
            <span>v{activeTool.version ?? 1}</span>
          </div>
        </div>
        <div className="tool-header-actions">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void openToolWindow()}
            aria-label="Open tool in new window"
          >
            <ExternalLink size={16} aria-hidden />
            Open
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void undoTool()}
            aria-label="Undo last tool change"
          >
            <History size={16} aria-hidden />
            Undo
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={closeToolCanvas}
            aria-label="Close tool canvas"
          >
            <X size={18} />
          </button>
        </div>
      </header>
      <div className="tool-canvas-body">
        <ToolRenderer
          tool={activeTool}
          state={toolState}
          onStateChange={onStateChange}
          onSubmitToAgent={onSubmitToAgent}
        />
      </div>
    </section>
  );
}
