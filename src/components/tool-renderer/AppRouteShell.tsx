import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { api } from "@/lib/tauri";
import { surfaceIdForTool } from "@/lib/surface-ops";
import { hasInteractiveDefinition } from "@/lib/interactive-surface";
import {
  canNavigateBack,
  canNavigateForward,
  isRouteNavigationNoOp,
} from "@/lib/route-navigation";
import type { ActionOutcome, ApplicationManifest } from "@/types/application-kernel";
import type { RouteState } from "@/types/runtime-v2";
import type { ToolComponent, ToolDefinition, ToolState } from "@/types/tool";
import { ToolRenderer } from "./ToolRenderer";

type AppRouteShellProps = {
  applicationId: string;
  manifest?: ApplicationManifest | null;
  surfacesById?: Record<string, ToolDefinition>;
  state: ToolState;
  onStateChange: (state: ToolState) => void;
  onPersistState?: (state: ToolState) => Promise<void>;
  surfaceId?: string | null;
  conversationId?: string | null;
  projectId?: string | null;
  isCustomizing?: boolean;
  mode?: "live" | "customize" | "preview";
  selectedComponentId?: string | null;
  onSelectComponent?: (component: ToolComponent) => void;
  onSubmitToAgent?: (payload: {
    toolId: string;
    eventName: string;
    componentId?: string;
    values: Record<string, unknown>;
    silent?: boolean;
  }) => void;
  onPendingApproval?: (outcome: Extract<ActionOutcome, { status: "pendingApproval" }>) => void;
};

export function AppRouteShell({
  applicationId,
  manifest,
  surfacesById = {},
  state,
  onStateChange,
  onPersistState,
  surfaceId,
  conversationId,
  projectId,
  isCustomizing,
  mode,
  selectedComponentId,
  onSelectComponent,
  onSubmitToAgent,
  onPendingApproval,
}: AppRouteShellProps) {
  const isPreview = mode === "preview";
  const routes = useMemo(() => manifest?.routes ?? [], [manifest?.routes]);
  const [routeState, setRouteState] = useState<RouteState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [perSurfaceState, setPerSurfaceState] = useState<Record<string, ToolState>>({});
  const [perSurfaceRevision, setPerSurfaceRevision] = useState<Record<string, number>>({});

  const loadRouteState = useCallback(async () => {
    if (isPreview) {
      const first = routes[0];
      if (first) {
        setRouteState({
          id: "preview-route-state",
          applicationId,
          windowId: "preview",
          currentRouteId: first.routeId,
          routeParams: {},
          history: [{ routeId: first.routeId, params: {} }],
          historyIndex: 0,
          updatedAt: new Date().toISOString(),
        });
      }
      return;
    }
    try {
      const next = await api.getRouteState(applicationId);
      setRouteState(next);
      setError(null);
    } catch {
      const first = routes[0];
      if (!first) {
        setRouteState(null);
        return;
      }
      // SECURITY: Server owns history. Use navigateRoute for initial seeding.
      const result = await api.navigateRoute({
        applicationId,
        routeId: first.routeId,
        routeParams: {},
        pushHistory: false,
      });
      setRouteState(result.state);
    }
  }, [applicationId, isPreview, routes]);

  useEffect(() => {
    if (!routes.length) return;
    void loadRouteState();
  }, [loadRouteState, routes.length]);

  const navigate = async (routeId: string, routeParams: Record<string, unknown> = {}) => {
    if (
      routeState &&
      isRouteNavigationNoOp(
        routeState.currentRouteId,
        routeState.routeParams,
        routeId,
        routeParams,
      )
    ) {
      return;
    }
    if (isPreview) {
      setRouteState((prev) => {
        if (!prev) return null;
        const newHistory = [
          ...prev.history.slice(0, prev.historyIndex + 1),
          { routeId, params: routeParams },
        ];
        return {
          ...prev,
          currentRouteId: routeId,
          routeParams,
          history: newHistory,
          historyIndex: newHistory.length - 1,
          updatedAt: new Date().toISOString(),
        };
      });
      return;
    }
    const result = await api.navigateRoute({
      applicationId,
      routeId,
      routeParams,
      pushHistory: true,
    });
    if (!result.changed) return;
    setRouteState(result.state);
  };

  const goBack = async () => {
    if (!routeState || !canNavigateBack(routeState.historyIndex)) return;
    if (isPreview) {
      const nextIndex = routeState.historyIndex - 1;
      const entry = routeState.history[nextIndex] as {
        routeId?: string;
        params?: Record<string, unknown>;
      };
      if (!entry?.routeId) return;
      setRouteState((prev) => {
        if (!prev) return null;
        return {
          ...prev,
          currentRouteId: entry.routeId!,
          routeParams: entry.params ?? {},
          historyIndex: nextIndex,
          updatedAt: new Date().toISOString(),
        };
      });
      return;
    }
    // SECURITY: Server owns history. Use routeBack command.
    const result = await api.routeBack(applicationId);
    if (!result.changed) return;
    setRouteState(result.state);
  };

  const goForward = async () => {
    if (
      !routeState ||
      !canNavigateForward(routeState.historyIndex, routeState.history.length)
    ) {
      return;
    }
    if (isPreview) {
      const nextIndex = routeState.historyIndex + 1;
      const entry = routeState.history[nextIndex] as {
        routeId?: string;
        params?: Record<string, unknown>;
      };
      if (!entry?.routeId) return;
      setRouteState((prev) => {
        if (!prev) return null;
        return {
          ...prev,
          currentRouteId: entry.routeId!,
          routeParams: entry.params ?? {},
          historyIndex: nextIndex,
          updatedAt: new Date().toISOString(),
        };
      });
      return;
    }
    // SECURITY: Server owns history. Use routeForward command.
    const result = await api.routeForward(applicationId);
    if (!result.changed) return;
    setRouteState(result.state);
  };

  // All hooks MUST come before any early return (React rules of hooks).
  // The early return is moved to the JSX block below.

  // Compute active route/surface derivations — guarded when routes is empty
  const activeRoute = routes.length
    ? (routes.find((route) => route.routeId === routeState?.currentRouteId) ?? routes[0])
    : undefined;
  const activeTool = activeRoute
    ? (activeRoute.surfaceId && surfacesById[activeRoute.surfaceId]) ||
      (activeRoute.surfaceId && surfacesById[surfaceIdForTool(activeRoute.surfaceId)]) ||
      null
    : null;
  const routeSurfaceId = activeRoute?.surfaceId ?? surfaceId ?? activeTool?.id ?? "";
  const routeDef =
    surfacesById[routeSurfaceId] ??
    surfacesById[surfaceIdForTool(routeSurfaceId)] ??
    null;
  const routeIsInteractive = hasInteractiveDefinition(routeDef);

  // Multi-route state: load and scope state per active surface with atomic revision.
  // Interactive routes must NOT seed an empty `{}` into perSurfaceState — `{}` is
  // truthy and would shadow the parent InteractiveView hydration forever.
  useEffect(() => {
    // Guard: no routes or in preview — do not load from backend
    if (!routeSurfaceId || !routes.length || isPreview) return;
    let cancelled = false;

    if (routeIsInteractive) {
      setPerSurfaceState((prev) => {
        if (!(routeSurfaceId in prev)) return prev;
        const next = { ...prev };
        delete next[routeSurfaceId];
        return next;
      });
      // Hydrate via authoritative interactive view (renderer-projected), never raw state.
      void api
        .interactiveView(routeSurfaceId)
        .then((view) => {
          if (cancelled || !view) return;
          setPerSurfaceRevision((prev) => ({
            ...prev,
            [routeSurfaceId]: view.stateRevision,
          }));
          if (view.state && typeof view.state === "object") {
            onStateChange(view.state as ToolState);
          }
        })
        .catch(() => {
          // Revision-only fallback: getSurfaceStateWithRevision is already
          // renderer-projected, but do not seed interactive parent state from it
          // (shape differs from InteractiveView).
          void api
            .getSurfaceStateWithRevision(routeSurfaceId)
            .then((res) => {
              if (cancelled || !res) return;
              setPerSurfaceRevision((prev) => ({
                ...prev,
                [routeSurfaceId]: res.stateRevision,
              }));
            })
            .catch(() => undefined);
        });
      return () => {
        cancelled = true;
      };
    }

    void api
      .getSurfaceStateWithRevision(routeSurfaceId)
      .then((res) => {
        if (cancelled || !res) return;
        setPerSurfaceRevision((prev) => ({ ...prev, [routeSurfaceId]: res.stateRevision }));
        if (res.state && typeof res.state === "object") {
          setPerSurfaceState((prev) => ({ ...prev, [routeSurfaceId]: res.state as ToolState }));
        }
      })
      .catch(() => {
        void api
          .getSurfaceState(routeSurfaceId)
          .then((st) => {
            if (!cancelled && st && typeof st === "object") {
              setPerSurfaceState((prev) => ({ ...prev, [routeSurfaceId]: st }));
            }
          })
          .catch(() => undefined);
      });
    return () => {
      cancelled = true;
    };
    // onStateChange intentionally omitted: parent often passes an unstable callback;
    // including it would re-hydrate on every parent render. Route identity drives hydrate.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see above
  }, [routeSurfaceId, isPreview, routes.length, routeIsInteractive]);

  const currentScopedState = useMemo(() => {
    if (routeIsInteractive) return state;
    if (routeSurfaceId && perSurfaceState[routeSurfaceId] !== undefined) {
      return perSurfaceState[routeSurfaceId];
    }
    return state;
  }, [routeIsInteractive, routeSurfaceId, perSurfaceState, state]);

  const handleRouteStateChange = useCallback(
    (nextState: ToolState) => {
      if (routeSurfaceId) {
        setPerSurfaceState((prev) => ({ ...prev, [routeSurfaceId]: nextState }));
      }
      onStateChange(nextState);
    },
    [routeSurfaceId, onStateChange],
  );

  const handleRoutePersistState = useCallback(
    async (nextState: ToolState) => {
      if (routeSurfaceId && !isPreview) {
        const currentRev = perSurfaceRevision[routeSurfaceId];
        try {
          const newRev = await api.saveSurfaceState(routeSurfaceId, nextState, currentRev);
          setPerSurfaceRevision((prev) => ({ ...prev, [routeSurfaceId]: newRev }));
        } catch {
          // On OCC conflict or failure, reload authoritative state and revision
          const fresh = await api.getSurfaceStateWithRevision(routeSurfaceId).catch(() => null);
          if (fresh) {
            setPerSurfaceRevision((prev) => ({ ...prev, [routeSurfaceId]: fresh.stateRevision }));
            if (routeIsInteractive) {
              // Parent owns interactive public state — do not shadow via perSurfaceState.
              onStateChange(fresh.state as ToolState);
            } else {
              setPerSurfaceState((prev) => ({
                ...prev,
                [routeSurfaceId]: fresh.state as ToolState,
              }));
            }
          }
        }
      }
      if (onPersistState) {
        await onPersistState(nextState);
      }
    },
    [
      routeSurfaceId,
      isPreview,
      onPersistState,
      onStateChange,
      perSurfaceRevision,
      routeIsInteractive,
    ],
  );

  // Early return is now AFTER all hooks — React rules compliant
  if (!routes.length) return null;

  return (
    <div className="app-route-shell" data-application-id={applicationId}>
      <header className="app-route-shell-header">
        <div className="button-row">
          <button
            type="button"
            className="btn btn-ghost"
            aria-label="Back"
            disabled={!routeState || !canNavigateBack(routeState.historyIndex)}
            onClick={() => void goBack()}
          >
            <ChevronLeft size={16} />
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            aria-label="Forward"
            disabled={
              !routeState ||
              !canNavigateForward(routeState.historyIndex, routeState.history.length)
            }
            onClick={() => void goForward()}
          >
            <ChevronRight size={16} />
          </button>
        </div>
        <nav aria-label="Application routes" className="app-route-nav">
          {routes.map((route) => (
            <button
              key={route.routeId}
              type="button"
              className={`btn btn-secondary${
                route.routeId === routeState?.currentRouteId ? " active" : ""
              }`}
              onClick={() => void navigate(route.routeId)}
            >
              {route.title}
            </button>
          ))}
        </nav>
      </header>
      {error ? <p className="tr-validation">{error}</p> : null}
      {activeTool ? (
        <ToolRenderer
          tool={activeTool}
          state={currentScopedState}
          onStateChange={handleRouteStateChange}
          onPersistState={handleRoutePersistState}
          onSubmitToAgent={onSubmitToAgent}
          applicationId={applicationId}
          surfaceId={routeSurfaceId}
          conversationId={conversationId}
          projectId={projectId}
          isCustomizing={isCustomizing}
          mode={mode}
          selectedComponentId={selectedComponentId}
          onSelectComponent={onSelectComponent}
          onPendingApproval={onPendingApproval}
        />
      ) : (
        <div className="empty-state">
          <p>
            {activeRoute
              ? `No surface is bound to route ${activeRoute.routeId}.`
              : "No active route."}
          </p>
        </div>
      )}
    </div>
  );
}

