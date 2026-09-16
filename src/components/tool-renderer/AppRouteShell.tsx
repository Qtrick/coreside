import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { api } from "@/lib/tauri";
import {
  canNavigateBack,
  canNavigateForward,
  isRouteNavigationNoOp,
} from "@/lib/route-navigation";
import type { ActionOutcome, ApplicationManifest } from "@/types/application-kernel";
import type { RouteState } from "@/types/runtime-v2";
import type { ToolDefinition, ToolState } from "@/types/tool";
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
  onSubmitToAgent,
  onPendingApproval,
}: AppRouteShellProps) {
  const routes = useMemo(() => manifest?.routes ?? [], [manifest?.routes]);
  const [routeState, setRouteState] = useState<RouteState | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadRouteState = useCallback(async () => {
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
  }, [applicationId, routes]);

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
    activeRoute?.surfaceId && surfacesById[activeRoute.surfaceId]
      ? surfacesById[activeRoute.surfaceId]
      : null;

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
          state={state}
          onStateChange={onStateChange}
          onPersistState={onPersistState}
          onSubmitToAgent={onSubmitToAgent}
          applicationId={applicationId}
          surfaceId={surfaceId ?? activeTool.id}
          conversationId={conversationId}
          projectId={projectId}
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
