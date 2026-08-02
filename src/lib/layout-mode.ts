/**
 * Application layout mode classification for Chat + Tool Canvas.
 * CSS handles most reflow; this classifies trusted behavioral modes.
 */

export type LayoutMode = "wide" | "standard" | "compact";

export const LAYOUT_SAFE = {
  minChatWidth: 320,
  minToolWidth: 360,
  minSidebarExpanded: 200,
  /** Below this main-area width, force compact (one pane). */
  compactMainThreshold: 720,
  /** Below this, prefer standard condensation over wide dual-pane. */
  standardMainThreshold: 960,
  defaultSplitRatio: 0.5,
  minSplitRatio: 0.28,
  maxSplitRatio: 0.72,
} as const;

export type LayoutMetrics = {
  shellWidth: number;
  mainWidth: number;
  sidebarWidth: number;
  toolOpen: boolean;
  splitRatio: number;
};

export function clampSplitRatio(ratio: unknown): number {
  const n = typeof ratio === "number" ? ratio : Number(ratio);
  if (!Number.isFinite(n)) return LAYOUT_SAFE.defaultSplitRatio;
  return Math.min(
    LAYOUT_SAFE.maxSplitRatio,
    Math.max(LAYOUT_SAFE.minSplitRatio, n),
  );
}

/** Classify layout mode from available main content width. */
export function classifyLayoutMode(metrics: LayoutMetrics): LayoutMode {
  if (!metrics.toolOpen) {
    return metrics.mainWidth >= LAYOUT_SAFE.standardMainThreshold
      ? "wide"
      : "standard";
  }
  const chatW = metrics.mainWidth * metrics.splitRatio;
  const toolW = metrics.mainWidth * (1 - metrics.splitRatio);
  if (
    metrics.mainWidth < LAYOUT_SAFE.compactMainThreshold ||
    chatW < LAYOUT_SAFE.minChatWidth ||
    toolW < LAYOUT_SAFE.minToolWidth
  ) {
    return "compact";
  }
  if (metrics.mainWidth < LAYOUT_SAFE.standardMainThreshold) {
    return "standard";
  }
  return "wide";
}

/** Clamp split so both panes keep safe minima within mainWidth. */
export function clampSplitForWidth(
  ratio: number,
  mainWidth: number,
): number {
  const r = clampSplitRatio(ratio);
  if (mainWidth <= 0) return r;
  const minChat = LAYOUT_SAFE.minChatWidth / mainWidth;
  const minTool = LAYOUT_SAFE.minToolWidth / mainWidth;
  if (minChat + minTool >= 1) return 0.5;
  return Math.min(1 - minTool, Math.max(minChat, r));
}
