import { describe, expect, it } from "vitest";

/**
 * Mirrors ToolRenderer's concurrent hydration merge: each completed query
 * writes into a shared results map, then spreads that map over current state
 * so two simultaneous dataSources cannot drop each other's keys.
 */
function mergeHydrationResults(
  state: Record<string, unknown>,
  results: Record<string, unknown>,
  resultKey: string,
  data: unknown,
): Record<string, unknown> {
  results[resultKey] = data;
  return { ...state, ...results, [resultKey]: data };
}

describe("hydration multi-key merge", () => {
  it("preserves both result keys when B resolves before A is committed to state", () => {
    const state: Record<string, unknown> = { unrelated: true };
    const results: Record<string, unknown> = {};

    const afterA = mergeHydrationResults(state, results, "tasksResult", {
      totalCount: 10,
      records: [],
    });
    // Parent has not yet committed afterA into stateRef — B still sees stale state.
    const afterB = mergeHydrationResults(state, results, "summaryResult", {
      totalCount: 10,
      aggregates: { total: 10 },
    });

    expect(afterA.tasksResult).toBeTruthy();
    expect(afterB.tasksResult).toBeTruthy();
    expect(afterB.summaryResult).toBeTruthy();
    expect(afterB.unrelated).toBe(true);
  });

  it("does not drop an earlier key when only spreading stateRef", () => {
    const state: Record<string, unknown> = {};
    const results: Record<string, unknown> = {};
    mergeHydrationResults(state, results, "a", { n: 1 });
    const lostIfNaive: Record<string, unknown> = { ...state, b: { n: 2 } };
    expect(lostIfNaive.a).toBeUndefined();
    const merged = mergeHydrationResults(state, results, "b", { n: 2 });
    expect(merged.a).toEqual({ n: 1 });
    expect(merged.b).toEqual({ n: 2 });
  });
});
