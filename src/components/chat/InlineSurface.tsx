import { useCallback, useEffect, useRef, useState } from "react";
import {
  ChevronsDownUp,
  ChevronsUpDown,
  ExternalLink,
  Maximize2,
  PackagePlus,
} from "lucide-react";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { CustomizeMode } from "@/components/tool-canvas/CustomizeMode";
import { DraftConflictBanner } from "@/components/chat/DraftConflictBanner";
import { api, TauriCommandError } from "@/lib/tauri";
import {
  captureFocusSnapshot,
  captureMediaSnapshot,
  captureScrollSnapshot,
  restoreFocusSnapshot,
  restoreMediaSnapshot,
  restoreScrollSnapshot,
} from "@/lib/preservation";
import { mergeStateForDefinitionPatch } from "@/lib/surface-ops";
import { verifySurfaceElement } from "@/lib/visual-verification";
import type { SurfaceRecord } from "@/types/runtime-v2";
import type { ToolDefinition, ToolState } from "@/types/tool";
import { useAppStore } from "@/stores/app-store";

function asToolDefinition(surface: SurfaceRecord): ToolDefinition {
  const def = surface.definition as ToolDefinition;
  return {
    id: def.id || surface.id,
    name: def.name || surface.name || "Inline surface",
    description: def.description ?? "",
    layout: def.layout ?? { type: "single-column" },
    components: Array.isArray(def.components) ? def.components : [],
    version: surface.currentRevision,
  };
}

function useNewUpdatesIndicator(
  contentVersion: number,
  scrollRef: React.RefObject<HTMLElement | null>,
) {
  const [showNewUpdates, setShowNewUpdates] = useState(false);
  const stickToBottom = useRef(true);
  const prevVersion = useRef(contentVersion);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onScroll = () => {
      const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
      stickToBottom.current = distance < 48;
      if (stickToBottom.current) setShowNewUpdates(false);
    };
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, [scrollRef]);

  useEffect(() => {
    if (contentVersion !== prevVersion.current) {
      if (!stickToBottom.current) {
        setShowNewUpdates(true);
      } else {
        scrollRef.current?.scrollTo({
          top: scrollRef.current.scrollHeight,
          behavior: "smooth",
        });
      }
      prevVersion.current = contentVersion;
    }
  }, [contentVersion, scrollRef]);

  const jumpToLatest = () => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
    setShowNewUpdates(false);
    stickToBottom.current = true;
  };

  return { showNewUpdates, jumpToLatest };
}

export function InlineSurfaceCard({
  surface: initial,
  conversationId,
}: {
  surface: SurfaceRecord;
  conversationId: string;
}) {
  const [surface, setSurface] = useState(initial);
  const [collapsed, setCollapsed] = useState(false);
  const [fullWidth, setFullWidth] = useState(false);
  const [state, setState] = useState<ToolState>({});
  const [hydrated, setHydrated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [a11yWarn, setA11yWarn] = useState<string | null>(null);
  // Real kernel application id for crash strikes — never invent from toolId.
  const [kernelApplicationId, setKernelApplicationId] = useState<string | null>(
    null,
  );
  const rootRef = useRef<HTMLElement | null>(null);
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const previousToolRef = useRef<ToolDefinition | null>(null);
  // Hydration is keyed on `surface.id`, but its async body must record the
  // newest definition, not the one captured when the effect first ran.
  const latestSurfaceRef = useRef(surface);
  const sendMessage = useAppStore((s) => s.sendMessage);
  const activeProjectId = useAppStore((s) => s.activeProjectId);
  const setSurfaceDraftConflict = useAppStore((s) => s.setSurfaceDraftConflict);

  const contentVersion = surface.currentRevision + Object.keys(state).length;
  const { showNewUpdates, jumpToLatest } = useNewUpdatesIndicator(
    contentVersion,
    bodyRef,
  );

  useEffect(() => {
    const prevTool = previousToolRef.current;
    const nextTool = asToolDefinition(initial);
    setSurface(initial);
    if (!hydrated) return;
    if (prevTool && prevTool.id === nextTool.id) {
      setState((current) =>
        mergeStateForDefinitionPatch({
          previousComponents: prevTool.components ?? [],
          nextComponents: nextTool.components ?? [],
          previousState: current,
          hydratedState: current,
        }),
      );
    }
    previousToolRef.current = nextTool;
  }, [hydrated, initial]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [savedState, continuity] = await Promise.all([
          api.getSurfaceState(surface.id),
          api.getContinuity(surface.id).catch(() => null),
        ]);
        if (cancelled) return;
        setState(savedState ?? {});
        setHydrated(true);
        previousToolRef.current = asToolDefinition(latestSurfaceRef.current);
        if (continuity && bodyRef.current) {
          restoreScrollSnapshot(
            bodyRef.current,
            continuity.scroll as Record<string, { scrollTop: number }>,
          );
          restoreFocusSnapshot(
            bodyRef.current,
            continuity.focus as {
              componentId?: string | null;
              fieldId?: string | null;
            },
          );
          restoreMediaSnapshot(
            bodyRef.current,
            continuity.media as Record<string, { paused?: boolean }>,
          );
        }
      } catch {
        if (!cancelled) {
          setState({});
          setHydrated(true);
          previousToolRef.current = asToolDefinition(latestSurfaceRef.current);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [surface.id]);

  const tool = asToolDefinition(surface);
  const manifestLookupId = surface.toolId?.trim() || tool.id;

  useEffect(() => {
    let cancelled = false;
    void api
      .kernelGetManifest(manifestLookupId)
      .then((record) => {
        if (!cancelled) setKernelApplicationId(record.applicationId);
      })
      .catch(() => {
        if (!cancelled) setKernelApplicationId(null);
      });
    return () => {
      cancelled = true;
    };
  }, [manifestLookupId]);

  const suspend = useCallback(async () => {
    const root = bodyRef.current ?? rootRef.current;
    if (!root) {
      await api.suspendSurface(surface.id).catch(() => undefined);
      return;
    }
    const focus = captureFocusSnapshot(root);
    const scroll = captureScrollSnapshot(root);
    const media = captureMediaSnapshot(root);
    try {
      await api.saveContinuity({
        surfaceId: surface.id,
        focus,
        scroll,
        media,
        suspensionState: "suspended",
      });
    } catch {
      await api.suspendSurface(surface.id).catch(() => undefined);
    }
  }, [surface.id]);

  useEffect(() => {
    if (collapsed) {
      void suspend();
    }
  }, [collapsed, suspend]);

  useEffect(() => {
    return () => {
      void suspend();
    };
  }, [surface.id, suspend]);

  useEffect(() => {
    if (collapsed || !rootRef.current) {
      setA11yWarn(null);
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      if (!rootRef.current) return;
      const result = verifySurfaceElement(rootRef.current);
      const fails = result.checks.filter((c) => c.status === "fail");
      setA11yWarn(
        fails.length > 0
          ? fails.map((f) => f.detail || f.id).join("; ")
          : null,
      );
    });
    return () => window.cancelAnimationFrame(frame);
  }, [surface, state, collapsed, fullWidth]);

  const persistState = useCallback(
    async (next: ToolState) => {
      const previous = state;
      setState(next);
      try {
        await api.saveSurfaceState(surface.id, next);
      } catch {
        setState(previous);
        throw new Error("Failed to save surface state");
      }
    },
    [state, surface.id],
  );

  const saveComponentDraft = useCallback(
    async (componentId: string, draft: unknown, formId?: string | null) => {
      try {
        await api.saveDraft({
          surfaceId: surface.id,
          componentId,
          baseRevision: surface.currentRevision,
          draft,
          formId: formId ?? null,
        });
      } catch (error) {
        if (
          error instanceof TauriCommandError &&
          error.code === "draft_conflict"
        ) {
          setSurfaceDraftConflict({
            surfaceId: surface.id,
            componentId,
            windowId: "main",
            storedRevision: surface.currentRevision,
            requestedRevision: surface.currentRevision,
            userDraft: draft,
            agentDraft: null,
            formId: formId ?? null,
          });
        }
      }
    },
    [setSurfaceDraftConflict, surface.currentRevision, surface.id],
  );

  const promote = async () => {
    try {
      const next = await api.promoteSurface(surface.id);
      setSurface(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const openWindow = async () => {
    if (surface.toolId) {
      try {
        await api.openToolWindow(surface.toolId);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    }
  };

  return (
    <section
      ref={rootRef}
      className={`inline-surface${fullWidth ? " full-width" : ""}${collapsed ? " collapsed" : ""}`}
      data-surface-id={surface.id}
      data-instance-id={surface.instanceId}
      aria-label={surface.name || "Inline surface"}
    >
      <header className="inline-surface-header">
        <div className="inline-surface-title">
          <strong>{surface.name || tool.name}</strong>
          <span className="muted">r{surface.currentRevision}</span>
        </div>
        <div className="inline-surface-actions" role="toolbar" aria-label="Surface actions">
          <CustomizeMode
            surfaceId={surface.id}
            conversationId={conversationId}
            tool={tool}
            baseRevision={surface.currentRevision}
            onApplied={() => {
              void api.getSurface(surface.id).then(setSurface).catch(() => undefined);
            }}
          />
          <button
            type="button"
            className="btn btn-ghost"
            aria-expanded={!collapsed}
            onClick={() => setCollapsed((c) => !c)}
            aria-label={collapsed ? "Expand surface" : "Collapse surface"}
            title={collapsed ? "Expand" : "Collapse"}
          >
            {collapsed ? <ChevronsUpDown size={16} /> : <ChevronsDownUp size={16} />}
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => setFullWidth((f) => !f)}
            aria-label="Toggle full width"
            title="Full width"
          >
            <Maximize2 size={16} />
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => void promote()}
            aria-label="Promote to Personal Tool"
            title="Promote to Personal Tool"
          >
            <PackagePlus size={16} />
          </button>
          {surface.toolId ? (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => void openWindow()}
              aria-label="Open surface in a new window"
              title="Open in window"
            >
              <ExternalLink size={16} />
            </button>
          ) : null}
        </div>
      </header>
      <DraftConflictBanner />
      {error ? (
        <p className="tr-validation" role="alert">
          {error}
        </p>
      ) : null}
      {a11yWarn ? (
        <p className="muted" role="status" style={{ margin: "0.35rem 0.75rem" }}>
          Layout check: {a11yWarn}
        </p>
      ) : null}
      {!collapsed ? (
        <div
          className="inline-surface-body"
          ref={bodyRef}
          data-scroll-key="inline-surface-body"
        >
          {showNewUpdates ? (
            <button
              type="button"
              className="new-updates-chip"
              onClick={jumpToLatest}
            >
              New updates
            </button>
          ) : null}
          <ToolRenderer
            tool={tool}
            state={state}
            onStateChange={(next) => setState(next)}
            onPersistState={persistState}
            // Prefer a real kernel application id when present; never invent one
            // solely so render failures can advance crash_count for legacy tools.
            applicationId={kernelApplicationId}
            surfaceId={surface.id}
            conversationId={conversationId}
            projectId={activeProjectId}
            onPendingApproval={() => {
              window.dispatchEvent(new Event("coreside:pending-approval"));
            }}
            onSubmitToAgent={(payload) => {
              const summary = `Surface form submitted (${payload.eventName})`;
              // Typed StructuredUserInput is sealed in Rust via send_message.
              // Chat text is human-readable only — not trust authority.
              const fields =
                Object.keys(payload.values).length > 64
                  ? Object.fromEntries(Object.entries(payload.values).slice(0, 64))
                  : payload.values;
              void (async () => {
                try {
                  await api.appendContextLedger({
                    conversationId,
                    projectId: activeProjectId,
                    entryType: "surface_form_submit",
                    visibility: "model_context_only",
                    payload: {
                      surfaceId: surface.id,
                      formId: payload.componentId ?? surface.id,
                      instanceId: surface.instanceId,
                      componentId: payload.componentId ?? null,
                      eventName: payload.eventName,
                      values: payload.values,
                    },
                    summary,
                  });
                } catch {
                  // Ledger is best-effort; typed send_message path still seals trust.
                }
                void saveComponentDraft(
                  payload.componentId ?? "form",
                  payload.values,
                  payload.componentId ?? null,
                );
                void sendMessage(summary, [], [], {
                  formId: payload.componentId ?? surface.id,
                  surfaceId: surface.id,
                  fields,
                });
              })();
            }}
          />
        </div>
      ) : null}
    </section>
  );
}

export function InlineSurfacesForMessage({
  conversationId,
  messageId,
}: {
  conversationId: string;
  messageId: string;
}) {
  const [surfaces, setSurfaces] = useState<SurfaceRecord[]>([]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const all = await api.listConversationSurfaces(conversationId);
        if (!cancelled) {
          setSurfaces(all.filter((s) => s.messageId === messageId));
        }
      } catch {
        if (!cancelled) setSurfaces([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [conversationId, messageId]);

  if (!surfaces.length) return null;
  // Soft per-message UI cap (matches MAX_INLINE_SURFACES_VISIBLE). Backend list
  // returns the full conversation set so older messages are not starved.
  const visible = surfaces.slice(0, 12);
  return (
    <div className="inline-surfaces">
      {visible.map((s) => (
        <InlineSurfaceCard
          key={s.instanceId}
          surface={s}
          conversationId={conversationId}
        />
      ))}
    </div>
  );
}
