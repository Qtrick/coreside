import type { ActionDefinition, ToolComponent, ToolState } from "@/types/tool";

export interface DeclaredBindings {
  readable: Set<string>;
  writable: Set<string>;
  resultTargets: Set<string>;
}

/**
 * Derives explicit declared state and data bindings from component contracts.
 * Component IDs are identities for patching and layout, NEVER automatic permissions to read or mutate state.
 */
export function collectDeclaredBindings(
  components: ToolComponent[],
  into: DeclaredBindings = {
    readable: new Set(),
    writable: new Set(),
    resultTargets: new Set(),
  },
): DeclaredBindings {
  for (const component of components) {
    const type = component.type;
    const valueKey = component.valueKey?.trim();
    const props = component.props ?? {};

    // 1. Inputs / stateful mutable controls:
    if (
      [
        "textInput",
        "textArea",
        "numberInput",
        "select",
        "checkbox",
        "switch",
        "slider",
        "dateInput",
        "timeInput",
        "dateTimeInput",
        "colorInput",
        "codeEditor",
        "counter",
        "quiz",
        "mediaPicker",
      ].includes(type)
    ) {
      const vk =
        valueKey ||
        (typeof props.valueKey === "string" ? props.valueKey.trim() : "") ||
        (typeof props.stateKey === "string" ? props.stateKey.trim() : "");
      if (vk) {
        into.readable.add(vk);
        into.writable.add(vk);
      }
    }

    // 2. Tabs: explicit valueKey binds active tab
    if (type === "tabs") {
      const vk =
        valueKey ||
        (typeof props.valueKey === "string" ? props.valueKey.trim() : "");
      if (vk) {
        into.readable.add(vk);
        into.writable.add(vk);
      }
    }

    // 3. Display / Metric controls: stat, progress
    if (type === "stat" || type === "progress") {
      const vk =
        valueKey ||
        (typeof props.valueKey === "string" ? props.valueKey.trim() : "");
      if (vk) {
        into.readable.add(vk);
      }
    }

    // 4. Data collection controls: dataTable, table, list, checklist
    if (["dataTable", "table", "list", "checklist"].includes(type)) {
      const dataKeys = [
        valueKey,
        typeof props.rowsKey === "string" ? props.rowsKey.trim() : "",
        typeof props.dataKey === "string" ? props.dataKey.trim() : "",
        typeof props.valueKey === "string" ? props.valueKey.trim() : "",
      ].filter((k): k is string => Boolean(k));
      for (const dk of dataKeys) {
        into.readable.add(dk);
        into.writable.add(dk);
      }
      const selKey =
        (typeof props.selectionKey === "string" ? props.selectionKey.trim() : "") ||
        ((component as unknown as { selectionKey?: string }).selectionKey?.trim() ?? "");
      if (selKey) {
        into.readable.add(selKey);
        into.writable.add(selKey);
      }
    }

    // 5. Chart components
    if (type.startsWith("chart")) {
      const chartKeys = [
        valueKey,
        typeof props.dataKey === "string" ? props.dataKey.trim() : "",
        typeof props.valueKey === "string" ? props.valueKey.trim() : "",
      ].filter((k): k is string => Boolean(k));
      for (const ck of chartKeys) {
        into.readable.add(ck);
      }
    }

    // 6. Registered actions: resultKey is an output target, NOT a read authorization
    if (component.actions) {
      for (const action of component.actions) {
        if (
          "resultKey" in action &&
          typeof action.resultKey === "string" &&
          action.resultKey.trim()
        ) {
          if (
            action.type === "invokeRegisteredAction" &&
            action.actionName &&
            !isStateBindableAction(action.actionName)
          ) {
            continue;
          }
          const rk = action.resultKey.trim();
          into.resultTargets.add(rk);
          into.writable.add(rk);
        }
      }
    }

    if (component.children) {
      collectDeclaredBindings(component.children, into);
    }
  }

  return into;
}

export function collectTargets(
  components: ToolComponent[],
  into = new Set<string>(),
): Set<string> {
  const bindings = collectDeclaredBindings(components);
  for (const k of bindings.readable) into.add(k);
  for (const k of bindings.writable) into.add(k);
  for (const k of bindings.resultTargets) into.add(k);
  return into;
}

const MAX_ACTION_DEPTH = 12;

export const NON_STATE_BINDABLE_ACTIONS = new Set<string>([
  "external_link.open",
  "export.prepare",
  "web_search.request",
  "automation.propose",
  "agent.submit_event",
]);

export function isStateBindableAction(actionName: string): boolean {
  return !NON_STATE_BINDABLE_ACTIONS.has(actionName);
}

export type RegisteredActionOutcome =
  | { status: "ok"; data?: unknown; stateBindable?: boolean }
  | { status: "pendingApproval"; approvalId?: string; reason?: string }
  | { status: "error"; message: string }
  | { status: "blocked"; reason: string }
  | void;

export type ActionEngineOptions = {
  state: ToolState;
  toolId: string;
  bindings?: DeclaredBindings;
  allowedTargets?: Set<string> | string[];
  onSubmitToAgent?: (payload: {
    toolId: string;
    eventName: string;
    componentId?: string;
    values: Record<string, unknown>;
  }) => void;
  onInvokeRegisteredAction?: (payload: {
    toolId: string;
    actionName: string;
    input: Record<string, unknown>;
    componentId?: string;
    resultKey?: string;
  }) => RegisteredActionOutcome | Promise<RegisteredActionOutcome>;
  depth?: number;
};

export type ActionEngineResult = {
  state: ToolState;
  errors: string[];
  changedKeys: string[];
  pendingTasks: Promise<void>[];
  pendingApproval?: boolean;
};

function cloneState(state: ToolState): ToolState {
  return structuredClone(state);
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

function checkTargetWritable(
  target: string,
  options: ActionEngineOptions,
): boolean {
  if (options.bindings) {
    return options.bindings.writable.has(target);
  }
  if (options.allowedTargets) {
    if (options.allowedTargets instanceof Set) return options.allowedTargets.has(target);
    return options.allowedTargets.includes(target);
  }
  // Fail closed: without declared bindings or allowed targets, state mutation is unauthorized.
  return false;
}

function checkTargetReadable(
  sourceKey: string,
  options: ActionEngineOptions,
): boolean {
  if (options.bindings) {
    return options.bindings.readable.has(sourceKey);
  }
  if (options.allowedTargets) {
    if (options.allowedTargets instanceof Set) return options.allowedTargets.has(sourceKey);
    return options.allowedTargets.includes(sourceKey);
  }
  // Fail closed: without declared bindings or allowed targets, state reading is unauthorized.
  return false;
}

export function applyAction(
  action: ActionDefinition,
  options: ActionEngineOptions,
): ActionEngineResult {
  const depth = options.depth ?? 0;
  const errors: string[] = [];
  const changedKeys: string[] = [];
  const pendingTasks: Promise<void>[] = [];
  const state = cloneState(options.state);

  if (depth > MAX_ACTION_DEPTH) {
    return {
      state,
      errors: ["Action loop protection triggered — too many nested actions"],
      changedKeys,
      pendingTasks,
    };
  }

  const ensureWritable = (target: string): boolean => {
    if (!checkTargetWritable(target, options)) {
      errors.push(`Action target "${target}" is outside the current tool scope`);
      return false;
    }
    return true;
  };

  const ensureReadable = (sourceKey: string): boolean => {
    if (!checkTargetReadable(sourceKey, options)) {
      errors.push(`Field "${sourceKey}" is outside the current tool scope`);
      return false;
    }
    return true;
  };

  switch (action.type) {
    case "setValue": {
      if (!ensureWritable(action.target)) break;
      state[action.target] = action.value;
      changedKeys.push(action.target);
      break;
    }
    case "toggle": {
      if (!ensureWritable(action.target)) break;
      state[action.target] = !state[action.target];
      changedKeys.push(action.target);
      break;
    }
    case "increment": {
      if (!ensureWritable(action.target)) break;
      {
        const amount = action.amount ?? 1;
        const current = Number(state[action.target] ?? 0);
        state[action.target] = (Number.isFinite(current) ? current : 0) + amount;
        changedKeys.push(action.target);
      }
      break;
    }
    case "decrement": {
      if (!ensureWritable(action.target)) break;
      {
        const amount = action.amount ?? 1;
        const current = Number(state[action.target] ?? 0);
        state[action.target] = (Number.isFinite(current) ? current : 0) - amount;
        changedKeys.push(action.target);
      }
      break;
    }
    case "reset": {
      if (!ensureWritable(action.target)) break;
      state[action.target] = action.value ?? 0;
      changedKeys.push(action.target);
      break;
    }
    case "appendItem": {
      if (!ensureWritable(action.target)) break;
      {
        const items = asArray(state[action.target]);
        items.push(action.item);
        state[action.target] = items;
        changedKeys.push(action.target);
      }
      break;
    }
    case "removeItem": {
      if (!ensureWritable(action.target)) break;
      {
        const items = asArray(state[action.target]);
        const idx = findItemIndex(items, action.index, action.id);
        if (idx >= 0 && idx < items.length) {
          items.splice(idx, 1);
          state[action.target] = items;
          changedKeys.push(action.target);
        }
      }
      break;
    }
    case "updateItem": {
      if (!ensureWritable(action.target)) break;
      {
        const items = asArray(state[action.target]);
        const idx = findItemIndex(items, action.index, action.id);
        if (idx >= 0 && idx < items.length) {
          const current = items[idx];
          if (current && typeof current === "object" && typeof action.patch === "object") {
            items[idx] = { ...current, ...action.patch };
            state[action.target] = items;
            changedKeys.push(action.target);
          }
        }
      }
      break;
    }
    case "selectTab": {
      if (!ensureWritable(action.target)) break;
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
        if (!action.includeFields || action.includeFields.length === 0) {
          errors.push("submitToAgent requires explicit includeFields declaration");
          break;
        }
        const values: Record<string, unknown> = {};
        let hasUnauthorized = false;
        for (const field of action.includeFields) {
          if (!ensureReadable(field)) {
            hasUnauthorized = true;
          } else {
            values[field] = state[field];
          }
        }
        if (hasUnauthorized) {
          break;
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
    case "invokeRegisteredAction": {
      if (!options.onInvokeRegisteredAction) {
        errors.push("invokeRegisteredAction is not available in this context");
        break;
      }
      {
        if (!action.actionName || !/^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$/.test(action.actionName)) {
          errors.push(`Invalid registered action name: "${action.actionName ?? ""}"`);
          break;
        }
        const input: Record<string, unknown> = { ...(action.input ?? {}) };
        let hasUnauthorized = false;
        for (const [key, stateKey] of Object.entries(action.inputFromState ?? {})) {
          if (!ensureReadable(stateKey)) {
            hasUnauthorized = true;
          } else {
            input[key] = state[stateKey];
          }
        }
        if (hasUnauthorized) {
          break;
        }
        if (action.resultKey) {
          if (!isStateBindableAction(action.actionName)) {
            errors.push(`Action "${action.actionName}" does not allow binding outputs to state`);
            break;
          }
          if (!ensureWritable(action.resultKey)) {
            errors.push(`Result target "${action.resultKey}" is outside the current tool scope`);
            break;
          }
        }
        const task = options.onInvokeRegisteredAction({
          toolId: options.toolId,
          actionName: action.actionName,
          input,
          componentId: action.componentId,
          resultKey: action.resultKey,
        });
        if (task instanceof Promise) {
          pendingTasks.push(
            task.then((outcome) => {
              if (outcome && typeof outcome === "object" && outcome.status === "ok" && action.resultKey) {
                if (!outcome.stateBindable || !isStateBindableAction(action.actionName)) {
                  errors.push(`Action "${action.actionName}" is not state-bindable`);
                  return;
                }
                state[action.resultKey] = outcome.data;
                changedKeys.push(action.resultKey);
              }
            }),
          );
        }
      }
      break;
    }
    default: {
      const _exhaustive: never = action;
      errors.push(`Unsupported action: ${JSON.stringify(_exhaustive)}`);
    }
  }

  return { state, errors, changedKeys, pendingTasks };
}

export function applyActions(
  actions: ActionDefinition[],
  options: ActionEngineOptions,
): ActionEngineResult {
  let state = cloneState(options.state);
  const errors: string[] = [];
  const changedKeys: string[] = [];
  const pendingTasks: Promise<void>[] = [];
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
    pendingTasks.push(...result.pendingTasks);
  }

  return {
    state,
    errors,
    changedKeys: [...new Set(changedKeys)],
    pendingTasks,
  };
}

/**
 * Deterministic sequential async action execution pipeline.
 * Each action awaits prior async actions, merges state, and gives subsequent actions the updated state.
 */
export async function applyActionsAsync(
  actions: ActionDefinition[],
  options: ActionEngineOptions,
): Promise<ActionEngineResult> {
  let state = cloneState(options.state);
  const errors: string[] = [];
  const changedKeys: string[] = [];
  let depth = options.depth ?? 0;
  let pendingApproval = false;

  for (const action of actions) {
    depth += 1;
    if (depth > MAX_ACTION_DEPTH) {
      errors.push("Action loop protection triggered — too many actions");
      break;
    }

    if (action.type === "invokeRegisteredAction") {
      if (!options.onInvokeRegisteredAction) {
        errors.push("invokeRegisteredAction is not available in this context");
        break;
      }
      if (!action.actionName || !/^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$/.test(action.actionName)) {
        errors.push(`Invalid registered action name: "${action.actionName ?? ""}"`);
        break;
      }
      let hasUnauthorized = false;
      const input: Record<string, unknown> = { ...(action.input ?? {}) };
      for (const [key, stateKey] of Object.entries(action.inputFromState ?? {})) {
        if (!checkTargetReadable(stateKey, options)) {
          errors.push(`Field "${stateKey}" is outside the current tool scope`);
          hasUnauthorized = true;
        } else {
          input[key] = state[stateKey];
        }
      }
      if (hasUnauthorized) break;
      if (action.resultKey) {
        if (!isStateBindableAction(action.actionName)) {
          errors.push(`Action "${action.actionName}" does not allow binding outputs to state`);
          break;
        }
        if (!checkTargetWritable(action.resultKey, options)) {
          errors.push(`Result target "${action.resultKey}" is outside the current tool scope`);
          break;
        }
      }

      try {
        const outcome = await options.onInvokeRegisteredAction({
          toolId: options.toolId,
          actionName: action.actionName,
          input,
          componentId: action.componentId,
          resultKey: action.resultKey,
        });

        if (outcome && typeof outcome === "object") {
          if (outcome.status === "ok") {
            if (action.resultKey) {
              if (!outcome.stateBindable || !isStateBindableAction(action.actionName)) {
                errors.push(`Action "${action.actionName}" is not state-bindable`);
                break;
              }
              // Merge result into current state instead of overwriting.
              // The action result updates only the resultKey; all other
              // state (including user edits made during the async gap)
              // is preserved.
              if (
                outcome.data &&
                typeof outcome.data === "object" &&
                !Array.isArray(outcome.data) &&
                state[action.resultKey] &&
                typeof state[action.resultKey] === "object" &&
                !Array.isArray(state[action.resultKey])
              ) {
                state[action.resultKey] = {
                  ...(state[action.resultKey] as Record<string, unknown>),
                  ...(outcome.data as Record<string, unknown>),
                };
              } else {
                state[action.resultKey] = outcome.data;
              }
              changedKeys.push(action.resultKey);
            }
          } else if (outcome.status === "pendingApproval") {
            pendingApproval = true;
            break;
          } else if (outcome.status === "error") {
            errors.push(outcome.message || "Action failed");
            break;
          } else if (outcome.status === "blocked") {
            errors.push(outcome.reason || "Action blocked");
            break;
          }
        }
      } catch (err) {
        errors.push(err instanceof Error ? err.message : String(err));
        break;
      }
      continue;
    }

    const result = applyAction(action, {
      ...options,
      state,
      depth,
    });
    state = result.state;
    errors.push(...result.errors);
    changedKeys.push(...result.changedKeys);
    if (result.errors.length > 0) {
      break;
    }
  }

  return {
    state,
    errors,
    changedKeys: [...new Set(changedKeys)],
    pendingTasks: [],
    pendingApproval,
  };
}

export { MAX_ACTION_DEPTH };
