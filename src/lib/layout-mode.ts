/**
 * Application layout mode classification for Chat + Tool Canvas.
 * CSS handles most reflow; this classifies trusted behavioral modes.
 */

export type LayoutMode = "wide" | "standard" | "compact";

export type ToolHeaderDensity = "full" | "icons" | "menu";

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
  /** Tool header widths that force denser action chrome. */
  toolHeaderMenuMax: 480,
  toolHeaderIconsMax: 720,
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

/**
 * Condense Tool Canvas header actions from the header's available width.
 * Measure the header (not the actions row) so labeled buttons cannot inflate
 * the measurement and keep density stuck on "full".
 */
export function classifyToolHeaderDensity(args: {
  headerWidth: number;
  layoutMode: LayoutMode;
}): ToolHeaderDensity {
  const { headerWidth, layoutMode } = args;
  if (layoutMode === "compact") return "menu";
  // Before the first measure, prefer condensed chrome so labeled actions
  // cannot paint past the pane on the initial frame.
  if (!(headerWidth > 0)) {
    return layoutMode === "wide" ? "icons" : "menu";
  }
  if (headerWidth < LAYOUT_SAFE.toolHeaderMenuMax) {
    return "menu";
  }
  if (
    layoutMode === "standard" ||
    headerWidth < LAYOUT_SAFE.toolHeaderIconsMax
  ) {
    return "icons";
  }
  return "full";
}
