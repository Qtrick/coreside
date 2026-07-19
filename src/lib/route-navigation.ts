/** Generated route navigation helpers — mirrors Rust same-route no-op logic. */

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  const obj = value as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableJson(obj[k])}`).join(",")}}`;
}

export function isRouteNavigationNoOp(
  currentRouteId: string | null | undefined,
  currentParams: unknown,
  nextRouteId: string,
  nextParams: unknown,
): boolean {
  return (
    (currentRouteId ?? null) === nextRouteId &&
    stableJson(currentParams ?? {}) === stableJson(nextParams ?? {})
  );
}

export function canNavigateBack(historyIndex: number): boolean {
  return historyIndex > 0;
}

export function canNavigateForward(historyIndex: number, historyLength: number): boolean {
  return historyIndex < historyLength - 1;
}
