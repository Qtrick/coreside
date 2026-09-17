import { api } from "@/lib/tauri";
import { shouldPreserveComponent, type PreservationPolicy } from "@/lib/preservation";
import type { AppOperation } from "@/types/runtime-v2";
import type { ActionDefinition, ToolComponent } from "@/types/tool";

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

export function makeInsertComponentOp(args: {
  surfaceId: string;
  parentId?: string | null;
  component: ToolComponent;
  index?: number;
  baseRevision: number;
}): AppOperation {
  return {
    id: opId(),
    type: "component.insert",
    target: {
      surfaceId: args.surfaceId,
      parentId: args.parentId ?? undefined,
    },
    baseRevision: args.baseRevision,
    payload: {
      component: args.component,
      index: args.index,
    },
  };
}

export function makeRemoveComponentOp(args: {
  surfaceId: string;
  componentId: string;
  baseRevision: number;
}): AppOperation {
  return {
    id: opId(),
    type: "component.remove",
    target: {
      surfaceId: args.surfaceId,
      componentId: args.componentId,
    },
    baseRevision: args.baseRevision,
    payload: {},
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
}): Promise<import("@/types/runtime-v2").ScheduledPatch[]> {
  return await api.schedulePatches({
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

/**
 * Safely duplicates a component tree:
 * 1. Generates fresh, unique IDs for the root and all recursive descendants.
 * 2. Isolates preservation keys so the duplicated instance does not steal focus/caret.
 * 3. Disambiguates interactive input `valueKey` bindings (e.g. `title` -> `title_copy_<suffix>`)
 *    so edits to the clone do not clobber the original's live state.
 * 4. Preserves shared read-only collections (rowsKey, dataKey, itemsKey) so the clone displays
 *    the expected dataset without broken bindings.
 */
export function safeDuplicateComponent(
  component: ToolComponent,
  idSuffix: string = Math.random().toString(36).substring(2, 8),
): ToolComponent {
  const idMap = new Map<string, string>();
  const keyMap = new Map<string, string>();

  // Pass 1: generate all new IDs and collect valueKey remappings
  const preScan = (node: ToolComponent) => {
    const newId = `${node.type}-${idSuffix}-${Math.random().toString(36).substring(2, 6)}`;
    idMap.set(node.id, newId);

    const valKey =
      typeof node.valueKey === "string" && node.valueKey.trim()
        ? node.valueKey.trim()
        : typeof node.props?.valueKey === "string" && node.props.valueKey.trim()
          ? (node.props.valueKey as string).trim()
          : null;

    if (valKey && !keyMap.has(valKey)) {
      keyMap.set(valKey, `${valKey}_copy_${idSuffix}`);
    }

    if (typeof node.props?.preservationKey === "string" && node.props.preservationKey.trim()) {
      const presKey = (node.props.preservationKey as string).trim();
      if (!keyMap.has(presKey)) {
        keyMap.set(presKey, `${presKey}_copy_${idSuffix}`);
      }
    }

    if (typeof node.props?.selectionKey === "string" && node.props.selectionKey.trim()) {
      const selKey = (node.props.selectionKey as string).trim();
      if (!keyMap.has(selKey)) {
        keyMap.set(selKey, `${selKey}_copy_${idSuffix}`);
      }
    }

    node.children?.forEach(preScan);
  };

  preScan(component);

  // Helper to remap an action
  const remapAction = (action: ActionDefinition, owningNodeId: string): ActionDefinition => {
    const cloned: ActionDefinition = { ...action };

    // Remap componentId if present
    if ("componentId" in cloned && typeof cloned.componentId === "string") {
      if (idMap.has(cloned.componentId)) {
        cloned.componentId = idMap.get(cloned.componentId);
      } else if (cloned.componentId === owningNodeId) {
        cloned.componentId = idMap.get(owningNodeId);
      }
    }

    // Remap state target
    if ("target" in cloned && typeof cloned.target === "string" && keyMap.has(cloned.target)) {
      cloned.target = keyMap.get(cloned.target)!;
    }

    // Remap submitToAgent includeFields
    if ("includeFields" in cloned && Array.isArray(cloned.includeFields)) {
      cloned.includeFields = cloned.includeFields.map((f: string) => keyMap.get(f) ?? f);
    }

    // Remap invokeRegisteredAction inputFromState
    if ("inputFromState" in cloned && cloned.inputFromState && typeof cloned.inputFromState === "object") {
      const remappedInput: Record<string, string> = {};
      for (const [param, stateKey] of Object.entries(cloned.inputFromState as Record<string, string>)) {
        remappedInput[param] = typeof stateKey === "string" ? (keyMap.get(stateKey) ?? stateKey) : String(stateKey);
      }
      cloned.inputFromState = remappedInput;
    }

    // Remap resultKey if it was pointing to an isolated interactive state
    if ("resultKey" in cloned && typeof cloned.resultKey === "string" && keyMap.has(cloned.resultKey)) {
      cloned.resultKey = keyMap.get(cloned.resultKey);
    }

    return cloned;
  };

  // Pass 2: clone nodes with remapped IDs, keys, and actions
  const cloneNode = (node: ToolComponent): ToolComponent => {
    const newId = idMap.get(node.id) ?? `${node.type}-${idSuffix}-${Math.random().toString(36).substring(2, 6)}`;
    const clonedProps: Record<string, unknown> = node.props ? { ...node.props } : {};

    if (typeof clonedProps.valueKey === "string" && keyMap.has(clonedProps.valueKey)) {
      clonedProps.valueKey = keyMap.get(clonedProps.valueKey);
    }

    if (typeof clonedProps.preservationKey === "string" && keyMap.has(clonedProps.preservationKey)) {
      clonedProps.preservationKey = keyMap.get(clonedProps.preservationKey);
    }

    if (typeof clonedProps.selectionKey === "string" && keyMap.has(clonedProps.selectionKey)) {
      clonedProps.selectionKey = keyMap.get(clonedProps.selectionKey);
    }

    const remappedActions = node.actions?.map((act) => remapAction(act, node.id));

    const children = node.children?.map((child) => cloneNode(child));

    const clonedNode: ToolComponent = {
      ...node,
      id: newId,
      props: clonedProps,
      actions: remappedActions && remappedActions.length > 0 ? remappedActions : undefined,
      children: children && children.length > 0 ? children : undefined,
    };

    if (typeof node.valueKey === "string" && keyMap.has(node.valueKey)) {
      clonedNode.valueKey = keyMap.get(node.valueKey);
    }

    return clonedNode;
  };

  return cloneNode(component);
}


