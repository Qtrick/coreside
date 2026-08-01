import { Download, ExternalLink, History, Info, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { ApplicationDetailsPanel } from "@/components/applications/ApplicationDetailsPanel";
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
import type {
  ActionOutcome,
  ManifestRecord,
  RecoveryState,
} from "@/types/application-kernel";
import { useAppStore } from "@/stores/app-store";

function isApplicationUnavailable(
  record: ManifestRecord | null,
  recovery: RecoveryState | null,
): { blocked: boolean; message: string } {
  if (recovery?.disableUserSurfaces) {
    return {
      blocked: true,
      message:
        "User-created surfaces are temporarily disabled while Coreside is in recovery mode.",
    };
  }
  if (!record) {
    return { blocked: false, message: "" };
  }
  if (record.disabled) {
    return {
      blocked: true,
      message: "This application is turned off.",
    };
  }
  if (
    record.lifecycleState === "suspended" ||
    record.lifecycleState === "failed" ||
    record.lifecycleState === "disabled"
  ) {
    return {
      blocked: true,
      message: `This application is ${record.lifecycleState}.`,
    };
  }
  if (record.healthState === "failed" || record.healthState === "suspended") {
    return {
      blocked: true,
      message: `This application is not healthy (${record.healthState}).`,
    };
  }
  return { blocked: false, message: "" };
}

export function ToolCanvas() {
  // Selectors, not the whole store: this canvas stays mounted while chat
  // streams, and a full-store subscription re-renders the tool on every token.
  const activeTool = useAppStore((s) => s.activeTool);
  const toolState = useAppStore((s) => s.toolState);
  const updateToolState = useAppStore((s) => s.updateToolState);
  const closeToolCanvas = useAppStore((s) => s.closeToolCanvas);
  const openToolWindow = useAppStore((s) => s.openToolWindow);
  const undoTool = useAppStore((s) => s.undoTool);
  const sendMessage = useAppStore((s) => s.sendMessage);
  const openExportDialog = useAppStore((s) => s.openExportDialog);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const activeProjectId = useAppStore((s) => s.activeProjectId);
  const [manifestRecord, setManifestRecord] = useState<ManifestRecord | null>(null);
  const [recovery, setRecovery] = useState<RecoveryState | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [canvasError, setCanvasError] = useState<string | null>(null);

  const runHeaderAction = useCallback(
    async (label: string, run: () => Promise<unknown>) => {
      setCanvasError(null);
      try {
        await run();
      } catch (error) {
        setCanvasError(
          error instanceof Error ? error.message : `${label} failed.`,
        );
      }
    },
    [],
  );

  const applicationId = useMemo(() => {
    if (!activeTool) return null;
    return manifestRecord?.applicationId ?? activeTool.id;
  }, [activeTool, manifestRecord?.applicationId]);

  const surfaceId = useMemo(
    () => (activeTool ? surfaceIdForTool(activeTool.id) : null),
    [activeTool],
  );

  const availability = useMemo(
    () => isApplicationUnavailable(manifestRecord, recovery),
    [manifestRecord, recovery],
  );

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
      setManifestRecord(null);
      setRecovery(null);
      return;
    }
    const sid = surfaceIdForTool(activeTool.id);
    void api
      .kernelGetManifest(sid)
      .then((rec) => setManifestRecord(rec))
      .catch(() =>
        api
          .kernelGetManifest(activeTool.id)
          .then((rec) => setManifestRecord(rec))
          .catch(() => setManifestRecord(null)),
      );
    void api.kernelGetRecoveryState().then(setRecovery).catch(() => setRecovery(null));

    void api
      .getContinuity(sid, "tool_canvas")
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
          surfaceId: sid,
          windowId: "tool_canvas",
          focus: captureFocusSnapshot(root),
          scroll: captureScrollSnapshot(root),
          media: captureMediaSnapshot(root),
          suspensionState: "suspended",
        })
        .catch(() => api.suspendSurface(sid, "tool_canvas"));
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

  const onPendingApproval = useCallback(
    (_outcome: Extract<ActionOutcome, { status: "pendingApproval" }>) => {
      // Single host in AppShell owns approval cards; wake it immediately.
      window.dispatchEvent(new Event("coreside:pending-approval"));
    },
    [],
  );

  const restoreApplication = async () => {
    if (!applicationId) return;
    await api.kernelRestoreLastKnownGood(applicationId);
    const rec = await api.kernelGetManifest(applicationId).catch(() => null);
    setManifestRecord(rec);
  };

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

  const manifest = manifestRecord?.manifest ?? null;

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
            onClick={() => setDetailsOpen(true)}
            aria-label="Application details"
          >
            <Info size={16} aria-hidden />
            Details
          </button>
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
            onClick={() =>
              void runHeaderAction("Opening the tool window", openToolWindow)
            }
            aria-label="Open tool in new window"
          >
            <ExternalLink size={16} aria-hidden />
            Open
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void runHeaderAction("Undo", undoTool)}
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
      {canvasError ? (
        <div className="tr-action-error" role="alert">
          <span>{canvasError}</span>
          <button
            type="button"
            className="icon-btn"
            onClick={() => setCanvasError(null)}
            aria-label="Dismiss error"
          >
            <X size={14} aria-hidden />
          </button>
        </div>
      ) : null}
      <div className="tool-canvas-body" data-tool-id={activeTool.id}>
        {availability.blocked ? (
          <div className="empty-state tool-canvas-blocked">
            <h3>Application unavailable</h3>
            <p>{availability.message}</p>
            <div className="button-row">
              <button
                type="button"
                className="btn btn-primary"
                onClick={() =>
                  void runHeaderAction("Restore", restoreApplication)
                }
              >
                Restore last known good
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setDetailsOpen(true)}
              >
                Details
              </button>
            </div>
          </div>
        ) : manifest?.routes && manifest.routes.length > 0 ? (
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
            surfaceId={surfaceId}
            conversationId={activeConversationId}
            projectId={activeProjectId}
            onPendingApproval={onPendingApproval}
          />
        ) : (
          <ToolRenderer
            tool={activeTool}
            state={toolState}
            onStateChange={onStateChange}
            onPersistState={onPersistState}
            // Only a real manifest id — never invent one for legacy tools.
            applicationId={manifestRecord?.applicationId ?? null}
            surfaceId={surfaceId}
            conversationId={activeConversationId}
            projectId={activeProjectId}
            onSubmitToAgent={onSubmitToAgent}
            onPendingApproval={onPendingApproval}
          />
        )}
      </div>

      {applicationId ? (
        <ApplicationDetailsPanel
          applicationId={applicationId}
          open={detailsOpen}
          fallbackName={activeTool.name}
          onClose={() => setDetailsOpen(false)}
        />
      ) : null}
    </section>
  );
}
