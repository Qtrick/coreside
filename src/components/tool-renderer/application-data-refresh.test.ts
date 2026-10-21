import { describe, expect, it } from "vitest";
import {
  dataSourceMatchesEventModel,
  hydrationKeysToInvalidate,
  isCurrentHydrationGeneration,
  shouldRefreshForApplicationDataEvent,
} from "./application-data-refresh";

describe("shouldRefreshForApplicationDataEvent", () => {
  const sources = [
    { input: { modelId: "studyTasks", limit: 100 }, resultKey: "studyTasksResult" },
    { input: { model: "notes" }, resultKey: "notesResult" },
  ];

  it("ignores events for a different application", () => {
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-other",
        eventModelId: "studyTasks",
        dataSources: sources,
      }),
    ).toBe(false);
  });

  it("ignores blank local app id", () => {
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "   ",
        eventApplicationId: "tool-study-planner",
        eventModelId: null,
        dataSources: sources,
      }),
    ).toBe(false);
  });

  it("refreshes when modelId matches a dataSource (modelId or model alias)", () => {
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-study-planner",
        eventModelId: "studyTasks",
        dataSources: sources,
      }),
    ).toBe(true);
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-study-planner",
        eventModelId: "notes",
        dataSources: sources,
      }),
    ).toBe(true);
  });

  it("treats inputFromState modelId as a dynamic match", () => {
    const dynamic = [
      {
        input: { limit: 50 },
        inputFromState: { modelId: "selectedModelId" },
        resultKey: "dynResult",
      },
    ];
    expect(
      dataSourceMatchesEventModel(dynamic[0]!, "anything"),
    ).toBe(true);
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-study-planner",
        eventModelId: "studyTasks",
        dataSources: dynamic,
      }),
    ).toBe(true);
  });

  it("does not refresh for an unrelated modelId when sources are scoped", () => {
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-study-planner",
        eventModelId: "unrelatedModel",
        dataSources: sources,
      }),
    ).toBe(false);
  });

  it("refreshes all sources when modelId is absent (delete path)", () => {
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-study-planner",
        eventModelId: null,
        dataSources: sources,
      }),
    ).toBe(true);
    expect(
      shouldRefreshForApplicationDataEvent({
        appId: "tool-study-planner",
        eventApplicationId: "tool-study-planner",
        eventModelId: "  ",
        dataSources: sources,
      }),
    ).toBe(true);
  });
});

describe("hydrationKeysToInvalidate", () => {
  const sources = [
    { input: { modelId: "studyTasks" }, resultKey: "studyTasksResult" },
    { input: { modelId: "notes" }, resultKey: "notesResult" },
  ];

  it("returns all when modelId is absent", () => {
    expect(hydrationKeysToInvalidate(sources, null)).toBe("all");
    expect(hydrationKeysToInvalidate(sources, "")).toBe("all");
  });

  it("returns only matching resultKeys for a scoped model", () => {
    expect(hydrationKeysToInvalidate(sources, "studyTasks")).toEqual([
      "studyTasksResult",
    ]);
    expect(hydrationKeysToInvalidate(sources, "missing")).toEqual([]);
  });
});

describe("isCurrentHydrationGeneration", () => {
  it("accepts matching generation and rejects stale out-of-order results", () => {
    const gens = { studyTasksResult: 3 };
    expect(isCurrentHydrationGeneration(gens, "studyTasksResult", 3)).toBe(true);
    expect(isCurrentHydrationGeneration(gens, "studyTasksResult", 2)).toBe(false);
    expect(isCurrentHydrationGeneration(gens, "otherKey", 3)).toBe(false);
  });
});
