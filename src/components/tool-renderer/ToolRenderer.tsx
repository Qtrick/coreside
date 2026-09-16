import {
  Component,
  useCallback,
  useMemo,
  useState,
  type ErrorInfo,
  type ReactNode,
} from "react";
import { AlertTriangle, X } from "lucide-react";
import { applyActionsAsync, collectDeclaredBindings } from "@/lib/actions";
import { api } from "@/lib/tauri";
import type { ActionOutcome } from "@/types/application-kernel";
import type { ActionDefinition, ToolComponent, ToolDefinition, ToolState } from "@/types/tool";
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
}: {
  component: ToolComponent;
  resetKey?: string;
  applicationId?: string | null;
  toolId?: string | null;
  isCustomizing?: boolean;
  selectedComponentId?: string | null;
}) {
  const Node = resolveComponent(component.type);
  const renderChild = (child: ToolComponent) => (
    <RenderNode
      component={child}
      resetKey={resetKey}
      applicationId={applicationId}
      toolId={toolId}
      isCustomizing={isCustomizing}
      selectedComponentId={selectedComponentId}
    />
  );

  const isSelected = isCustomizing && selectedComponentId === component.id;

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
  selectedComponentId,
  onSelectComponent,
  onSubmitToAgent,
  onPendingApproval,
}: ToolRendererProps) {
  const declaredBindings = useMemo(
    () => collectDeclaredBindings(tool.components ?? []),
    [tool.components],
  );
  // Interaction failures are shown in the surface itself: a button that cannot
  // do anything must say so rather than only logging to the console.
  const [actionError, setActionError] = useState<string | null>(null);

  const runActions = useCallback(
    async (actions: ActionDefinition[], componentId?: string) => {
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
        state,
        toolId: tool.id,
        bindings: declaredBindings,
        onSubmitToAgent,
        onInvokeRegisteredAction: async (payload: {
          toolId: string;
          actionName: string;
          input: Record<string, unknown>;
          componentId?: string;
          resultKey?: string;
        }) => {
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
        if (onPersistState) {
          void onPersistState(result.state).catch(() => {
            setActionError("That change could not be saved.");
          });
        } else {
          onStateChange(result.state);
        }
      }
      if (result.errors.length > 0) {
        setActionError(result.errors[0]);
      }
    },
    [
      declaredBindings,
      applicationId,
      conversationId,
      onPendingApproval,
      onPersistState,
      onStateChange,
      onSubmitToAgent,
      projectId,
      state,
      surfaceId,
      tool.id,
    ],
  );

  const setValue = useCallback(
    (key: string, value: unknown) => {
      onStateChange({ ...state, [key]: value });
    },
    [onStateChange, state],
  );

  const setValueOptimistic = useCallback(
    (key: string, value: unknown) => {
      const next = { ...state, [key]: value };
      if (onPersistState) {
        void onPersistState(next).catch(() => {
          setActionError("That change could not be saved.");
        });
      } else {
        onStateChange(next);
      }
    },
    [onPersistState, onStateChange, state],
  );

  const getValue = useCallback(
    <T,>(key: string, fallback?: T): T => {
      if (key in state) return state[key] as T;
      return fallback as T;
    },
    [state],
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
            e.stopPropagation();
            onSelectComponent(deepest);
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
            />
          )}
        />
      </div>
    </ToolRuntimeProvider>
  );
}
