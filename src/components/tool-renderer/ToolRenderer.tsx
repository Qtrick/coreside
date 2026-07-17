import { Component, useCallback, useMemo, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle } from "lucide-react";
import { applyActions } from "@/lib/actions";
import type { ActionDefinition, ToolComponent, ToolDefinition, ToolState } from "@/types/tool";
import { ToolRuntimeProvider } from "./context";
import { resolveComponent } from "./registry";

type ToolRendererProps = {
  tool: ToolDefinition;
  state: ToolState;
  onStateChange: (state: ToolState) => void;
  onSubmitToAgent?: (payload: {
    toolId: string;
    eventName: string;
    componentId?: string;
    values: Record<string, unknown>;
  }) => void;
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
  onSubmitToAgent,
}: ToolRendererProps) {
  const allowedTargets = useMemo(
    () => collectTargets(tool.components ?? []),
    [tool.components],
  );

  const runActions = useCallback(
    (actions: ActionDefinition[], componentId?: string) => {
      const normalized = actions.map((action) =>
        action.type === "submitToAgent"
          ? { ...action, componentId: action.componentId ?? componentId }
          : action,
      );
      const result = applyActions(normalized, {
        state,
        toolId: tool.id,
        allowedTargets,
        onSubmitToAgent,
      });
      if (result.changedKeys.length > 0) {
        onStateChange(result.state);
      }
      if (result.errors.length > 0) {
        console.warn("Tool action errors", result.errors);
      }
    },
    [allowedTargets, onStateChange, onSubmitToAgent, state, tool.id],
  );

  const setValue = useCallback(
    (key: string, value: unknown) => {
      onStateChange({ ...state, [key]: value });
    },
    [onStateChange, state],
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
      getValue,
    }),
    [getValue, runActions, setValue, state, tool.id],
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
