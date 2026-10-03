import { X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApplicationDetailsPanel } from "@/components/applications/ApplicationDetailsPanel";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { AppRouteShell } from "@/components/tool-renderer/AppRouteShell";
import { ToolHeaderActions } from "@/components/tool-canvas/ToolHeaderActions";
import { InteractiveStatusBar } from "@/components/tool-renderer/InteractiveStatusBar";
import { hasInteractiveDefinition, useInteractiveSurface } from "@/lib/interactive-surface";
import { api } from "@/lib/tauri";
import { consumerErrorMessage } from "@/lib/consumer-errors";
import {
  captureFocusSnapshot,
  captureMediaSnapshot,
  captureScrollSnapshot,
  restoreFocusSnapshot,
  restoreMediaSnapshot,
  restoreScrollSnapshot,
  resolvePendingInteractionIdempotencyKey,
  startRendererMountLifecycle,
} from "@/lib/preservation";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { classifyToolHeaderDensity } from "@/lib/layout-mode";
import { persistenceScheduler } from "@/lib/persistence-scheduler";
import { surfaceIdForTool } from "@/lib/surface-ops";
import type { ToolDefinition } from "@/types/tool";
import type { SurfaceRecord } from "@/types/runtime-v2";
import { toToolDefinition, SoftwareDocumentSchema } from "@/lib/software-document";
import type {
  ActionOutcome,
  ManifestRecord,
  RecoveryState,
} from "@/types/application-kernel";
import { useAppStore } from "@/stores/app-store";
import { getPreviewOverlayForTool } from "@/lib/preview/surface-overlay";

function asToolDefinition(surface: SurfaceRecord): ToolDefinition {
  const def = surface.definition as Record<string, unknown>;
  if (def && Array.isArray(def.sections) && !("components" in def)) {
    const parsed = SoftwareDocumentSchema.safeParse(def);
    if (parsed.success) {
      return toToolDefinition(parsed.data);
    }
  }
  const toolDef = def as ToolDefinition;
  return {
    id: (toolDef.id as string) || surface.id,
    name: (toolDef.name as string) || surface.name || "Application",
    description: (toolDef.description as string) ?? "",
    layout: (toolDef.layout as ToolDefinition["layout"]) ?? { type: "single-column" },
    components: Array.isArray(toolDef.components) ? toolDef.components : [],
    version: surface.currentRevision,
  };
}

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
  const flushToolState = useAppStore((s) => s.flushToolState);
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
  const [canonicalSurface, setCanonicalSurface] = useState<SurfaceRecord | null>(null);
  const [canonicalState, setCanonicalState] = useState<Record<string, unknown> | null>(null);
  /** State revision — the monotonic counter for the surface's *state* (not definition).
   * This is the correct value to pass as expectedStateRevision to saveSurfaceState.
   * Do NOT use canonicalSurface.currentRevision — that is the *definition* revision. */
  const [stateRevision, setStateRevision] = useState<number>(0);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [canvasError, setCanvasError] = useState<string | null>(null);
  const [isCustomizing, setIsCustomizing] = useState(false);
  const [selectedComponentId, setSelectedComponentId] = useState<string | null>(null);
  /** Available width for the whole Tool Canvas header (not the actions row). */
  const [headerWidth, setHeaderWidth] = useState(0);
  const headerRef = useRef<HTMLElement>(null);

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

  const headerDensity = classifyToolHeaderDensity({
    headerWidth,
    layoutMode,
  });

  const applicationId = useMemo(() => {
    if (!activeTool) return null;
    return manifestRecord?.applicationId ?? activeTool.id;
  }, [activeTool, manifestRecord?.applicationId]);

  const surfaceId = useMemo(() => {
    if (!activeTool) return null;
    // Authoritative DB id once loaded; surf-* is only a first-load compatibility hint.
    // Never reuse a prior tool's surface across switches (hydration / OCC leak).
    if (
      canonicalSurface?.id &&
      canonicalSurface.toolId === activeTool.id
    ) {
      return canonicalSurface.id;
    }
    return surfaceIdForTool(activeTool.id);
  }, [activeTool, canonicalSurface?.id, canonicalSurface?.toolId]);

  const availability = useMemo(
    () => isApplicationUnavailable(manifestRecord, recovery),
    [manifestRecord, recovery],
  );

  const activeToolId = activeTool?.id;
  const activeToolVersion = activeTool?.version ?? null;
  const prevCanvasToolIdRef = useRef<string | null>(null);

  useEffect(() => {
    if (!activeToolId) {
      prevCanvasToolIdRef.current = null;
      setCanonicalSurface(null);
      setCanonicalState(null);
      setStateRevision(0);
      return;
    }
    // Drop prior tool binding on identity change only (keep canvas during version Sync).
    if (prevCanvasToolIdRef.current !== activeToolId) {
      prevCanvasToolIdRef.current = activeToolId;
      setCanonicalSurface(null);
      setCanonicalState(null);
      setStateRevision(0);
    }
    let cancelled = false;
    // Prefer SQLite lineage resolution over assuming surf-${toolId}.
    void (async () => {
      const bound = await api
        .resolveBoundSurfaceForTool(activeToolId)
        .catch(() => null);
      const sid = bound?.id ?? surfaceIdForTool(activeToolId);
      const [surf, stWithRev] = await Promise.all([
        bound ?? api.getSurface(sid).catch(() => null),
        api.getSurfaceStateWithRevision(sid).catch(() => null),
      ]);
      if (cancelled) return;
      if (surf && (surf.toolId == null || surf.toolId === activeToolId)) {
        setCanonicalSurface(surf);
        if (hasInteractiveDefinition(surf.definition)) {
          setCanonicalState({});
        } else {
          setCanonicalState(stWithRev?.state ?? {});
        }
        setStateRevision(stWithRev?.stateRevision ?? 0);
      } else {
        setCanonicalSurface(null);
        setCanonicalState(null);
        setStateRevision(0);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [activeToolId, activeToolVersion]);

  const interactiveRevisionRef = useRef(stateRevision);
  interactiveRevisionRef.current = stateRevision;
  const interactionLockRef = useRef(false);
  /** Stable idempotency for in-flight interaction retries until success. */
  const pendingIdempotencyRef = useRef<{
    logicalKey: string;
    idempotencyKey: string;
  } | null>(null);
  const isInteractive = hasInteractiveDefinition(canonicalSurface?.definition);
  const adoptInteractiveState = useCallback(
    (update: (current: Record<string, unknown>) => Record<string, unknown>) =>
      setCanonicalState((current) => update(current ?? {})),
    [],
  );
  const interactive = useInteractiveSurface({
    surfaceId: surfaceId ?? "",
    stateRevisionRef: interactiveRevisionRef,
    setState: adoptInteractiveState,
    onAdopt: (view) => setStateRevision(view.stateRevision),
  });
  const hydrateInteractive = interactive.hydrate;
  const canonicalSurfaceId = canonicalSurface?.id;
  const canonicalDefinitionRevision = canonicalSurface?.currentRevision;
  useEffect(() => {
    if (!isInteractive || !canonicalSurfaceId) return;
    hydrateInteractive().catch(() => undefined);
  }, [isInteractive, canonicalSurfaceId, canonicalDefinitionRevision, hydrateInteractive]);

  // After the canvas mounts (or remounts after Sync), register renderer readiness
  // and promote deferred patches. SQLite existence alone is not mount readiness.
  useEffect(() => {
    if (!canonicalSurfaceId) return;
    return startRendererMountLifecycle({
      surfaceId: canonicalSurfaceId,
      conversationId: activeConversationId,
      definitionRevision: canonicalDefinitionRevision,
      stateRevision: stateRevision ?? undefined,
      registerMount: (args) => api.registerSurfaceMount(args),
      unregisterMount: (args) => api.unregisterSurfaceMount(args),
      flushPatchScheduler: (args) => api.flushPatchScheduler(args),
    });
  }, [canonicalSurfaceId, canonicalDefinitionRevision, activeConversationId, stateRevision]);

  const onStateChange = useCallback(
    (state: Record<string, unknown>) => {
      if (canonicalSurface && activeTool) {
        setCanonicalState(state);
      }
      void updateToolState(state, true);
    },
    [canonicalSurface, activeTool, updateToolState],
  );

  const onPersistState = useCallback(
    async (state: Record<string, unknown>) => {
      if (canonicalSurface && activeTool) {
        setCanonicalState(state);
        // Persist against the authoritative SQLite surface id, not surf-* guess.
        const sid = canonicalSurface.id;
        try {
          // Pass stateRevision (not definition revision) as the OCC guard.
          const newStateRev = await api.saveSurfaceState(sid, state, stateRevision);
          setStateRevision(newStateRev);
          await api.saveToolState(activeTool.id, state).catch(() => undefined);
          useAppStore.setState({ toolState: state });
        } catch (err) {
          // On conflict, reload fresh state + revision atomically.
          const [freshSurf, freshWithRev] = await Promise.all([
            api.getSurface(sid).catch(() => null),
            api.getSurfaceStateWithRevision(sid).catch(() => null),
          ]);
          if (freshSurf) setCanonicalSurface(freshSurf);
          if (freshWithRev) {
            setCanonicalState(freshWithRev.state);
            setStateRevision(freshWithRev.stateRevision);
            useAppStore.setState({ toolState: freshWithRev.state });
          }
          throw err;
        }
      } else {
        await updateToolState(state, true);
        await flushToolState(activeTool?.id);
      }
    },
    [canonicalSurface, activeTool, stateRevision, updateToolState, flushToolState],
  );

  useEffect(() => {
    const el = headerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    let frame = 0;
    const publishWidth = () => {
      // clientWidth matches padding-box; avoid mixing contentRect vs border-box.
      setHeaderWidth(el.clientWidth);
    };
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(publishWidth);
    });
    ro.observe(el);
    publishWidth();
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, [activeToolId]);

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
          .catch(() =>
            api
              .kernelEnsureToolManifest(activeTool.id, activeTool.name, sid)
              .then((rec) => setManifestRecord(rec))
              .catch(() => setManifestRecord(null)),
          ),
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
    async (payload: {
      toolId: string;
      eventName: string;
      componentId?: string;
      values: Record<string, unknown>;
      silent?: boolean;
    }) => {
      if (interactionLockRef.current) return;
      const toolId = activeTool?.id;
      if (!toolId) return;
      interactionLockRef.current = true;
      try {
        const summary = `Tool form submitted (${payload.eventName})`;
        const isSilent = Boolean(
          payload.silent ||
            payload.eventName.startsWith("game.") ||
            payload.eventName.endsWith(".silent") ||
            payload.values.silent === true,
        );
        const sid = surfaceId ?? surfaceIdForTool(toolId);
        // Flush scheduled ToolCanvas state with OCC before sealing the envelope.
        await persistenceScheduler.flush(toolId, async (tid, s) => {
          await api.saveToolState(tid, s);
          const newRev = await api.saveSurfaceState(
            sid,
            s,
            interactiveRevisionRef.current,
          );
          if (typeof newRev === "number") {
            interactiveRevisionRef.current = newRev;
            setStateRevision(newRev);
          }
        });
        const logicalKey = [
          toolId,
          payload.eventName,
          payload.componentId ?? "",
          String(interactiveRevisionRef.current),
        ].join(":");
        const { idempotencyKey, pending } =
          resolvePendingInteractionIdempotencyKey(
            pendingIdempotencyRef.current,
            logicalKey,
            (key) => `idem-${key}-${crypto.randomUUID()}`,
          );
        pendingIdempotencyRef.current = pending;
        // Single authority: send_message seals StructuredUserInput in Rust.
        await sendMessage(summary, [], [], {
          formId: payload.componentId ?? toolId,
          applicationId: manifestRecord?.applicationId ?? null,
          surfaceId: sid,
          surfaceRevision:
            canonicalSurface?.currentRevision ?? activeTool.version ?? null,
          stateRevision: interactiveRevisionRef.current,
          componentId: payload.componentId ?? null,
          idempotencyKey,
          eventName: payload.eventName,
          fields: {
            ...payload.values,
            ...(isSilent ? { silent: true } : {}),
          },
        });
        // Success — next click is a new logical interaction.
        pendingIdempotencyRef.current = null;
      } finally {
        interactionLockRef.current = false;
      }
    },
    [
      activeTool,
      canonicalSurface?.currentRevision,
      manifestRecord?.applicationId,
      sendMessage,
      surfaceId,
    ],
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
  const canonicalDef = canonicalSurface ? asToolDefinition(canonicalSurface) : null;
  const renderTool = previewOverlay?.tool ?? canonicalDef ?? activeTool;
  const isPreviewPaint = Boolean(previewOverlay);
  const liveState =
    previewOverlay?.state ??
    (canonicalSurface && canonicalState && Object.keys(canonicalState).length > 0
      ? // Store/toolState last so Sync dirty reconciliation and live typing win.
        { ...canonicalState, ...toolState }
      : toolState);
  const inspectingReplay =
    !isPreviewPaint && isInteractive && interactive.replayState != null;
  const renderState = inspectingReplay
    ? { ...liveState, ...interactive.replayState }
    : liveState;

  const surfacesById = useMemo(() => {
    const map: Record<string, ToolDefinition> = {};
    if (activeTool) {
      const current = renderTool ?? activeTool;
      map[activeTool.id] = current;
      map[surfaceIdForTool(activeTool.id)] = current;
    }
    return map;
  }, [activeTool, renderTool]);

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
      <header className="tool-canvas-header" ref={headerRef}>
        <div className="tool-canvas-header-title">
          <h2>
            {activeTool.name}
            {isPreviewPaint ? (
              <span
                className="tool-preview-badge"
                data-testid="tool-preview-badge"
                aria-label="Preview — not saved yet"
              >
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
        <div className="tool-canvas-header-actions">
          <ToolHeaderActions
            tool={activeTool}
            // surfaceId is non-null once activeTool exists (canonical or first-load hint).
            surfaceId={surfaceId as string}
            conversationId={activeConversationId}
            density={headerDensity}
            closeLabel={
              layoutMode === "compact" ? "Back to chat" : "Close tool canvas"
            }
            isCustomizing={isCustomizing}
            onToggleCustomizing={() => setIsCustomizing((v) => !v)}
            selectedComponentId={selectedComponentId}
            onSelectComponentId={setSelectedComponentId}
            onAskAi={(prompt) => {
              void sendMessage(prompt);
            }}
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
            surfacesById={surfacesById}
            state={renderState}
            onStateChange={isPreviewPaint ? () => undefined : onStateChange}
            onPersistState={isPreviewPaint ? async () => undefined : onPersistState}
            onSubmitToAgent={isPreviewPaint ? undefined : onSubmitToAgent}
            surfaceId={surfaceId}
            conversationId={activeConversationId}
            projectId={activeProjectId}
            isCustomizing={isCustomizing}
            mode={isPreviewPaint ? "preview" : isCustomizing ? "customize" : "live"}
            selectedComponentId={selectedComponentId}
            onSelectComponent={(c) => setSelectedComponentId(c.id)}
            onPendingApproval={onPendingApproval}
          />
        ) : (
          <>
          {isInteractive && !isPreviewPaint ? (
            <InteractiveStatusBar
              view={interactive.view}
              error={interactive.error}
              pending={interactive.pending}
              history={interactive.history}
              replaySeq={interactive.replaySeq}
              onUndo={() => void interactive.undo()}
              onLoadHistory={() => void interactive.loadHistory()}
              onReplayAt={(seq) => void interactive.replayAt(seq)}
              onClearReplay={interactive.clearReplay}
            />
          ) : null}
          <ToolRenderer
            tool={renderTool ?? activeTool}
            state={renderState}
            onStateChange={
              isPreviewPaint || inspectingReplay ? () => undefined : onStateChange
            }
            onPersistState={
              isPreviewPaint || inspectingReplay ? async () => undefined : onPersistState
            }
            isCustomizing={isCustomizing}
            mode={isPreviewPaint ? "preview" : isCustomizing ? "customize" : "live"}
            selectedComponentId={selectedComponentId}
            onSelectComponent={(c) => setSelectedComponentId(c.id)}
            // Only a real manifest id — never invent one for legacy tools.
            applicationId={manifestRecord?.applicationId ?? null}
            surfaceId={surfaceId}
            conversationId={activeConversationId}
            projectId={activeProjectId}
            onSubmitToAgent={isPreviewPaint ? undefined : onSubmitToAgent}
            onPendingApproval={onPendingApproval}
            onInteractiveDispatch={
              isInteractive && !isPreviewPaint && !inspectingReplay
                ? interactive.dispatch
                : undefined
            }
          />
          </>
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
