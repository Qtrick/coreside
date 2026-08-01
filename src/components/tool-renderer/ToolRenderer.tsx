import { Component, useCallback, useMemo, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle } from "lucide-react";
import { applyActions } from "@/lib/actions";
import { api } from "@/lib/tauri";
import type { ActionOutcome } from "@/types/application-kernel";
import type { ActionDefinition, ToolComponent, ToolDefinition, ToolState } from "@/types/tool";
import { ToolRuntimeProvider } from "./context";
import { resolveComponent } from "./registry";

type ToolRendererProps = {
  tool: ToolDefinition;
  state: ToolState;
  onStateChange: (state: ToolState) => void;
  onPersistState?: (state: ToolState) => Promise<void>;
  applicationId?: string | null;
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

class ToolErrorBoundary extends Component<
  { children: ReactNode; componentId?: string },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Tool component failed", error, info);
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

function collectTargets(components: ToolComponent[], into = new Set<string>()) {
  for (const component of components) {
    into.add(component.id);
    into.add(`${component.id}:tab`);
    if (component.valueKey) into.add(component.valueKey);
    const props = component.props ?? {};
    for (const key of ["valueKey", "stateKey", "target"] as const) {
      const value = props[key];
      if (typeof value === "string" && value.trim()) into.add(value);
    }
    if (Array.isArray(component.actions)) {
      for (const action of component.actions) {
        if ("target" in action && typeof action.target === "string") {
          into.add(action.target);
        }
        if (
          action.type === "invokeRegisteredAction" &&
          action.inputFromState
        ) {
          for (const stateKey of Object.values(action.inputFromState)) {
            if (stateKey.trim()) into.add(stateKey);
          }
        }
      }
    }
    if (component.children) collectTargets(component.children, into);
  }
  return into;
}

function RenderNode({ component }: { component: ToolComponent }) {
  const Node = resolveComponent(component.type);
  const renderChild = (child: ToolComponent) => <RenderNode component={child} />;

  return (
    <ToolErrorBoundary componentId={component.id}>
      <Node component={component} renderChild={renderChild} />
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
  onSubmitToAgent,
  onPendingApproval,
}: ToolRendererProps) {
  const allowedTargets = useMemo(
    () => collectTargets(tool.components ?? []),
    [tool.components],
  );

  const runActions = useCallback(
    async (actions: ActionDefinition[], componentId?: string) => {
      const normalized = actions.map((action) => {
        if (action.type === "submitToAgent") {
          return { ...action, componentId: action.componentId ?? componentId };
        }
        if (action.type === "invokeRegisteredAction") {
          return { ...action, componentId: action.componentId ?? componentId };
        }
        return action;
      });
      const result = applyActions(normalized, {
        state,
        toolId: tool.id,
        allowedTargets,
        onSubmitToAgent,
        onInvokeRegisteredAction: async (payload) => {
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
          } else if (outcome.status === "error" || outcome.status === "blocked") {
            console.warn("Registered action failed", outcome);
          }
        },
      });
      if (result.changedKeys.length > 0) {
        onStateChange(result.state);
      }
      if (result.errors.length > 0) {
        console.warn("Tool action errors", result.errors);
      }
      if (result.pendingTasks.length > 0) {
        await Promise.all(result.pendingTasks);
      }
    },
    [
      allowedTargets,
      applicationId,
      conversationId,
      onPendingApproval,
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
      const previous = state[key];
      const next = { ...state, [key]: value };
      onStateChange(next);
      const persist = onPersistState ?? (async (saved) => onStateChange(saved));
      void persist(next).catch(() => {
        onStateChange({ ...state, [key]: previous });
      });
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

  if (!tool.components?.length) {
    return (
      <div className="empty-state">
        <h3>Empty tool</h3>
        <p>This tool has no components yet.</p>
      </div>
    );
  }

  return (
    <ToolRuntimeProvider value={runtime}>
      <div className="tr-container" data-tool-id={tool.id}>
        {tool.components.map((component) => (
          <RenderNode key={component.id} component={component} />
        ))}
      </div>
    </ToolRuntimeProvider>
  );
}
