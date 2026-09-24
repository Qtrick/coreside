import { describe, expect, it, vi } from "vitest";
import {
  collectDeclaredBindings,
  applyActionsAsync,
} from "@/lib/actions";
import type { ActionDefinition } from "@/types/tool";
import { FIXTURE_TOOLS } from "@/lib/benchmarks/fixture-tools";

describe("CRUD Task Manager Acceptance & Authority Suite", () => {
  const tool = FIXTURE_TOOLS.crud_task_manager;

  it("extracts all declared input, selection, and resultKey bindings", () => {
    const bindings = collectDeclaredBindings(tool.components);

    // Inputs: newTaskTitle, newTaskPriority
    expect(bindings.readable.has("newTaskTitle")).toBe(true);
    expect(bindings.writable.has("newTaskTitle")).toBe(true);
    expect(bindings.readable.has("newTaskPriority")).toBe(true);
    expect(bindings.writable.has("newTaskPriority")).toBe(true);

    // DataTable: tasks, selectedTaskId
    expect(bindings.readable.has("tasks")).toBe(true);
    expect(bindings.writable.has("tasks")).toBe(true);
    expect(bindings.readable.has("selectedTaskId")).toBe(true);
    expect(bindings.writable.has("selectedTaskId")).toBe(true);

    // Actions output target: lastTask
    expect(bindings.resultTargets.has("lastTask")).toBe(true);
    expect(bindings.writable.has("lastTask")).toBe(true);

    // Unauthorized keys must not be present
    expect(bindings.readable.has("secretApiKey")).toBe(false);
    expect(bindings.writable.has("unauthorizedKey")).toBe(false);
  });

  it("executes create task registered action with inputFromState and updates resultKey", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const initialState = {
      newTaskTitle: "Implement security boundary test",
      newTaskPriority: "High",
      tasks: [
        { id: "TSK-1", title: "Existing task", priority: "Low", status: "Open" },
      ],
    };

    const mockInvoke = vi.fn().mockImplementation(async (payload) => {
      if (payload.actionName === "local_data.write") {
        return {
          status: "ok",
          data: {
            id: "TSK-2",
            title: payload.input.title,
            priority: payload.input.priority,
            status: "Open",
          },
          stateBindable: true,
        };
      }
      if (payload.actionName === "local_data.query") {
        return {
          status: "ok",
          data: [
            { id: "TSK-1", title: "Existing task", priority: "Low", status: "Open" },
            { id: "TSK-2", title: "Implement security boundary test", priority: "High", status: "Open" },
          ],
          stateBindable: true,
        };
      }
      return { status: "ok", data: {}, stateBindable: true };
    });

    const btn = tool.components.find((c) => c.id === "btn-add-task");
    expect(btn).toBeDefined();
    const actions = (btn?.actions ?? []) as ActionDefinition[];

    const result = await applyActionsAsync(actions, {
      state: initialState,
      toolId: tool.id,
      bindings,
      onInvokeRegisteredAction: mockInvoke,
    });

    expect(result.errors).toHaveLength(0);
    expect(mockInvoke).toHaveBeenCalledTimes(2);

    // Verify first call passed inputs from state correctly
    expect(mockInvoke.mock.calls[0][0]).toMatchObject({
      actionName: "local_data.write",
      input: {
        model: "tasks",
        title: "Implement security boundary test",
        priority: "High",
      },
    });

    // Verify state was updated with resultKey
    expect(result.changedKeys).toContain("lastTask");
    expect(result.changedKeys).toContain("tasks");
    expect(result.state.lastTask).toMatchObject({
      id: "TSK-2",
      title: "Implement security boundary test",
    });
    expect(result.state.tasks).toHaveLength(2);
  });

  it("executes delete registered action using selected row ID", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const initialState = {
      selectedTaskId: "TSK-101",
      tasks: [
        { id: "TSK-101", title: "Task to delete", priority: "High", status: "Done" },
      ],
    };

    const mockInvoke = vi.fn().mockImplementation(async (payload) => {
      if (payload.actionName === "local_data.delete") {
        return {
          status: "ok",
          data: { deletedId: payload.input.id },
          stateBindable: true,
        };
      }
      if (payload.actionName === "local_data.query") {
        return {
          status: "ok",
          data: [],
          stateBindable: true,
        };
      }
      return { status: "ok", data: {}, stateBindable: true };
    });

    const btn = tool.components.find((c) => c.id === "btn-delete-task");
    expect(btn).toBeDefined();
    const actions = (btn?.actions ?? []) as ActionDefinition[];

    const result = await applyActionsAsync(actions, {
      state: initialState,
      toolId: tool.id,
      bindings,
      onInvokeRegisteredAction: mockInvoke,
    });

    expect(result.errors).toHaveLength(0);
    expect(mockInvoke).toHaveBeenCalledTimes(2);
    expect(mockInvoke.mock.calls[0][0]).toMatchObject({
      actionName: "local_data.delete",
      input: {
        model: "tasks",
        id: "TSK-101",
      },
    });
    expect(result.state.tasks).toEqual([]);
  });

  it("fails closed when an adversarial action requests undeclared state keys", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const initialState = {
      secretCredentialKey: "super-secret-token",
      newTaskTitle: "Valid task",
    };

    const mockInvoke = vi.fn();
    const adversarialAction: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.write",
      input: { model: "tasks" },
      inputFromState: {
        stolen: "secretCredentialKey", // Undeclared target!
      },
    };

    const result = await applyActionsAsync([adversarialAction], {
      state: initialState,
      toolId: tool.id,
      bindings,
      onInvokeRegisteredAction: mockInvoke,
    });

    expect(result.errors.length).toBeGreaterThan(0);
    expect(result.errors[0]).toContain("outside the current tool scope");
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("fails closed when an adversarial action attempts to write to undeclared state via resultKey", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const initialState = {
      userRole: "viewer",
    };

    const mockInvoke = vi.fn().mockResolvedValue({
      status: "ok",
      data: { role: "admin" },
    });

    const adversarialAction: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.query",
      input: { model: "tasks" },
      resultKey: "userRole", // Not in declared bindings!
    };

    const result = await applyActionsAsync([adversarialAction], {
      state: initialState,
      toolId: tool.id,
      bindings,
      onInvokeRegisteredAction: mockInvoke,
    });

    expect(result.errors.length).toBeGreaterThan(0);
    expect(result.errors[0]).toContain("outside the current tool scope");
    expect(result.state.userRole).toBe("viewer");
  });

  it("safely maps nested dotted paths in inputFromState without prototype pollution", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const state = {
      selectedTaskId: "TSK-42",
      newTaskTitle: "Deep nested task",
      newTaskPriority: "Urgent",
    };

    const mockInvoke = vi.fn().mockResolvedValue({
      status: "ok",
      data: { success: true },
      stateBindable: true,
    });

    const updateAction: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.write",
      input: {
        modelId: "tasks",
        data: {},
      },
      inputFromState: {
        recordId: "selectedTaskId",
        "data.title": "newTaskTitle",
        "data.priority": "newTaskPriority",
      },
      resultKey: "lastTask",
    };

    const result = await applyActionsAsync([updateAction], {
      state,
      toolId: tool.id,
      bindings,
      onInvokeRegisteredAction: mockInvoke,
    });

    expect(result.errors).toHaveLength(0);
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke.mock.calls[0][0].input).toEqual({
      modelId: "tasks",
      recordId: "TSK-42",
      data: {
        title: "Deep nested task",
        priority: "Urgent",
      },
    });
  });

  it("rejects prototype pollution attempts in inputFromState dotted paths", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const state = {
      newTaskTitle: "Evil payload",
    };

    const mockInvoke = vi.fn();
    const maliciousAction: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.write",
      input: { modelId: "tasks" },
      inputFromState: {
        "__proto__.polluted": "newTaskTitle",
      },
    };

    const result = await applyActionsAsync([maliciousAction], {
      state,
      toolId: tool.id,
      bindings,
      onInvokeRegisteredAction: mockInvoke,
    });

    expect(result.errors.length).toBeGreaterThan(0);
    expect(result.errors[0]).toContain("Dangerous path segment");
    expect(mockInvoke).not.toHaveBeenCalled();
    expect((Object.prototype as Record<string, unknown>).polluted).toBeUndefined();
  });

  it("handles state-based CRUD items (itemFromState, idFromState, patchFromState)", async () => {
    const bindings = collectDeclaredBindings(tool.components);
    const state = {
      selectedTaskId: "TSK-1",
      newTaskTitle: "State-based item",
      tasks: [{ id: "TSK-1", title: "Old task" }],
    };

    // Test appendItem with itemFromState
    const appendAction: ActionDefinition = {
      type: "appendItem",
      target: "tasks",
      itemFromState: {
        title: "newTaskTitle",
      },
    };

    const appendResult = await applyActionsAsync([appendAction], {
      state,
      toolId: tool.id,
      bindings,
    });

    expect(appendResult.errors).toHaveLength(0);
    expect(appendResult.state.tasks).toHaveLength(2);
    expect((appendResult.state.tasks as Array<Record<string, unknown>>)[1]).toMatchObject({
      title: "State-based item",
    });

    // Test updateItem with idFromState and patchFromState
    const updateAction: ActionDefinition = {
      type: "updateItem",
      target: "tasks",
      idFromState: "selectedTaskId",
      patchFromState: {
        title: "newTaskTitle",
      },
    };

    const updateResult = await applyActionsAsync([updateAction], {
      state: appendResult.state,
      toolId: tool.id,
      bindings,
    });

    expect(updateResult.errors).toHaveLength(0);
    expect((updateResult.state.tasks as Array<Record<string, unknown>>)[0]).toMatchObject({
      id: "TSK-1",
      title: "State-based item",
    });

    // Test removeItem with idFromState
    const removeAction: ActionDefinition = {
      type: "removeItem",
      target: "tasks",
      idFromState: "selectedTaskId",
    };

    const removeResult = await applyActionsAsync([removeAction], {
      state: updateResult.state,
      toolId: tool.id,
      bindings,
    });

    expect(removeResult.errors).toHaveLength(0);
    expect(removeResult.state.tasks).toHaveLength(1);
  });
});
