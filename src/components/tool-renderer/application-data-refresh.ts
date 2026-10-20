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
};

/**
 * Whether a committed application-data-changed event should bump refresh for
 * this surface. Wrong application is ignored; a modelId only refreshes when a
 * dataSource targets that model; missing modelId (typical delete) refreshes all.
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
  return args.dataSources.some((ds) => {
    const inputModel = ds.input?.modelId ?? ds.input?.model;
    return typeof inputModel === "string" && inputModel.trim() === modelId;
  });
}

/** Out-of-order query responses must not regress UI when a newer gen exists. */
export function isCurrentHydrationGeneration(
  gens: Record<string, number>,
  resultKey: string,
  gen: number,
): boolean {
  return gens[resultKey] === gen;
}
