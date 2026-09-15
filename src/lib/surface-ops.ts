import { api } from "@/lib/tauri";
import { shouldPreserveComponent, type PreservationPolicy } from "@/lib/preservation";
import type { AppOperation } from "@/types/runtime-v2";
import type { ToolComponent } from "@/types/tool";

/** Matches Rust `runtime_v2::surfaces::surface_id_for_tool`. */
export function surfaceIdForTool(toolId: string): string {
  return `surf-${toolId}`;
}

function opId(): string {
  return crypto.randomUUID();
}

export function makeUpdatePropsOp(args: {
  surfaceId: string;
  componentId: string;
  props: Record<string, unknown>;
  baseRevision: number;
}): AppOperation {
  return {
    id: opId(),
    type: "component.update_props",
    target: {
      surfaceId: args.surfaceId,
      componentId: args.componentId,
    },
    baseRevision: args.baseRevision,
    payload: { props: args.props },
  };
}

export function makeReplaceComponentOp(args: {
  surfaceId: string;
  componentId: string;
  component: ToolComponent;
  baseRevision: number;
}): AppOperation {
  return {
    id: opId(),
    type: "component.replace",
    target: {
      surfaceId: args.surfaceId,
      componentId: args.componentId,
    },
    baseRevision: args.baseRevision,
    payload: { component: args.component },
  };
}

export function makeHideComponentOp(args: {
  surfaceId: string;
  componentId: string;
  baseRevision: number;
  visible?: boolean;
}): AppOperation {
  return {
    id: opId(),
    type: "component.update_visibility",
    target: {
      surfaceId: args.surfaceId,
      componentId: args.componentId,
    },
    baseRevision: args.baseRevision,
    payload: { visible: args.visible ?? false },
  };
}

export function makeMoveComponentOp(args: {
  surfaceId: string;
  componentId: string;
  parentId?: string | null;
  index: number;
  baseRevision: number;
}): AppOperation {
  return {
    id: opId(),
    type: "component.move",
    target: {
      surfaceId: args.surfaceId,
      componentId: args.componentId,
      parentId: args.parentId ?? undefined,
    },
    baseRevision: args.baseRevision,
    payload: { index: args.index },
  };
}

export async function applyDirectManipulationOps(args: {
  conversationId?: string | null;
  surfaceId: string;
  operations: AppOperation[];
}): Promise<void> {
  await api.schedulePatches({
    conversationId: args.conversationId ?? null,
    surfaceId: args.surfaceId,
    priority: "direct_user_interaction",
    operations: args.operations,
    sourceType: "direct_manipulation",
    fromAgent: false,
    applyImmediately: true,
    approvalGranted: true,
  });
}

export function flattenComponents(
  components: ToolComponent[],
  parentId: string | null = null,
  out: Array<{ component: ToolComponent; parentId: string | null; index: number }> = [],
): Array<{ component: ToolComponent; parentId: string | null; index: number }> {
  components.forEach((component, index) => {
    out.push({ component, parentId, index });
    if (component.children?.length) {
      flattenComponents(component.children, component.id, out);
    }
  });
  return out;
}

export function collectComponentIds(components: ToolComponent[]): Set<string> {
  return new Set(flattenComponents(components).map((row) => row.component.id));
}

function componentById(
  components: ToolComponent[],
  id: string,
): ToolComponent | undefined {
  return flattenComponents(components).find((row) => row.component.id === id)
    ?.component;
}

function policyFromProps(
  component: ToolComponent | undefined,
): PreservationPolicy {
  const raw = component?.props?.preservationPolicy;
  if (
    typeof raw === "string" &&
    [
      "replace",
      "preserve_instance",
      "preserve_state",
      "preserve_user_input",
      "preserve_media_state",
      "preserve_scroll",
      "preserve_focus",
      "preserve_selection",
      "preserve_if_compatible",
      "reset_explicitly",
    ].includes(raw)
  ) {
    return raw as PreservationPolicy;
  }
  return "preserve_if_compatible";
}

/** Keep live state for components that survive a definition patch under policy. */
export function mergeStateForDefinitionPatch(args: {
  previousComponents: ToolComponent[];
  nextComponents: ToolComponent[];
  previousState: Record<string, unknown>;
  hydratedState: Record<string, unknown>;
}): Record<string, unknown> {
  const nextIds = collectComponentIds(args.nextComponents);
  const merged = { ...args.hydratedState };
  for (const [key, value] of Object.entries(args.previousState)) {
    const ownerId = [...nextIds].find(
      (id) => key === id || key.startsWith(`${id}:`),
    );
    if (!ownerId) continue;
    const prev = componentById(args.previousComponents, ownerId);
    const next = componentById(args.nextComponents, ownerId);
    if (!next) continue;
    const policy =
      policyFromProps(next) !== "preserve_if_compatible"
        ? policyFromProps(next)
        : policyFromProps(prev);
    if (!shouldPreserveComponent(policy, prev?.type ?? "", next.type)) {
      continue;
    }
    if (!(key in merged)) merged[key] = value;
  }
  return merged;
}
