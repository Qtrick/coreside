import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { api } from "@/lib/tauri";
import { surfaceIdForTool } from "@/lib/surface-ops";
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
      const seeded = await api.setRouteState({
        applicationId,
        currentRouteId: first.routeId,
        routeParams: {},
        history: [{ routeId: first.routeId, params: {} }],
        historyIndex: 0,
      });
      setRouteState(seeded);
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
    const nextIndex = routeState.historyIndex - 1;
    const entry = routeState.history[nextIndex] as {
      routeId?: string;
      params?: Record<string, unknown>;
    };
    if (!entry?.routeId) return;
    if (isPreview) {
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
    const next = await api.setRouteState({
      applicationId,
      currentRouteId: entry.routeId,
      routeParams: entry.params ?? {},
      history: routeState.history,
      historyIndex: nextIndex,
    });
    setRouteState(next);
  };

  const goForward = async () => {
    if (
      !routeState ||
      !canNavigateForward(routeState.historyIndex, routeState.history.length)
    ) {
      return;
    }
    const nextIndex = routeState.historyIndex + 1;
    const entry = routeState.history[nextIndex] as {
      routeId?: string;
      params?: Record<string, unknown>;
    };
    if (!entry?.routeId) return;
    if (isPreview) {
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
    const next = await api.setRouteState({
      applicationId,
      currentRouteId: entry.routeId,
      routeParams: entry.params ?? {},
      history: routeState.history,
      historyIndex: nextIndex,
    });
    setRouteState(next);
  };

  if (!routes.length) return null;

  const activeRoute =
    routes.find((route) => route.routeId === routeState?.currentRouteId) ??
    routes[0];
  const activeTool =
    (activeRoute?.surfaceId && surfacesById[activeRoute.surfaceId]) ||
    (activeRoute?.surfaceId && surfacesById[surfaceIdForTool(activeRoute.surfaceId)]) ||
    null;
  const routeSurfaceId = activeRoute?.surfaceId ?? surfaceId ?? activeTool?.id ?? "";

  // Multi-route state: load and scope state per active surface
  useEffect(() => {
    if (!routeSurfaceId || isPreview) return;
    let cancelled = false;
    void api.getSurfaceState(routeSurfaceId).then((st) => {
      if (!cancelled && st && typeof st === "object") {
        setPerSurfaceState((prev) => ({ ...prev, [routeSurfaceId]: st }));
      }
    }).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [routeSurfaceId, isPreview]);

  const currentScopedState = useMemo(() => {
    if (routeSurfaceId && perSurfaceState[routeSurfaceId]) {
      return perSurfaceState[routeSurfaceId];
    }
    return state;
  }, [routeSurfaceId, perSurfaceState, state]);

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
        await api.saveSurfaceState(routeSurfaceId, nextState).catch(() => {});
      }
      if (onPersistState) {
        await onPersistState(nextState);
      }
    },
    [routeSurfaceId, isPreview, onPersistState],
  );

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
          <p>No surface is bound to route {activeRoute?.routeId}.</p>
        </div>
      )}
    </div>
  );
}
