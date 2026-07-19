import { api } from "@/lib/tauri";
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

export function mergeStateForDefinitionPatch(args: {
  previousComponents: ToolComponent[];
  nextComponents: ToolComponent[];
  previousState: Record<string, unknown>;
  hydratedState: Record<string, unknown>;
}): Record<string, unknown> {
  const prevIds = collectComponentIds(args.previousComponents);
  const nextIds = collectComponentIds(args.nextComponents);
  const merged = { ...args.hydratedState };
  for (const [key, value] of Object.entries(args.previousState)) {
    const ownerStillPresent = [...nextIds].some((id) => key === id || key.startsWith(`${id}:`));
    if (ownerStillPresent || prevIds.has(key)) {
      if (!(key in merged)) merged[key] = value;
    }
  }
  return merged;
}
