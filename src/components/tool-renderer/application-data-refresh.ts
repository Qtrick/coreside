/**
 * Pure helpers for ToolRenderer dataSource invalidation / stale-query rejection.
 * Kept free of React so generation races and model scoping stay unit-testable.
 */

export type DataSourceModelInput = {
  input?: {
    modelId?: unknown;
    model?: unknown;
    [key: string]: unknown;
  };
  /** When modelId is bound from state, treat as a dynamic match for scoped events. */
  inputFromState?: Record<string, string>;
  resultKey?: string;
};

/** Static modelId/model from input, if present. */
export function staticDataSourceModelId(ds: DataSourceModelInput): string | null {
  const inputModel = ds.input?.modelId ?? ds.input?.model;
  if (typeof inputModel === "string" && inputModel.trim()) {
    return inputModel.trim();
  }
  return null;
}

/**
 * Whether this dataSource could be affected by a model-scoped event.
 * - Static modelId/model match → yes
 * - modelId (or model) supplied only via inputFromState → yes (dynamic; fail-open for that source)
 * - Unrelated static model → no
 */
export function dataSourceMatchesEventModel(
  ds: DataSourceModelInput,
  eventModelId: string,
): boolean {
  const staticModel = staticDataSourceModelId(ds);
  if (staticModel) return staticModel === eventModelId;
  const fromState = ds.inputFromState;
  if (!fromState) return false;
  return Object.keys(fromState).some((key) => {
    const path = key.trim();
    return path === "modelId" || path === "model" || path.endsWith(".modelId") || path.endsWith(".model");
  });
}

/**
 * Whether a committed application-data-changed event should bump refresh for
 * this surface. Wrong application is ignored; a modelId only refreshes when a
 * dataSource targets that model (static) or binds model via inputFromState;
 * missing modelId refreshes all.
 */
export function shouldRefreshForApplicationDataEvent(args: {
  appId: string;
  eventApplicationId: string;
  eventModelId: string | null | undefined;
  dataSources: DataSourceModelInput[];
}): boolean {
  const appId = args.appId.trim();
  if (!appId || args.eventApplicationId !== appId) return false;
  const modelId =
    typeof args.eventModelId === "string" ? args.eventModelId.trim() : "";
  if (!modelId) return true;
  return args.dataSources.some((ds) => dataSourceMatchesEventModel(ds, modelId));
}

/**
 * Result keys whose hydration cache should be cleared for this event.
 * Empty model → all keys; otherwise only matching dataSources.
 */
export function hydrationKeysToInvalidate(
  dataSources: DataSourceModelInput[],
  eventModelId: string | null | undefined,
): string[] | "all" {
  const modelId =
    typeof eventModelId === "string" ? eventModelId.trim() : "";
  if (!modelId) return "all";
  const keys = dataSources
    .filter((ds) => dataSourceMatchesEventModel(ds, modelId))
    .map((ds) => (typeof ds.resultKey === "string" ? ds.resultKey.trim() : ""))
    .filter(Boolean);
  return keys;
}

/** Out-of-order query responses must not regress UI when a newer gen exists. */
export function isCurrentHydrationGeneration(
  gens: Record<string, number>,
  resultKey: string,
  gen: number,
): boolean {
  return gens[resultKey] === gen;
}
