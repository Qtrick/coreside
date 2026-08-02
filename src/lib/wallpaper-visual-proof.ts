/**
 * Deterministic wallpaper visibility contribution model.
 * Used for visual-proof unit tests until Canvas screenshot fixtures land in CI.
 * Higher transparency → higher wallpaper contribution through a panel (monotonic).
 */

import {
  computeInterfaceTransparencyTokens,
  type InterfaceTransparencyTokens,
} from "./interface-transparency";

/** Synthetic wallpaper RGB under a panel after applying surface alpha. */
export type WallpaperSample = {
  preference: number;
  panelAlpha: number;
  /** 0 = fully opaque panel (no wallpaper), 1 = full wallpaper visible through panel. */
  wallpaperContribution: number;
  /** Blended sample assuming wallpaper rgb(40,180,120) under surface rgb(245,245,242). */
  blended: { r: number; g: number; b: number };
};

const WALLPAPER = { r: 40, g: 180, b: 120 };
const SURFACE = { r: 245, g: 245, b: 242 };

export function wallpaperContributionThroughPanel(
  tokens: InterfaceTransparencyTokens,
): number {
  return clamp01(1 - tokens.panelAlpha);
}

export function sampleWallpaperThroughPanel(preference: number): WallpaperSample {
  const tokens = computeInterfaceTransparencyTokens(preference, {
    wallpaperActive: true,
  });
  const wallpaperContribution = wallpaperContributionThroughPanel(tokens);
  const a = tokens.panelAlpha;
  const blended = {
    r: Math.round(SURFACE.r * a + WALLPAPER.r * (1 - a)),
    g: Math.round(SURFACE.g * a + WALLPAPER.g * (1 - a)),
    b: Math.round(SURFACE.b * a + WALLPAPER.b * (1 - a)),
  };
  return {
    preference: tokens.preference,
    panelAlpha: tokens.panelAlpha,
    wallpaperContribution,
    blended,
  };
}

/** Prove 0→20→40→60 increases wallpaper contribution and shifts color toward wallpaper. */
export function assertMonotonicWallpaperVisibility(
  levels: number[] = [0, 20, 40, 60],
): WallpaperSample[] {
  const samples = levels.map(sampleWallpaperThroughPanel);
  for (let i = 1; i < samples.length; i++) {
    const prev = samples[i - 1]!;
    const cur = samples[i]!;
    if (cur.wallpaperContribution <= prev.wallpaperContribution) {
      throw new Error(
        `Wallpaper contribution not monotonic at ${cur.preference}% vs ${prev.preference}%`,
      );
    }
    // Distance to wallpaper RGB must shrink as transparency rises.
    const prevDist = colorDistance(prev.blended, WALLPAPER);
    const curDist = colorDistance(cur.blended, WALLPAPER);
    if (curDist >= prevDist) {
      throw new Error(
        `Blended color not approaching wallpaper at ${cur.preference}% (nested occlusion would cause this)`,
      );
    }
  }
  if (samples[0]!.wallpaperContribution !== 0) {
    throw new Error("0% transparency must fully occlude wallpaper contribution");
  }
  return samples;
}

function colorDistance(
  a: { r: number; g: number; b: number },
  b: { r: number; g: number; b: number },
): number {
  const dr = a.r - b.r;
  const dg = a.g - b.g;
  const db = a.b - b.b;
  return Math.sqrt(dr * dr + dg * dg + db * db);
}

function clamp01(n: number): number {
  return Math.min(1, Math.max(0, n));
}
