import { Download, ExternalLink, History, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { AppRouteShell } from "@/components/tool-renderer/AppRouteShell";
import { CustomizeMode } from "@/components/tool-canvas/CustomizeMode";
import { api } from "@/lib/tauri";
import {
  captureFocusSnapshot,
  captureMediaSnapshot,
  captureScrollSnapshot,
  restoreFocusSnapshot,
  restoreMediaSnapshot,
  restoreScrollSnapshot,
} from "@/lib/preservation";
import { surfaceIdForTool } from "@/lib/surface-ops";
import type { ApplicationManifest } from "@/types/application-kernel";
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
    openExportDialog,
    activeConversationId,
    activeProjectId,
  } = useAppStore();
  const [manifest, setManifest] = useState<ApplicationManifest | null>(null);

  const onStateChange = useCallback(
    (state: Record<string, unknown>) => {
      void updateToolState(state, true);
    },
    [updateToolState],
  );

  const onPersistState = useCallback(
    async (state: Record<string, unknown>) => {
      await updateToolState(state, true);
    },
    [updateToolState],
  );

  useEffect(() => {
    if (!activeTool) {
      setManifest(null);
      return;
    }
    const surfaceId = surfaceIdForTool(activeTool.id);
    void api
      .kernelGetManifest(surfaceId)
      .then((rec) => setManifest(rec.manifest ?? null))
      .catch(() =>
        api
          .kernelGetManifest(activeTool.id)
          .then((rec) => setManifest(rec.manifest ?? null))
          .catch(() => setManifest(null)),
      );

    void api
      .getContinuity(surfaceId, "tool_canvas")
      .then((c) => {
        const root = document.querySelector<HTMLElement>(
          `.tool-canvas-body[data-tool-id="${activeTool.id}"]`,
        );
        if (!root || !c) return;
        restoreScrollSnapshot(
          root,
          c.scroll as import("@/lib/preservation").ScrollSnapshot | null,
        );
        restoreFocusSnapshot(
          root,
          c.focus as import("@/lib/preservation").FocusSnapshot | null,
        );
        restoreMediaSnapshot(
          root,
          c.media as import("@/lib/preservation").MediaSnapshot | null,
        );
      })
      .catch(() => undefined);

    return () => {
      const root = document.querySelector<HTMLElement>(
        `.tool-canvas-body[data-tool-id="${activeTool.id}"]`,
      );
      if (!root) return;
      void api
        .saveContinuity({
          surfaceId,
          windowId: "tool_canvas",
          focus: captureFocusSnapshot(root),
          scroll: captureScrollSnapshot(root),
          media: captureMediaSnapshot(root),
          suspensionState: "suspended",
        })
        .catch(() => api.suspendSurface(surfaceId, "tool_canvas"));
    };
  }, [activeTool]);

  const onSubmitToAgent = useCallback(
    (payload: {
      toolId: string;
      eventName: string;
      componentId?: string;
      values: Record<string, unknown>;
    }) => {
      const summary = `Tool form submitted (${payload.eventName})`;
      if (activeConversationId) {
        void api.appendContextLedger({
          conversationId: activeConversationId,
          projectId: activeProjectId,
          entryType: "tool_form_submit",
          visibility: "model_context_only",
          payload: {
            toolId: payload.toolId,
            componentId: payload.componentId ?? null,
            eventName: payload.eventName,
            values: payload.values,
          },
          summary,
        });
      }
      void sendMessage(summary);
    },
    [activeConversationId, activeProjectId, sendMessage],
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
          <CustomizeMode
            surfaceId={surfaceIdForTool(activeTool.id)}
            conversationId={activeConversationId}
            tool={activeTool}
            baseRevision={activeTool.version ?? 1}
            onApplied={() => {
              void useAppStore.getState().selectTool(activeTool.id);
            }}
          />
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => openExportDialog(activeTool.id, activeTool.name)}
            aria-label="Export tool"
          >
            <Download size={16} aria-hidden />
            Export
          </button>
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
      <div className="tool-canvas-body" data-tool-id={activeTool.id}>
        {manifest?.routes && manifest.routes.length > 0 ? (
          <AppRouteShell
            applicationId={manifest.applicationId || activeTool.id}
            manifest={manifest}
            surfacesById={{
              [surfaceIdForTool(activeTool.id)]: activeTool,
              [activeTool.id]: activeTool,
            }}
            state={toolState}
            onStateChange={onStateChange}
            onSubmitToAgent={onSubmitToAgent}
          />
        ) : (
          <ToolRenderer
            tool={activeTool}
            state={toolState}
            onStateChange={onStateChange}
            onPersistState={onPersistState}
            onSubmitToAgent={onSubmitToAgent}
          />
        )}
      </div>
    </section>
  );
}
