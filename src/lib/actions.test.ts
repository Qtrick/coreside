import { describe, expect, it, vi } from "vitest";
import { applyAction, applyActions } from "@/lib/actions";
import type { ActionDefinition } from "@/types/tool";

describe("invokeRegisteredAction", () => {
  it("resolves inputFromState from allowed targets and calls callback", () => {
    const onInvokeRegisteredAction = vi.fn();
    const action: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.write",
      input: { table: "notes" },
      inputFromState: { body: "draft" },
      componentId: "save-btn",
    };

    const result = applyAction(action, {
      state: { draft: "Hello world", other: "ignored" },
      toolId: "tool-1",
      allowedTargets: new Set(["draft"]),
      onInvokeRegisteredAction,
    });

    expect(onInvokeRegisteredAction).toHaveBeenCalledWith({
      toolId: "tool-1",
      actionName: "local_data.write",
      input: { table: "notes", body: "Hello world" },
      componentId: "save-btn",
    });
    expect(result.errors).toEqual([]);
    expect(result.changedKeys).toEqual([]);
  });

  it("blocks invokeRegisteredAction fail-closed if any inputFromState field is outside allowed targets", () => {
    const onInvokeRegisteredAction = vi.fn();
    const result = applyAction(
      {
        type: "invokeRegisteredAction",
        actionName: "local_data.write",
        input: { publicParam: "ok" },
        inputFromState: { secret: "hidden", allowed: "draft" },
      },
      {
        state: { hidden: "sensitive_value", draft: "draft_value" },
        toolId: "tool-1",
        allowedTargets: new Set(["draft"]),
        onInvokeRegisteredAction,
      },
    );

    // Fail closed: zero privileged calls! No partial execution!
    expect(onInvokeRegisteredAction).not.toHaveBeenCalled();
    expect(result.errors).toContain(
      'Field "hidden" is outside the current tool scope',
    );
    expect(result.changedKeys).toEqual([]);
  });

  it("blocks submitToAgent fail-closed if any requested field is outside allowed targets", () => {
    const onSubmitToAgent = vi.fn();
    const result = applyAction(
      {
        type: "submitToAgent",
        eventName: "form_submit",
        includeFields: ["allowedField", "secretField"],
      },
      {
        state: { allowedField: "hello", secretField: "secret_token" },
        toolId: "tool-1",
        allowedTargets: new Set(["allowedField"]),
        onSubmitToAgent,
      },
    );

    // Fail closed: zero submissions to agent!
    expect(onSubmitToAgent).not.toHaveBeenCalled();
    expect(result.errors).toContain(
      'Field "secretField" is outside the current tool scope',
    );
  });

  it("keeps local actions working", () => {
    const result = applyActions(
      [
        { type: "increment", target: "count", amount: 2 },
        { type: "setValue", target: "label", value: "Done" },
      ],
      {
        state: { count: 1 },
        toolId: "tool-1",
        allowedTargets: new Set(["count", "label"]),
      },
    );

    expect(result.state).toEqual({ count: 3, label: "Done" });
    expect(result.errors).toEqual([]);
  });

  it("does not expose a generic invoke path", () => {
    const unsupported = {
      type: "customInvoke",
      command: "kernel_invoke_registered_action",
    } as unknown as ActionDefinition;

    const result = applyAction(unsupported, {
      state: {},
      toolId: "tool-1",
    });

    expect(result.errors[0]).toContain("Unsupported action");
  });

  it("passes resultKey to onInvokeRegisteredAction when target is in allowed targets", () => {
    const onInvokeRegisteredAction = vi.fn();
    const action: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.query",
      input: { modelId: "habits" },
      resultKey: "habitRecords",
    };

    const result = applyAction(action, {
      state: {},
      toolId: "tool-1",
      allowedTargets: new Set(["habitRecords"]),
      onInvokeRegisteredAction,
    });

    expect(onInvokeRegisteredAction).toHaveBeenCalledWith({
      toolId: "tool-1",
      actionName: "local_data.query",
      input: { modelId: "habits" },
      componentId: undefined,
      resultKey: "habitRecords",
    });
    expect(result.errors).toEqual([]);
  });

  it("blocks invokeRegisteredAction if resultKey is outside allowed targets", () => {
    const onInvokeRegisteredAction = vi.fn();
    const action: ActionDefinition = {
      type: "invokeRegisteredAction",
      actionName: "local_data.query",
      input: { modelId: "habits" },
      resultKey: "unauthorizedTarget",
    };

    const result = applyAction(action, {
      state: {},
      toolId: "tool-1",
      allowedTargets: new Set(["otherField"]),
      onInvokeRegisteredAction,
    });

    expect(onInvokeRegisteredAction).not.toHaveBeenCalled();
    expect(result.errors).toContain(
      'Result target "unauthorizedTarget" is outside the current tool scope',
    );
  });
});

describe("collectTargets security boundary", () => {
  it("derives targets strictly from declared component bindings without self-authorizing inputFromState", async () => {
    const { collectTargets } = await import("@/lib/actions");

    const components = [
      {
        id: "input-name",
        type: "textInput",
        valueKey: "userName",
      },
      {
        id: "table-results",
        type: "dataTable",
        props: {
          rowsKey: "queriedRecords",
        },
      },
      {
        id: "save-button",
        type: "button",
        actions: [
          {
            type: "invokeRegisteredAction" as const,
            actionName: "malicious.steal",
            inputFromState: {
              secretToken: "superSecretApiKey",
            },
            resultKey: "queriedRecords",
          },
        ],
      },
    ];

    const targets = collectTargets(components);

    // CRITICAL P0: Component IDs are NOT automatically authorized state bindings:
    expect(targets.has("input-name")).toBe(false);

    // Declared component bindings are allowed:
    expect(targets.has("userName")).toBe(true);
    expect(targets.has("queriedRecords")).toBe(true);

    // CRITICAL P0: inputFromState keys MUST NOT be authorized:
    expect(targets.has("superSecretApiKey")).toBe(false);
    expect(targets.has("secretToken")).toBe(false);
  });

  it("fails closed when an action attempts self-authorization via resultKey and inputFromState", async () => {
    const { collectDeclaredBindings, applyAction } = await import("@/lib/actions");

    // Attack payload: Component has no state binding. Action sets resultKey to a secret
    // and attempts to read it via inputFromState.
    const components = [
      {
        id: "attack-btn",
        type: "button",
        actions: [
          {
            type: "invokeRegisteredAction" as const,
            actionName: "steal.token",
            resultKey: "victimSecretKey",
            inputFromState: { token: "victimSecretKey" },
          },
        ],
      },
    ];

    const bindings = collectDeclaredBindings(components);
    // victimSecretKey is only a resultTarget, NOT readable:
    expect(bindings.readable.has("victimSecretKey")).toBe(false);
    expect(bindings.resultTargets.has("victimSecretKey")).toBe(true);

    const onInvokeRegisteredAction = vi.fn().mockResolvedValue({ status: "ok", data: {} });
    const result = applyAction(components[0].actions![0], {
      state: { victimSecretKey: "classified-vault-token" },
      toolId: "test-tool",
      bindings,
      onInvokeRegisteredAction,
    });

    // Must fail closed:
    expect(onInvokeRegisteredAction).not.toHaveBeenCalled();
    expect(result.errors).toContain('Field "victimSecretKey" is outside the current tool scope');
  });

  it("executes registered actions sequentially and deterministically merges results", async () => {
    const { applyActionsAsync, collectDeclaredBindings } = await import("@/lib/actions");

    const components = [
      {
        id: "task-table",
        type: "dataTable",
        props: { rowsKey: "tasks" },
      },
      {
        id: "status-field",
        type: "textInput",
        valueKey: "lastStatus",
      },
    ];

    const bindings = collectDeclaredBindings(components);

    const onInvokeRegisteredAction = vi.fn().mockImplementation(async (params) => {
      if (params.actionName === "local_data.write") {
        return { status: "ok", data: { recordId: "rec-1", created: true } };
      }
      if (params.actionName === "local_data.query") {
        return {
          status: "ok",
          data: {
            records: [{ _id: "rec-1", title: "Complete audit" }],
            count: 1,
          },
        };
      }
      return { status: "ok", data: null };
    });

    const actions = [
      {
        type: "invokeRegisteredAction" as const,
        actionName: "local_data.write",
        input: { modelId: "tasks", data: { title: "Complete audit" } },
        resultKey: "lastStatus",
      },
      {
        type: "invokeRegisteredAction" as const,
        actionName: "local_data.query",
        input: { modelId: "tasks" },
        resultKey: "tasks",
      },
    ];

    const result = await applyActionsAsync(actions, {
      state: { tasks: [], lastStatus: "" },
      toolId: "tasks-app",
      bindings,
      onInvokeRegisteredAction,
    });

    expect(result.errors).toEqual([]);
    expect(onInvokeRegisteredAction).toHaveBeenCalledTimes(2);
    expect(result.state.lastStatus).toEqual({ recordId: "rec-1", created: true });
    expect(result.state.tasks).toEqual({
      records: [{ _id: "rec-1", title: "Complete audit" }],
      count: 1,
    });
  });

  it("blocks submitToAgent fail-closed if includeFields is empty or missing", async () => {
    const { applyAction, collectDeclaredBindings } = await import("@/lib/actions");

    const components = [
      {
        id: "field-1",
        type: "textInput",
        valueKey: "username",
      },
    ];
    const bindings = collectDeclaredBindings(components);
    const onSubmitToAgent = vi.fn();

    // 1. Missing includeFields:
    const resultMissing = applyAction(
      {
        type: "submitToAgent",
        eventName: "submit_all",
      },
      {
        state: { username: "alice", secret: "12345" },
        toolId: "test-tool",
        bindings,
        onSubmitToAgent,
      },
    );
    expect(onSubmitToAgent).not.toHaveBeenCalled();
    expect(resultMissing.errors).toContain(
      "submitToAgent requires explicit includeFields declaration",
    );

    // 2. Empty includeFields array:
    const resultEmpty = applyAction(
      {
        type: "submitToAgent",
        eventName: "submit_empty",
        includeFields: [],
      },
      {
        state: { username: "alice" },
        toolId: "test-tool",
        bindings,
        onSubmitToAgent,
      },
    );
    expect(onSubmitToAgent).not.toHaveBeenCalled();
    expect(resultEmpty.errors).toContain(
      "submitToAgent requires explicit includeFields declaration",
    );
  });

  it("proves component ID, arbitrary props.target, props.field, props.name do not authorize state access", async () => {
    const { collectDeclaredBindings, applyAction } = await import("@/lib/actions");

    // Component crafted with inert props containing sensitive state key names
    const components = [
      {
        id: "victimKey1", // ID matching secret
        type: "card",
        props: {
          target: "victimKey2",
          field: "victimKey3",
          name: "victimKey4",
        },
      },
    ];

    const bindings = collectDeclaredBindings(components);
    // None of these should be readable or writable:
    expect(bindings.readable.has("victimKey1")).toBe(false);
    expect(bindings.readable.has("victimKey2")).toBe(false);
    expect(bindings.readable.has("victimKey3")).toBe(false);
    expect(bindings.readable.has("victimKey4")).toBe(false);

    const onInvokeRegisteredAction = vi.fn();
    const result = applyAction(
      {
        type: "invokeRegisteredAction",
        actionName: "exfiltrate",
        inputFromState: {
          k1: "victimKey1",
          k2: "victimKey2",
          k3: "victimKey3",
          k4: "victimKey4",
        },
      },
      {
        state: {
          victimKey1: "sec1",
          victimKey2: "sec2",
          victimKey3: "sec3",
          victimKey4: "sec4",
        },
        toolId: "test-tool",
        bindings,
        onInvokeRegisteredAction,
      },
    );

    expect(onInvokeRegisteredAction).not.toHaveBeenCalled();
    expect(result.errors.length).toBeGreaterThan(0);
  });

  it("executes complete CRUD flow: write record -> query records -> select -> delete -> query", async () => {
    const { applyActionsAsync, collectDeclaredBindings } = await import("@/lib/actions");

    const components = [
      { id: "task-title", type: "textInput", valueKey: "newTaskTitle" },
      { id: "task-priority", type: "select", valueKey: "newTaskPriority" },
      { id: "task-table", type: "dataTable", valueKey: "tasks", props: { selectionKey: "selectedTaskId" } },
      {
        id: "add-task-btn",
        type: "button",
        actions: [
          {
            type: "invokeRegisteredAction" as const,
            actionName: "local_data.write",
            resultKey: "lastTask",
          },
          {
            type: "invokeRegisteredAction" as const,
            actionName: "local_data.query",
            resultKey: "tasks",
          },
        ],
      },
    ];
    const bindings = collectDeclaredBindings(components);

    let dbRecords: Array<{ id: string; title: string; priority: string }> = [];

    const onInvokeRegisteredAction = vi.fn().mockImplementation(async (params) => {
      if (params.actionName === "local_data.write") {
        const record = {
          id: `task-${Date.now()}`,
          title: params.input.title,
          priority: params.input.priority,
        };
        dbRecords.push(record);
        return { status: "ok", data: record };
      }
      if (params.actionName === "local_data.query") {
        return {
          status: "ok",
          data: {
            records: [...dbRecords],
            count: dbRecords.length,
          },
        };
      }
      if (params.actionName === "local_data.delete") {
        dbRecords = dbRecords.filter((r) => r.id !== params.input.id);
        return { status: "ok", data: { deleted: true } };
      }
      return { status: "ok", data: null };
    });

    // Step 1: Create a record and refresh query
    const createStep = await applyActionsAsync(
      [
        {
          type: "invokeRegisteredAction",
          actionName: "local_data.write",
          input: { model: "tasks" },
          inputFromState: { title: "newTaskTitle", priority: "newTaskPriority" },
          resultKey: "lastTask",
        },
        {
          type: "invokeRegisteredAction",
          actionName: "local_data.query",
          input: { model: "tasks" },
          resultKey: "tasks",
        },
      ],
      {
        state: {
          newTaskTitle: "Deploy kernel v2",
          newTaskPriority: "High",
          tasks: { records: [], count: 0 },
        },
        toolId: "crud-app",
        bindings,
        onInvokeRegisteredAction,
      },
    );

    expect(createStep.errors).toEqual([]);
    const createdTasks = createStep.state.tasks as { records: Array<{ id: string }>; count: number };
    expect(createdTasks.count).toBe(1);
    const createdId = createdTasks.records[0].id;

    // Step 2: Delete the created record and refresh query
    const deleteStep = await applyActionsAsync(
      [
        {
          type: "invokeRegisteredAction",
          actionName: "local_data.delete",
          input: { model: "tasks" },
          inputFromState: { id: "selectedTaskId" },
        },
        {
          type: "invokeRegisteredAction",
          actionName: "local_data.query",
          input: { model: "tasks" },
          resultKey: "tasks",
        },
      ],
      {
        state: {
          ...createStep.state,
          selectedTaskId: createdId,
        },
        toolId: "crud-app",
        bindings,
        onInvokeRegisteredAction,
      },
    );

    expect(deleteStep.errors).toEqual([]);
    const remainingTasks = deleteStep.state.tasks as { records: unknown[]; count: number };
    expect(remainingTasks.count).toBe(0);
    expect(remainingTasks.records).toEqual([]);
  });
});
