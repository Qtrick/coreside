import { X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApplicationDetailsPanel } from "@/components/applications/ApplicationDetailsPanel";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { AppRouteShell } from "@/components/tool-renderer/AppRouteShell";
import { ToolHeaderActions } from "@/components/tool-canvas/ToolHeaderActions";
import { api } from "@/lib/tauri";
import { consumerErrorMessage } from "@/lib/consumer-errors";
import {
  captureFocusSnapshot,
  captureMediaSnapshot,
  captureScrollSnapshot,
  restoreFocusSnapshot,
  restoreMediaSnapshot,
  restoreScrollSnapshot,
} from "@/lib/preservation";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { surfaceIdForTool } from "@/lib/surface-ops";
import type {
  ActionOutcome,
  ManifestRecord,
  RecoveryState,
} from "@/types/application-kernel";
import { useAppStore } from "@/stores/app-store";
import { getPreviewOverlayForTool } from "@/lib/preview/surface-overlay";

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
  const previewSurfacesByKey = useAppStore((s) => s.previewSurfacesByKey);
  const updateToolState = useAppStore((s) => s.updateToolState);
  const closeToolCanvas = useAppStore((s) => s.closeToolCanvas);
  const openToolWindow = useAppStore((s) => s.openToolWindow);
  const undoTool = useAppStore((s) => s.undoTool);
  const sendMessage = useAppStore((s) => s.sendMessage);
  const openExportDialog = useAppStore((s) => s.openExportDialog);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const activeProjectId = useAppStore((s) => s.activeProjectId);
  const layoutMode = useAppStore((s) => s.layoutMode);
  const adaptiveWindowSizing = useAppStore((s) => s.adaptiveWindowSizing);
  const windowExpandStatus = useAppStore((s) => s.windowExpandStatus);
  const clearWindowExpandStatus = useAppStore((s) => s.clearWindowExpandStatus);
  const setAdaptiveWindowSizing = useAppStore((s) => s.setAdaptiveWindowSizing);
  const developerMode = useAppStore((s) => s.developerMode);
  const createConversation = useAppStore((s) => s.createConversation);
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);
  const [manifestRecord, setManifestRecord] = useState<ManifestRecord | null>(null);
  const [recovery, setRecovery] = useState<RecoveryState | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [canvasError, setCanvasError] = useState<string | null>(null);
  /** Available width for header actions (not the whole header). */
  const [actionsWidth, setActionsWidth] = useState(0);
  const actionsRef = useRef<HTMLDivElement>(null);

  const runHeaderAction = useCallback(
    async (label: string, run: () => Promise<unknown>) => {
      setCanvasError(null);
      try {
        await run();
      } catch (error) {
        const { message, technical } = consumerErrorMessage(
          error,
          `${label} failed.`,
        );
        setCanvasError(
          developerMode && technical && technical !== message
            ? `${message} (${technical})`
            : message,
        );
      }
    },
    [developerMode],
  );

  const headerDensity =
    actionsWidth > 0 && actionsWidth < 220
      ? "menu"
      : actionsWidth > 0 && actionsWidth < 360
        ? "icons"
        : layoutMode === "compact"
          ? "icons"
          : "full";

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
    const el = actionsRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    let frame = 0;
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width ?? 0;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => setActionsWidth(w));
    });
    ro.observe(el);
    setActionsWidth(el.getBoundingClientRect().width);
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, [activeTool?.id]);

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
      // Single authority: send_message seals StructuredUserInput in Rust.
      // Do not also appendContextLedger here — that created a second ledger-* id
      // which was skipped on this turn then reinjected on the next ordinary turn.
      void sendMessage(summary, [], [], {
        formId: payload.toolId,
        applicationId: payload.toolId,
        fields: payload.values,
      });
    },
    [sendMessage],
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

  const previewOverlay = useMemo(
    () =>
      getPreviewOverlayForTool(
        previewSurfacesByKey,
        activeTool?.id,
        activeConversationId,
      ),
    [previewSurfacesByKey, activeTool?.id, activeConversationId],
  );
  const renderTool = previewOverlay?.tool ?? activeTool;
  const renderState = previewOverlay?.state ?? toolState;
  const isPreviewPaint = Boolean(previewOverlay);

  if (!activeTool) {
    return (
      <section className="tool-canvas" aria-label="App canvas" data-coreside-tour="app-panel">
        <div className="empty-state">
          <h3>{EMPTY_STATES.noAppOpen.title}</h3>
          <p>{EMPTY_STATES.noAppOpen.body}</p>
          <div className="button-row empty-state-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => {
                void createConversation().then(() => {
                  document.getElementById("composer-input")?.focus();
                });
              }}
            >
              {EMPTY_STATES.noAppOpen.primaryCta}
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => openHelpAndLearning(navigateToSettings)}
            >
              {EMPTY_STATES.helpLink.label}
            </button>
          </div>
        </div>
      </section>
    );
  }

  const manifest = manifestRecord?.manifest ?? null;

  return (
    <section className="tool-canvas" aria-label={`${activeTool.name} canvas`} data-coreside-tour="app-panel">
      <header className="tool-canvas-header">
        <div className="tool-canvas-header-title" style={{ minWidth: 0, flex: "1 1 auto" }}>
          <h2>
            {activeTool.name}
            {isPreviewPaint ? (
              <span className="tool-preview-badge" aria-label="Preview — not saved yet">
                Preview
              </span>
            ) : null}
          </h2>
          <div className="tool-meta">
            <span title={activeTool.description || "Personal tool"}>
              {activeTool.description || "Personal tool"}
            </span>
            <span>v{activeTool.version ?? 1}</span>
            {isPreviewPaint ? (
              <span className="tool-preview-note">Speculative — not saved yet</span>
            ) : null}
          </div>
        </div>
        <div
          ref={actionsRef}
          className="tool-canvas-header-actions"
          style={{ flex: "0 1 auto", minWidth: 0, maxWidth: "100%" }}
        >
          <ToolHeaderActions
            tool={activeTool}
            conversationId={activeConversationId}
            density={headerDensity}
            closeLabel={
              layoutMode === "compact" ? "Back to chat" : "Close tool canvas"
            }
            onDetails={() => setDetailsOpen(true)}
            onExport={() => openExportDialog(activeTool.id, activeTool.name)}
            onOpen={() =>
              void runHeaderAction("Opening the tool window", openToolWindow)
            }
            onUndo={() => void runHeaderAction("Undo", undoTool)}
            onClose={closeToolCanvas}
            onCustomizeApplied={() => {
              void useAppStore.getState().selectTool(activeTool.id);
            }}
          />
        </div>
      </header>
      {windowExpandStatus?.startsWith("ask:") ? (
        <div className="window-fit-prompt" role="status">
          <span>{activeTool.name} works best with more room.</span>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => {
              const reducedMotion = window.matchMedia(
                "(prefers-reduced-motion: reduce)",
              ).matches;
              void api
                .windowOrchestratorExpand({
                  toolId: activeTool.id,
                  minUsefulWidth: 520,
                  minUsefulHeight: 420,
                  direction: "right",
                  reducedMotion,
                })
                .then(() =>
                  useAppStore.setState({
                    windowExpandStatus: "Expanded the window to fit the tool.",
                  }),
                )
                .catch(() => clearWindowExpandStatus());
            }}
          >
            Expand
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void openToolWindow()}
          >
            Open in new window
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={clearWindowExpandStatus}
          >
            Keep current size
          </button>
          {adaptiveWindowSizing === "ask" ? (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => {
                void setAdaptiveWindowSizing("smart");
                clearWindowExpandStatus();
                void useAppStore.getState().maybeExpandForTool(activeTool.id);
              }}
            >
              Always expand automatically
            </button>
          ) : null}
        </div>
      ) : windowExpandStatus ? (
        <div className="window-fit-status" role="status">
          {windowExpandStatus}
          <button
            type="button"
            className="text-btn"
            onClick={() => {
              const reducedMotion = window.matchMedia(
                "(prefers-reduced-motion: reduce)",
              ).matches;
              void api
                .windowOrchestratorRestore(reducedMotion)
                .finally(clearWindowExpandStatus);
            }}
          >
            Restore previous size
          </button>
        </div>
      ) : null}
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
              [surfaceIdForTool(activeTool.id)]: renderTool ?? activeTool,
              [activeTool.id]: renderTool ?? activeTool,
            }}
            state={renderState}
            onStateChange={isPreviewPaint ? () => undefined : onStateChange}
            onSubmitToAgent={onSubmitToAgent}
            surfaceId={surfaceId}
            conversationId={activeConversationId}
            projectId={activeProjectId}
            onPendingApproval={onPendingApproval}
          />
        ) : (
          <ToolRenderer
            tool={renderTool ?? activeTool}
            state={renderState}
            onStateChange={isPreviewPaint ? () => undefined : onStateChange}
            onPersistState={isPreviewPaint ? async () => undefined : onPersistState}
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
