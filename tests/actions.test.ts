import { describe, expect, it } from "vitest";
import { applyAction, applyActions, MAX_ACTION_DEPTH } from "@/lib/actions";

describe("actions engine", () => {
  it("setValue, toggle, increment, decrement, reset", () => {
    let state: Record<string, unknown> = { count: 1, on: false, name: "a" };

    state = applyAction(
      { type: "setValue", target: "name", value: "b" },
      { state, toolId: "t" },
    ).state;
    expect(state.name).toBe("b");

    state = applyAction({ type: "toggle", target: "on" }, { state, toolId: "t" })
      .state;
    expect(state.on).toBe(true);

    state = applyAction(
      { type: "increment", target: "count", amount: 2 },
      { state, toolId: "t" },
    ).state;
    expect(state.count).toBe(3);

    state = applyAction(
      { type: "decrement", target: "count" },
      { state, toolId: "t" },
    ).state;
    expect(state.count).toBe(2);

    state = applyAction(
      { type: "reset", target: "count", value: 0 },
      { state, toolId: "t" },
    ).state;
    expect(state.count).toBe(0);
  });

  it("list mutations", () => {
    let state: Record<string, unknown> = {
      items: [{ id: "a", label: "A" }],
    };

    state = applyAction(
      { type: "appendItem", target: "items", item: { id: "b", label: "B" } },
      { state, toolId: "t" },
    ).state;
    expect(state.items).toHaveLength(2);

    state = applyAction(
      { type: "updateItem", target: "items", id: "b", patch: { label: "Bee" } },
      { state, toolId: "t" },
    ).state;
    expect((state.items as Array<{ label: string }>)[1].label).toBe("Bee");

    state = applyAction(
      { type: "removeItem", target: "items", id: "a" },
      { state, toolId: "t" },
    ).state;
    expect(state.items).toHaveLength(1);
  });

  it("selectTab and submitToAgent", () => {
    const submitted: unknown[] = [];
    const state = { tab: "one", total: 10, people: 2 };

    const tabbed = applyAction(
      { type: "selectTab", target: "tab", tabId: "two" },
      { state, toolId: "expense" },
    );
    expect(tabbed.state.tab).toBe("two");

    applyAction(
      {
        type: "submitToAgent",
        eventName: "expense-form-submitted",
        includeFields: ["total", "people"],
      },
      {
        state,
        toolId: "expense",
        onSubmitToAgent: (payload) => submitted.push(payload),
      },
    );

    expect(submitted[0]).toMatchObject({
      toolId: "expense",
      eventName: "expense-form-submitted",
      values: { total: 10, people: 2 },
    });
  });

  it("scopes targets to the current tool", () => {
    const result = applyAction(
      { type: "setValue", target: "secret", value: 1 },
      { state: {}, toolId: "t", allowedTargets: ["count"] },
    );
    expect(result.errors[0]).toMatch(/outside the current tool scope/);
    expect(result.state.secret).toBeUndefined();
  });

  it("protects against action loops", () => {
    const actions = Array.from({ length: MAX_ACTION_DEPTH + 5 }, () => ({
      type: "increment" as const,
      target: "count",
      amount: 1,
    }));
    const result = applyActions(actions, {
      state: { count: 0 },
      toolId: "t",
    });
    expect(result.errors.some((e) => e.includes("loop protection"))).toBe(true);
    expect(result.state.count).toBeLessThanOrEqual(MAX_ACTION_DEPTH);
  });
});
