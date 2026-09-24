import {
  Component,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ErrorInfo,
  type ReactNode,
} from "react";
import { AlertTriangle, X } from "lucide-react";
import { applyActionsAsync, collectDeclaredBindings, setDottedPath } from "@/lib/actions";
import { api } from "@/lib/tauri";
import type { ActionOutcome } from "@/types/application-kernel";
import type { ActionDefinition, DataSource, ToolComponent, ToolDefinition, ToolState } from "@/types/tool";
import { ToolRuntimeProvider } from "./context";
import { resolveComponent } from "./registry";
import { ToolLayoutContainer } from "./ToolLayoutContainer";

type ToolRendererProps = {
  tool: ToolDefinition;
  state: ToolState;
  onStateChange: (state: ToolState) => void;
  onPersistState?: (state: ToolState) => Promise<void>;
  applicationId?: string | null;
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

type ToolErrorBoundaryProps = {
  children: ReactNode;
  componentId?: string;
  resetKey?: string;
  applicationId?: string | null;
  toolId?: string | null;
};

/** One report per application + definition until the tool version changes. */
const reportedBuildFailures = new Set<string>();
const MAX_REPORTED_BUILD_FAILURES = 200;

function rememberBuildFailure(dedupeKey: string): boolean {
  if (reportedBuildFailures.has(dedupeKey)) return false;
  if (reportedBuildFailures.size >= MAX_REPORTED_BUILD_FAILURES) {
    reportedBuildFailures.clear();
  }
  reportedBuildFailures.add(dedupeKey);
  return true;
}

class ToolErrorBoundary extends Component<
  ToolErrorBoundaryProps,
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Tool component failed", error, info);

    const applicationId = this.props.applicationId?.trim();
    if (!applicationId) return;

    const resetKey = this.props.resetKey ?? "";
    const dedupeKey = `${applicationId}:${resetKey}`;
    if (!rememberBuildFailure(dedupeKey)) return;

    const name = error?.name || "Error";
    const detail = String(error?.message ?? "").slice(0, 180);
    const safeMessage = `${name}: ${detail}`.slice(0, 280);

    // Fire-and-forget: a reporter outage must not remount-loop this boundary.
    void api
      .kernelRecordBuildFailure(
        applicationId,
        safeMessage,
        false,
        this.props.toolId ?? null,
      )
      .catch(() => undefined);
  }

  /// A boundary that latched on an old definition must recover once the tool is
  /// edited, restored to a known-good version, or reopened.
  componentDidUpdate(prev: ToolErrorBoundaryProps) {
    if (this.state.error && prev.resetKey !== this.props.resetKey) {
      this.setState({ error: null });
    }
  }

  render() {
    if (this.state.error) {
      return (
        <div className="tr-fallback" role="alert">
          <AlertTriangle size={16} aria-hidden />{" "}
          This part of the tool failed to render
          {this.props.componentId ? ` (${this.props.componentId})` : ""}.
        </div>
      );
    }
    return this.props.children;
  }
}

function RenderNode({
  component,
  resetKey,
  applicationId,
  toolId,
  isCustomizing,
  selectedComponentId,
  onSelectComponent,
}: {
  component: ToolComponent;
  resetKey?: string;
  applicationId?: string | null;
  toolId?: string | null;
  isCustomizing?: boolean;
  selectedComponentId?: string | null;
  onSelectComponent?: (component: ToolComponent) => void;
}) {
  const Node = resolveComponent(component.type);
  const renderChild = (child: ToolComponent) => (
    <RenderNode
      key={child.id}
      component={child}
      resetKey={resetKey}
      applicationId={applicationId}
      toolId={toolId}
      isCustomizing={isCustomizing}
      selectedComponentId={selectedComponentId}
      onSelectComponent={onSelectComponent}
    />
  );

  const isSelected = isCustomizing && selectedComponentId === component.id;

  const handleClick = (e: React.MouseEvent) => {
    if (!isCustomizing || !onSelectComponent) return;
    onSelectComponent(component);
    const target = e.target as HTMLElement | null;
    const isInteractiveControl = Boolean(
      target?.closest(
        "input, textarea, select, button, a, [contenteditable='true'], [role='button'], [role='switch'], [role='checkbox'], [role='tab']",
      ) && !target?.closest(".tr-item-badge, .tr-item-border, .tr-customize-handle"),
    );
    if (!isInteractiveControl) {
      e.stopPropagation();
    }
  };

  return (
    <ToolErrorBoundary
      componentId={component.id}
      resetKey={resetKey}
      applicationId={applicationId}
      toolId={toolId}
    >
      <div
        className={`tr-node-wrapper${isSelected ? " tr-node-selected" : ""}`}
        data-component-id={component.id}
        data-component-type={component.type}
        data-is-selected={isSelected ? "true" : undefined}
        onClick={isCustomizing ? handleClick : undefined}
      >
        <Node component={component} renderChild={renderChild} />
      </div>
    </ToolErrorBoundary>
  );
}

export function ToolRenderer({
  tool,
  state,
  onStateChange,
  onPersistState,
  applicationId,
  surfaceId,
  conversationId,
  projectId,
  isCustomizing,
  mode,
  selectedComponentId,
  onSelectComponent,
  onSubmitToAgent,
  onPendingApproval,
}: ToolRendererProps) {
  const effectiveMode = mode ?? (isCustomizing ? "customize" : "live");
  const isPreviewMode = effectiveMode === "preview";
  const isCustomizingMode = effectiveMode === "customize" || isCustomizing;

  const declaredBindings = useMemo(
    () => collectDeclaredBindings(tool.components ?? []),
    [tool.components],
  );
  // Track authoritative live state in a ref to prevent in-flight async action results
  // from overwriting concurrent user input.
  const stateRef = useRef(state);
  stateRef.current = state;

  const dataSources = useMemo(() => {
    const list: DataSource[] = [];
    if (tool.dataSource) list.push(tool.dataSource);
    if (Array.isArray(tool.dataSources)) list.push(...tool.dataSources);
    const scanComps = (comps?: ToolComponent[]) => {
      if (!comps) return;
      for (const comp of comps) {
        if (comp.props?.dataSource && typeof comp.props.dataSource === "object") {
          const ds = comp.props.dataSource as DataSource;
          if (ds.actionName && ds.resultKey) list.push(ds);
        }
        if (comp.children) scanComps(comp.children);
      }
    };
    scanComps(tool.components);
    return list;
  }, [tool.dataSource, tool.dataSources, tool.components]);

  const hydrationRef = useRef<Record<string, { lastRun: number; depSignature: string }>>({});

  useEffect(() => {
    if (isPreviewMode || isCustomizingMode) return;
    if (dataSources.length === 0) return;

    for (const ds of dataSources) {
      if (!ds.actionName || (!ds.actionName.endsWith(".query") && ds.actionName !== "local_data.query")) {
        console.warn(`[ToolRenderer] Rejected non-read-only dataSource action: ${ds.actionName}`);
        continue;
      }
      if (!ds.resultKey) continue;

      const depValues = (ds.refreshOn ?? []).map((k) => String(stateRef.current[k]));
      const depSignature = depValues.join("::");

      const existing = hydrationRef.current[ds.resultKey];
      const now = Date.now();
      if (existing) {
        if (existing.depSignature === depSignature && now - existing.lastRun < 500) {
          continue;
        }
        if (existing.depSignature === depSignature) {
          continue;
        }
      }

      hydrationRef.current[ds.resultKey] = { lastRun: now, depSignature };

      const input: Record<string, unknown> = { ...(ds.input ?? {}) };
      if (ds.inputFromState) {
        for (const [key, stateKey] of Object.entries(ds.inputFromState)) {
          if (stateRef.current[stateKey] !== undefined) {
            setDottedPath(input, key, stateRef.current[stateKey]);
          }
        }
      }

      void api
        .kernelInvokeRegisteredAction({
          actionName: ds.actionName,
          input,
          applicationId: applicationId ?? tool.id,
          surfaceId: surfaceId ?? tool.id,
          componentId: null,
          conversationId: conversationId ?? null,
          projectId: projectId ?? null,
        })
        .then((outcome) => {
          if (outcome && outcome.status === "ok" && outcome.data !== undefined) {
            const currentVal = stateRef.current[ds.resultKey];
            if (JSON.stringify(currentVal) !== JSON.stringify(outcome.data)) {
              onStateChange({ ...stateRef.current, [ds.resultKey]: outcome.data });
            }
          }
        })
        .catch((err) => {
          console.error(`[ToolRenderer] dataSource hydration failed for ${ds.resultKey}:`, err);
        });
    }
  }, [
    dataSources,
    isPreviewMode,
    isCustomizingMode,
    applicationId,
    surfaceId,
    conversationId,
    projectId,
    tool.id,
    onStateChange,
    state,
  ]);

  // Interaction failures are shown in the surface itself: a button that cannot
  // do anything must say so rather than only logging to the console.
  const [actionError, setActionError] = useState<string | null>(null);

  const runActions = useCallback(
    async (actions: ActionDefinition[], componentId?: string) => {
      // In preview mode or customize mode, generated actions must not fire.
      if (isPreviewMode || isCustomizingMode) return;
      setActionError(null);
      const normalized = actions.map((action) => {
        if (action.type === "submitToAgent") {
          return { ...action, componentId: action.componentId ?? componentId };
        }
        if (action.type === "invokeRegisteredAction") {
          return { ...action, componentId: action.componentId ?? componentId };
        }
        return action;
      });
      const result = await applyActionsAsync(normalized, {
        state: stateRef.current,
        toolId: tool.id,
        bindings: declaredBindings,
        onSubmitToAgent: isPreviewMode ? undefined : onSubmitToAgent,
        onInvokeRegisteredAction: async (payload: {
          toolId: string;
          actionName: string;
          input: Record<string, unknown>;
          componentId?: string;
          resultKey?: string;
        }) => {
          if (isPreviewMode) {
            return {
              status: "blocked" as const,
              reason: "Actions are inert during speculative preview",
              durationMs: 0,
            };
          }
          const outcome = await api.kernelInvokeRegisteredAction({
            actionName: payload.actionName,
            input: payload.input,
            applicationId: applicationId ?? tool.id,
            surfaceId: surfaceId ?? tool.id,
            componentId: payload.componentId ?? componentId ?? null,
            conversationId: conversationId ?? null,
            projectId: projectId ?? null,
          });
          if (outcome.status === "pendingApproval") {
            onPendingApproval?.(outcome);
          } else if (outcome.status === "error") {
            setActionError(outcome.message);
          } else if (outcome.status === "blocked") {
            setActionError(outcome.reason);
          }
          return outcome;
        },
      });
      if (result.changedKeys.length > 0) {
        // Merge only modified keys into the latest live state to avoid stale-state overwrites
        const mergedState = { ...stateRef.current };
        for (const key of result.changedKeys) {
          mergedState[key] = result.state[key];
        }
        if (onPersistState) {
          void onPersistState(mergedState).catch(() => {
            setActionError("That change could not be saved.");
          });
        } else {
          onStateChange(mergedState);
        }
      }
      if (result.errors.length > 0) {
        setActionError(result.errors[0]);
      }
    },
    [
      isCustomizing,
      declaredBindings,
      applicationId,
      conversationId,
      onPendingApproval,
      onPersistState,
      onStateChange,
      onSubmitToAgent,
      projectId,
      surfaceId,
      tool.id,
    ],
  );

  const setValue = useCallback(
    (key: string, value: unknown) => {
      onStateChange({ ...stateRef.current, [key]: value });
    },
    [onStateChange],
  );

  const setValueOptimistic = useCallback(
    (key: string, value: unknown) => {
      const next = { ...stateRef.current, [key]: value };
      if (onPersistState) {
        void onPersistState(next).catch(() => {
          setActionError("That change could not be saved.");
        });
      } else {
        onStateChange(next);
      }
    },
    [onPersistState, onStateChange],
  );

  const getValue = useCallback(
    <T,>(key: string, fallback?: T): T => {
      const current = stateRef.current;
      if (key in current) return current[key] as T;
      return fallback as T;
    },
    [],
  );

  const runtime = useMemo(
    () => ({
      toolId: tool.id,
      state,
      runActions,
      setValue,
      setValueOptimistic,
      getValue,
    }),
    [getValue, runActions, setValue, setValueOptimistic, state, tool.id],
  );

  const handleCanvasClick = useCallback(
    (e: React.MouseEvent) => {
      if (!isCustomizing || !onSelectComponent) return;
      const target = e.target as HTMLElement | null;
      if (!target) return;
      const compEl = target.closest("[data-component-id]") as HTMLElement | null;
      if (compEl) {
        const id = compEl.getAttribute("data-component-id");
        if (id) {
          // Find the deepest component in the tree matching the DOM id.
          // deepest holds the last match found during DFS, ensuring
          // nested components are selected over their parents.
          let deepest: ToolComponent | null = null;
          const findInTree = (nodes: ToolComponent[]): void => {
            for (const n of nodes) {
              if (n.id === id) deepest = n;
              if (n.children) findInTree(n.children);
            }
          };
          findInTree(tool.components ?? []);
          if (deepest) {
            onSelectComponent(deepest);
            // Allow native form control and editable interaction (focus, typing, toggle)
            // instead of swallowing the click event. Only stop propagation on container cards/badges.
            const isInteractiveControl = Boolean(
              target.closest(
                "input, textarea, select, button, a, [contenteditable='true'], [role='button'], [role='switch'], [role='checkbox'], [role='tab']",
              ) && !target.closest(".tr-item-badge, .tr-item-border, .tr-customize-handle"),
            );
            if (!isInteractiveControl) {
              e.stopPropagation();
            }
          }
        }
      }
    },
    [isCustomizing, onSelectComponent, tool.components],
  );

  if (!tool.components?.length) {
    return (
      <div className="empty-state">
        <h3>Empty tool</h3>
        <p>This tool has no components yet.</p>
      </div>
    );
  }

  const resetKey = `${tool.id}:${tool.version ?? 1}`;

  return (
    <ToolRuntimeProvider value={runtime}>
      <div
        className="tr-container"
        data-tool-id={tool.id}
        data-customizing={isCustomizing ? "true" : undefined}
        onClick={isCustomizing ? handleCanvasClick : undefined}
      >
        {actionError ? (
          <div className="tr-action-error" role="alert">
            <AlertTriangle size={16} aria-hidden />
            <span>{actionError}</span>
            <button
              type="button"
              className="icon-btn"
              onClick={() => setActionError(null)}
              aria-label="Dismiss error"
            >
              <X size={14} aria-hidden />
            </button>
          </div>
        ) : null}
        <ToolLayoutContainer
          layout={tool.layout}
          components={tool.components}
          toolId={tool.id}
          isCustomizing={isCustomizing}
          selectedComponentId={selectedComponentId}
          onSelectComponent={onSelectComponent}
          renderComponent={(component) => (
            <RenderNode
              key={component.id}
              component={component}
              resetKey={resetKey}
              applicationId={applicationId}
              toolId={tool.id}
              isCustomizing={isCustomizing}
              selectedComponentId={selectedComponentId}
              onSelectComponent={onSelectComponent}
            />
          )}
        />
      </div>
    </ToolRuntimeProvider>
  );
}
