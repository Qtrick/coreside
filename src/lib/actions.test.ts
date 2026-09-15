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

    // Declared component bindings are allowed:
    expect(targets.has("input-name")).toBe(true);
    expect(targets.has("userName")).toBe(true);
    expect(targets.has("queriedRecords")).toBe(true);

    // CRITICAL P0: inputFromState keys MUST NOT be authorized:
    expect(targets.has("superSecretApiKey")).toBe(false);
    expect(targets.has("secretToken")).toBe(false);
  });
});
