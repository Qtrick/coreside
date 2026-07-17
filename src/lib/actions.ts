import type { ActionDefinition, ToolState } from "@/types/tool";

const MAX_ACTION_DEPTH = 12;

export type ActionEngineOptions = {
  state: ToolState;
  toolId: string;
  allowedTargets?: Set<string> | string[];
  onSubmitToAgent?: (payload: {
    toolId: string;
    eventName: string;
    componentId?: string;
    values: Record<string, unknown>;
  }) => void;
  depth?: number;
};

export type ActionEngineResult = {
  state: ToolState;
  errors: string[];
  changedKeys: string[];
};

function cloneState(state: ToolState): ToolState {
  return structuredClone(state);
}

function isAllowed(
  target: string,
  allowed?: Set<string> | string[],
): boolean {
  if (!allowed) return true;
  if (allowed instanceof Set) return allowed.has(target);
  return allowed.includes(target);
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? [...value] : [];
}

function findItemIndex(
  items: unknown[],
  index?: number,
  id?: string,
): number {
  if (typeof index === "number") return index;
  if (id == null) return -1;
  return items.findIndex((item) => {
    if (item && typeof item === "object" && "id" in item) {
      return String((item as { id: unknown }).id) === id;
    }
    return false;
  });
}

export function applyAction(
  action: ActionDefinition,
  options: ActionEngineOptions,
): ActionEngineResult {
  const depth = options.depth ?? 0;
  const errors: string[] = [];
  const changedKeys: string[] = [];
  const state = cloneState(options.state);

  if (depth > MAX_ACTION_DEPTH) {
    return {
      state,
      errors: ["Action loop protection triggered — too many nested actions"],
      changedKeys,
    };
  }

  const ensureTarget = (target: string): boolean => {
    if (!isAllowed(target, options.allowedTargets)) {
      errors.push(`Action target "${target}" is outside the current tool scope`);
      return false;
    }
    return true;
  };

  switch (action.type) {
    case "setValue": {
      if (!ensureTarget(action.target)) break;
      state[action.target] = action.value;
      changedKeys.push(action.target);
      break;
    }
    case "toggle": {
      if (!ensureTarget(action.target)) break;
      state[action.target] = !state[action.target];
      changedKeys.push(action.target);
      break;
    }
    case "increment": {
      if (!ensureTarget(action.target)) break;
      {
        const amount = action.amount ?? 1;
        const current = Number(state[action.target] ?? 0);
        state[action.target] = (Number.isFinite(current) ? current : 0) + amount;
        changedKeys.push(action.target);
      }
      break;
    }
    case "decrement": {
      if (!ensureTarget(action.target)) break;
      {
        const amount = action.amount ?? 1;
        const current = Number(state[action.target] ?? 0);
        state[action.target] = (Number.isFinite(current) ? current : 0) - amount;
        changedKeys.push(action.target);
      }
      break;
    }
    case "reset": {
      if (!ensureTarget(action.target)) break;
      state[action.target] = action.value ?? 0;
      changedKeys.push(action.target);
      break;
    }
    case "appendItem": {
      if (!ensureTarget(action.target)) break;
      {
        const items = asArray(state[action.target]);
        items.push(action.item);
        state[action.target] = items;
        changedKeys.push(action.target);
      }
      break;
    }
    case "removeItem": {
      if (!ensureTarget(action.target)) break;
      {
        const items = asArray(state[action.target]);
        const idx = findItemIndex(items, action.index, action.id);
        if (idx < 0 || idx >= items.length) {
          errors.push(`Could not remove item from "${action.target}"`);
          break;
        }
        items.splice(idx, 1);
        state[action.target] = items;
        changedKeys.push(action.target);
      }
      break;
    }
    case "updateItem": {
      if (!ensureTarget(action.target)) break;
      {
        const items = asArray(state[action.target]);
        const idx = findItemIndex(items, action.index, action.id);
        if (idx < 0 || idx >= items.length) {
          errors.push(`Could not update item in "${action.target}"`);
          break;
        }
        const current = items[idx];
        if (current && typeof current === "object" && !Array.isArray(current)) {
          items[idx] = { ...(current as Record<string, unknown>), ...action.patch };
        } else {
          items[idx] = action.patch;
        }
        state[action.target] = items;
        changedKeys.push(action.target);
      }
      break;
    }
    case "selectTab": {
      if (!ensureTarget(action.target)) break;
      state[action.target] = action.tabId;
      changedKeys.push(action.target);
      break;
    }
    case "submitToAgent": {
      if (!options.onSubmitToAgent) {
        errors.push("submitToAgent is not available in this context");
        break;
      }
      {
        const values: Record<string, unknown> = {};
        const fields = action.includeFields ?? Object.keys(state);
        for (const field of fields) {
          if (!isAllowed(field, options.allowedTargets)) {
            errors.push(`Field "${field}" is outside the current tool scope`);
            continue;
          }
          values[field] = state[field];
        }
        options.onSubmitToAgent({
          toolId: options.toolId,
          eventName: action.eventName,
          componentId: action.componentId,
          values,
        });
      }
      break;
    }
    default: {
      const _exhaustive: never = action;
      errors.push(`Unsupported action: ${JSON.stringify(_exhaustive)}`);
    }
  }

  return { state, errors, changedKeys };
}

export function applyActions(
  actions: ActionDefinition[],
  options: ActionEngineOptions,
): ActionEngineResult {
  let state = cloneState(options.state);
  const errors: string[] = [];
  const changedKeys: string[] = [];
  let depth = options.depth ?? 0;

  for (const action of actions) {
    depth += 1;
    if (depth > MAX_ACTION_DEPTH) {
      errors.push("Action loop protection triggered — too many actions");
      break;
    }
    const result = applyAction(action, {
      ...options,
      state,
      depth,
    });
    state = result.state;
    errors.push(...result.errors);
    changedKeys.push(...result.changedKeys);
  }

  return { state, errors, changedKeys: [...new Set(changedKeys)] };
}

export { MAX_ACTION_DEPTH };
