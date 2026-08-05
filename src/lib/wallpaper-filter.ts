import { z } from "zod";

/**
 * Typed wallpaper-layer filter (never raw CSS filter strings from the agent).
 * Applied only to `.live-wallpaper` — never to protected UI.
 */
export const WallpaperFilterConfigSchema = z.object({
  preset: z.enum(["none", "soft", "vivid", "muted", "blur"]).optional(),
  brightness: z.number().min(0.5).max(1.5).optional(),
  contrast: z.number().min(0.5).max(1.5).optional(),
  saturate: z.number().min(0).max(2).optional(),
  blurPx: z.number().min(0).max(20).optional(),
});

export type WallpaperFilterConfig = z.infer<typeof WallpaperFilterConfigSchema>;

export const WALLPAPER_FILTER_PRESETS: ReadonlyArray<{
  id: NonNullable<WallpaperFilterConfig["preset"]>;
  label: string;
  config: WallpaperFilterConfig;
}> = [
  { id: "none", label: "None", config: { preset: "none" } },
  {
    id: "soft",
    label: "Soft",
    config: { preset: "soft", brightness: 1.05, contrast: 0.95, saturate: 0.9 },
  },
  {
    id: "vivid",
    label: "Vivid",
    config: { preset: "vivid", brightness: 1.05, contrast: 1.1, saturate: 1.35 },
  },
  {
    id: "muted",
    label: "Muted",
    config: { preset: "muted", brightness: 0.95, contrast: 0.9, saturate: 0.55 },
  },
  {
    id: "blur",
    label: "Blur",
    config: { preset: "blur", blurPx: 6, brightness: 1, saturate: 1 },
  },
];

function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

/** Build a safe CSS `filter` value for the wallpaper layer only. */
export function cssFilterFromConfig(
  filter: WallpaperFilterConfig | null | undefined,
): string | undefined {
  if (!filter) return undefined;
  const preset = WALLPAPER_FILTER_PRESETS.find((p) => p.id === filter.preset);
  const merged: WallpaperFilterConfig = {
    ...(preset?.config ?? {}),
    ...filter,
  };
  if (merged.preset === "none") {
    const hasCustom =
      merged.brightness != null ||
      merged.contrast != null ||
      merged.saturate != null ||
      (merged.blurPx != null && merged.blurPx > 0);
    if (!hasCustom) return undefined;
  }
  const parts: string[] = [];
  if (merged.brightness != null) {
    parts.push(`brightness(${clamp(merged.brightness, 0.5, 1.5)})`);
  }
  if (merged.contrast != null) {
    parts.push(`contrast(${clamp(merged.contrast, 0.5, 1.5)})`);
  }
  if (merged.saturate != null) {
    parts.push(`saturate(${clamp(merged.saturate, 0, 2)})`);
  }
  if (merged.blurPx != null && merged.blurPx > 0) {
    parts.push(`blur(${clamp(merged.blurPx, 0, 20)}px)`);
  }
  return parts.length > 0 ? parts.join(" ") : undefined;
}
