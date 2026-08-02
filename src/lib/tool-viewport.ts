/**
 * Trusted tool viewport metadata + fit analysis.
 * Generated tools may declare preferences; they cannot resize the native window.
 */

import { z } from "zod";

export const ToolViewportPolicySchema = z.object({
  minUsefulWidth: z.number().min(280).max(2400).default(520),
  minUsefulHeight: z.number().min(200).max(1800).default(420),
  idealWidth: z.number().min(320).max(2800).optional(),
  idealHeight: z.number().min(240).max(2000).optional(),
  preferredExpansionDirection: z
    .enum(["right", "left", "down", "up", "balanced", "automatic"])
    .default("right"),
  supportsCompactMode: z.boolean().default(true),
  supportsReflow: z.boolean().default(true),
  secondaryWindowPreferred: z.boolean().default(false),
  maxColumns: z.number().int().min(1).max(12).optional(),
  minCardWidth: z.number().min(120).max(800).optional(),
});

export type ToolViewportPolicy = z.infer<typeof ToolViewportPolicySchema>;

export const DEFAULT_TOOL_VIEWPORT: ToolViewportPolicy = {
  minUsefulWidth: 520,
  minUsefulHeight: 420,
  idealWidth: 820,
  idealHeight: 680,
  preferredExpansionDirection: "right",
  supportsCompactMode: true,
  supportsReflow: true,
  secondaryWindowPreferred: false,
};

export type ToolFitOutcome =
  | "fits"
  | "fits_after_reflow"
  | "fits_after_split_adjustment"
  | "native_expansion_recommended"
  | "native_expansion_requires_confirmation"
  | "compact_mode_required"
  | "secondary_window_recommended"
  | "cannot_satisfy_requested_size";

export type AdaptiveWindowSizing = "smart" | "ask" | "off";

export function parseToolViewportPolicy(
  raw: unknown,
): ToolViewportPolicy {
  if (!raw || typeof raw !== "object") return { ...DEFAULT_TOOL_VIEWPORT };
  const nested =
    "viewportPolicy" in raw
      ? (raw as { viewportPolicy: unknown }).viewportPolicy
      : raw;
  const parsed = ToolViewportPolicySchema.safeParse(nested);
  return parsed.success ? parsed.data : { ...DEFAULT_TOOL_VIEWPORT };
}

export function analyzeToolFit(input: {
  canvasWidth: number;
  canvasHeight: number;
  policy: ToolViewportPolicy;
  layoutMode: "wide" | "standard" | "compact";
  adaptiveSizing: AdaptiveWindowSizing;
  canAdjustSplit: boolean;
}): ToolFitOutcome {
  const { canvasWidth, canvasHeight, policy, layoutMode, adaptiveSizing } =
    input;
  const widthOk = canvasWidth >= policy.minUsefulWidth;
  const heightOk = canvasHeight >= policy.minUsefulHeight;

  if (widthOk && heightOk) return "fits";

  if (policy.supportsReflow && layoutMode !== "compact") {
    // Reflow may recover when only slightly under minimum.
    if (
      canvasWidth >= policy.minUsefulWidth * 0.85 &&
      canvasHeight >= policy.minUsefulHeight * 0.85
    ) {
      return "fits_after_reflow";
    }
  }

  if (input.canAdjustSplit && !widthOk) {
    return "fits_after_split_adjustment";
  }

  if (layoutMode === "compact" || policy.supportsCompactMode) {
    if (adaptiveSizing === "off") {
      return policy.secondaryWindowPreferred
        ? "secondary_window_recommended"
        : "compact_mode_required";
    }
  }

  if (adaptiveSizing === "ask") {
    return "native_expansion_requires_confirmation";
  }
  if (adaptiveSizing === "smart") {
    return "native_expansion_recommended";
  }

  if (policy.secondaryWindowPreferred) {
    return "secondary_window_recommended";
  }
  return policy.supportsCompactMode
    ? "compact_mode_required"
    : "cannot_satisfy_requested_size";
}
