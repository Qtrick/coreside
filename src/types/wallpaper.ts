import { z } from "zod";
import type { WallpaperConfig, WallpaperKind } from "@/types/agent";
import { DEFAULT_WALLPAPER, WallpaperKindSchema } from "@/types/agent";
import {
  WallpaperFilterConfigSchema,
  type WallpaperFilterConfig,
} from "@/lib/wallpaper-filter";
import { normalizeCanonicalHex } from "@/lib/wallpaper-hex";

export type { WallpaperFilterConfig };
export {
  WALLPAPER_FILTER_PRESETS,
  cssFilterFromConfig,
  WallpaperFilterConfigSchema,
} from "@/lib/wallpaper-filter";
export {
  isCanonicalHexColor,
  normalizeCanonicalHex,
} from "@/lib/wallpaper-hex";

export const SchemaWallpaperTypeSchema = z.enum([
  "static-color",
  "linear-gradient",
  "ambient-gradient",
  "image-cover",
  "video-loop",
  "animated-image",
  "slideshow",
  "floating-particles",
  "canvas-preset",
]);

export type SchemaWallpaperType = z.infer<typeof SchemaWallpaperTypeSchema>;

const optionalHexColor = z
  .string()
  .optional()
  .nullable()
  .superRefine((v, ctx) => {
    if (v == null || v === "") return;
    if (normalizeCanonicalHex(v) == null) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: "expected #RRGGBB hex color",
      });
    }
  })
  .transform((v) => {
    if (v == null || v === "") return v;
    return normalizeCanonicalHex(v) ?? v;
  });

export const SchemaWallpaperConfigSchema = z.object({
  schemaVersion: z.string().default("1"),
  type: SchemaWallpaperTypeSchema,
  color: optionalHexColor,
  secondaryColor: optionalHexColor,
  gradientAngle: z.number().optional().nullable(),
  assetId: z.string().optional().nullable(),
  preset: z.string().optional().nullable(),
  slideAssetIds: z.array(z.string()).optional().nullable(),
  intervalMs: z.number().optional().nullable(),
  muted: z.boolean().optional().nullable(),
  reducedMotionFallback: optionalHexColor,
  opacity: z.number().optional().nullable(),
  filter: WallpaperFilterConfigSchema.optional().nullable(),
  extra: z
    .object({
      fit: z.enum(["cover", "contain"]).optional(),
      speed: z.number().optional(),
      density: z.number().optional(),
    })
    .passthrough()
    .optional()
    .nullable(),
});

export type SchemaWallpaperConfig = z.infer<typeof SchemaWallpaperConfigSchema>;

export type ResolvedWallpaper =
  | { format: "none" }
  | { format: "legacy"; config: WallpaperConfig }
  | { format: "schema"; config: SchemaWallpaperConfig };

export const CANVAS_PRESETS: Array<{
  id: WallpaperKind;
  label: string;
  description: string;
}> = [
  { id: "none", label: "None", description: "Solid app background only" },
  { id: "matrix", label: "Matrix", description: "Falling glyphs" },
  { id: "aurora", label: "Aurora", description: "Soft moving gradients" },
  { id: "particles", label: "Particles", description: "Floating dots" },
  { id: "rain", label: "Rain", description: "Gentle rainfall" },
  { id: "pulse", label: "Pulse", description: "Radial breathing glow" },
];

export function parseWallpaperJson(raw: string | null | undefined): ResolvedWallpaper {
  if (!raw?.trim()) return { format: "none" };
  try {
    const value = JSON.parse(raw) as unknown;
    if (!value || typeof value !== "object") return { format: "none" };
    const obj = value as Record<string, unknown>;
    if (obj.schemaVersion === "1" && typeof obj.type === "string") {
      const parsed = SchemaWallpaperConfigSchema.safeParse(value);
      if (parsed.success) return { format: "schema", config: parsed.data };
      return { format: "none" };
    }
    const kind = WallpaperKindSchema.safeParse(obj.kind);
    if (!kind.success || kind.data === "none") return { format: "none" };
    return {
      format: "legacy",
      config: {
        kind: kind.data,
        color: typeof obj.color === "string" ? obj.color : undefined,
        secondaryColor:
          typeof obj.secondaryColor === "string" ? obj.secondaryColor : undefined,
        speed: typeof obj.speed === "number" ? obj.speed : undefined,
        density: typeof obj.density === "number" ? obj.density : undefined,
        opacity: typeof obj.opacity === "number" ? obj.opacity : undefined,
      },
    };
  } catch {
    return { format: "none" };
  }
}

export function legacyWallpaperToJson(config: WallpaperConfig): string {
  return JSON.stringify(config);
}

export function schemaWallpaperToJson(config: SchemaWallpaperConfig): string {
  return JSON.stringify({
    ...config,
    schemaVersion: config.schemaVersion ?? "1",
  });
}

export function buildStaticColorProposal(
  color: string,
  filter?: WallpaperFilterConfig | null,
): SchemaWallpaperConfig | null {
  const hex = normalizeCanonicalHex(color);
  if (!hex) return null;
  return {
    schemaVersion: "1",
    type: "static-color",
    color: hex,
    opacity: 1,
    filter: filter ?? undefined,
    reducedMotionFallback: hex,
  };
}

export function buildMediaWallpaperProposal(input: {
  assetId: string;
  category: string;
  fit?: "cover" | "contain";
  opacity?: number;
  reducedMotionFallback?: string;
}): SchemaWallpaperConfig {
  const type =
    input.category === "video"
      ? "video-loop"
      : input.category === "animated"
        ? "animated-image"
        : "image-cover";
  const fallback =
    normalizeCanonicalHex(input.reducedMotionFallback ?? "#141714") ?? "#141714";
  return {
    schemaVersion: "1",
    type,
    assetId: input.assetId,
    opacity: input.opacity ?? 0.85,
    muted: type === "video-loop" ? true : undefined,
    reducedMotionFallback: fallback,
    extra:
      type === "image-cover"
        ? { fit: input.fit ?? "cover" }
        : undefined,
  };
}

export function buildCanvasPresetProposal(
  preset: WallpaperKind,
  opacity?: number,
): SchemaWallpaperConfig | WallpaperConfig {
  // Callers that apply "none" should clear wallpaperJson (empty string), not
  // persist this legacy sentinel as schema wallpaperJson.
  if (preset === "none") return DEFAULT_WALLPAPER;
  if (preset === "particles") {
    return {
      schemaVersion: "1",
      type: "floating-particles",
      preset: "particles",
      color: "#6ab0d4",
      secondaryColor: "#33ff66",
      opacity: opacity ?? 0.35,
      reducedMotionFallback: "#141714",
    };
  }
  return {
    schemaVersion: "1",
    type: "canvas-preset",
    preset,
    color: preset === "matrix" ? "#33ff66" : "#6ab0d4",
    secondaryColor: "#6ab0d4",
    opacity:
      opacity ??
      (preset === "matrix" ? 0.42 : preset === "aurora" ? 0.55 : 0.35),
    reducedMotionFallback: preset === "matrix" ? "#050805" : "#141714",
    extra: {
      speed: 1,
      density: preset === "matrix" ? 0.7 : 0.55,
    },
  };
}

export function wallpaperDataAttribute(resolved: ResolvedWallpaper): string | undefined {
  if (resolved.format === "none") return undefined;
  if (resolved.format === "legacy") {
    return resolved.config.kind !== "none" ? resolved.config.kind : undefined;
  }
  if (resolved.config.type === "canvas-preset") {
    return resolved.config.preset ?? resolved.config.type;
  }
  return resolved.config.type;
}

export interface WallpaperPresetDefinition {
  id: string;
  name: string;
  category:
    | "featured"
    | "ambient"
    | "minimal"
    | "space"
    | "particles"
    | "rain"
    | "motion"
    | "media"
    | "custom"
    | "solid"
    | "legacy";
  description: string;
  config: SchemaWallpaperConfig;
  previewColors: [string, string, string?];
}

export const CURATED_WALLPAPER_PRESETS: WallpaperPresetDefinition[] = [
  {
    id: "obsidian-flow",
    name: "Obsidian Flow",
    category: "featured",
    description: "Deep obsidian backdrop with calm twilight indigo depth",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#0a0c10",
      secondaryColor: "#1a1d28",
      gradientAngle: 135,
      opacity: 0.95,
      reducedMotionFallback: "#0a0c10",
    },
    previewColors: ["#0a0c10", "#141822", "#1a1d28"],
  },
  {
    id: "starlight-embers",
    name: "Starlight Embers",
    category: "featured",
    description: "Luminescent floating particles drifting through nocturnal space",
    config: {
      schemaVersion: "1",
      type: "floating-particles",
      preset: "particles",
      color: "#38bdf8",
      secondaryColor: "#818cf8",
      opacity: 0.35,
      reducedMotionFallback: "#07090e",
      extra: { speed: 0.5, density: 0.4 },
    },
    previewColors: ["#07090e", "#111726", "#38bdf8"],
  },
  {
    id: "midnight-rain",
    name: "Midnight Rain",
    category: "featured",
    description: "Atmospheric mist and delicate precipitation over deep slate",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "rain",
      color: "#64748b",
      secondaryColor: "#94a3b8",
      opacity: 0.3,
      reducedMotionFallback: "#06090e",
      extra: { speed: 0.6, density: 0.35 },
    },
    previewColors: ["#06090e", "#0e1626", "#64748b"],
  },
  {
    id: "harmonic-aurora",
    name: "Harmonic Aurora",
    category: "featured",
    description: "Atmospheric plasma waves undulating across dark emerald horizons",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "aurora",
      color: "#0d5c52",
      secondaryColor: "#14b8a6",
      opacity: 0.5,
      reducedMotionFallback: "#041a14",
      extra: { speed: 0.5, density: 0.45 },
    },
    previewColors: ["#041a14", "#0a3a2e", "#14b8a6"],
  },
  {
    id: "obsidian",
    name: "Obsidian",
    category: "minimal",
    description: "Deep charcoal with subtle obsidian undertones",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#0d0f12",
      secondaryColor: "#1a1d24",
      gradientAngle: 135,
      opacity: 0.95,
      reducedMotionFallback: "#0d0f12",
    },
    previewColors: ["#0d0f12", "#14171d", "#1a1d24"],
  },
  {
    id: "soft-graphite",
    name: "Soft Graphite",
    category: "minimal",
    description: "Neutral, glare-free dark matte finish",
    config: {
      schemaVersion: "1",
      type: "static-color",
      color: "#16181d",
      opacity: 1,
      reducedMotionFallback: "#16181d",
    },
    previewColors: ["#16181d", "#16181d"],
  },
  {
    id: "paper-light",
    name: "Paper / Light",
    category: "minimal",
    description: "Warm neutral parchment for high-contrast day focus",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#f4f4f0",
      secondaryColor: "#e8e8e0",
      gradientAngle: 180,
      opacity: 0.95,
      reducedMotionFallback: "#f4f4f0",
    },
    previewColors: ["#f4f4f0", "#eeeeea", "#e8e8e0"],
  },
  {
    id: "blue-hour",
    name: "Blue Hour",
    category: "ambient",
    description: "Calm twilight blue meeting deep night indigo",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#0b1426",
      secondaryColor: "#172554",
      gradientAngle: 160,
      opacity: 0.85,
      reducedMotionFallback: "#0b1426",
    },
    previewColors: ["#0b1426", "#111c38", "#172554"],
  },
  {
    id: "warm-dusk",
    name: "Warm Dusk",
    category: "ambient",
    description: "Muted amber and plum evening horizon",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#1c1214",
      secondaryColor: "#2d1b24",
      gradientAngle: 120,
      opacity: 0.85,
      reducedMotionFallback: "#1c1214",
    },
    previewColors: ["#1c1214", "#24171c", "#2d1b24"],
  },
  {
    id: "muted-ocean",
    name: "Muted Ocean",
    category: "ambient",
    description: "Restrained deep slate teal with calm oceanic depth",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#08181c",
      secondaryColor: "#132e35",
      gradientAngle: 145,
      opacity: 0.85,
      reducedMotionFallback: "#08181c",
    },
    previewColors: ["#08181c", "#0d2328", "#132e35"],
  },
  {
    id: "quiet-bloom",
    name: "Quiet Bloom",
    category: "ambient",
    description: "Subtle mauve and warm slate undertone",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#161218",
      secondaryColor: "#251c28",
      gradientAngle: 135,
      opacity: 0.85,
      reducedMotionFallback: "#161218",
    },
    previewColors: ["#161218", "#1d1720", "#251c28"],
  },
  {
    id: "deep-space",
    name: "Deep Space",
    category: "space",
    description: "Expansive cosmic dark with gentle violet glow",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#09090b",
      secondaryColor: "#1e1b4b",
      gradientAngle: 210,
      opacity: 0.9,
      reducedMotionFallback: "#09090b",
    },
    previewColors: ["#09090b", "#13122c", "#1e1b4b"],
  },
  {
    id: "cosmic-void",
    name: "Cosmic Void",
    category: "space",
    description: "Deep starlit darkness with subtle stellar glow",
    config: {
      schemaVersion: "1",
      type: "ambient-gradient",
      color: "#050608",
      secondaryColor: "#0f172a",
      gradientAngle: 180,
      opacity: 0.92,
      reducedMotionFallback: "#050608",
    },
    previewColors: ["#050608", "#0a0e18", "#0f172a"],
  },
  {
    id: "geometric-drift",
    name: "Geometric Drift",
    category: "particles",
    description: "Gentle floating particles with slow, restrained drift",
    config: {
      schemaVersion: "1",
      type: "floating-particles",
      preset: "particles",
      color: "#475569",
      secondaryColor: "#64748b",
      opacity: 0.28,
      reducedMotionFallback: "#0f172a",
      extra: { speed: 0.4, density: 0.35 },
    },
    previewColors: ["#0f172a", "#1e293b", "#475569"],
  },
  {
    id: "gentle-mist",
    name: "Gentle Mist",
    category: "rain",
    description: "Delicate rainfall streaks with soft ambient diffusion",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "rain",
      color: "#475569",
      secondaryColor: "#64748b",
      opacity: 0.25,
      reducedMotionFallback: "#090d16",
      extra: { speed: 0.5, density: 0.3 },
    },
    previewColors: ["#090d16", "#131c2e", "#475569"],
  },
  {
    id: "aurora-mist",
    name: "Aurora Mist",
    category: "motion",
    description: "Slow-moving soft emerald and teal mist",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "aurora",
      color: "#064e3b",
      secondaryColor: "#0f766e",
      opacity: 0.45,
      reducedMotionFallback: "#064e3b",
      extra: { speed: 0.6, density: 0.4 },
    },
    previewColors: ["#064e3b", "#0d5c52", "#0f766e"],
  },
  // Legacy presets preserved for backward compatibility
  {
    id: "matrix",
    name: "Matrix (Legacy)",
    category: "legacy",
    description: "Falling glyphs (legacy developer effect)",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "matrix",
      color: "#22c55e",
      secondaryColor: "#15803d",
      opacity: 0.35,
      reducedMotionFallback: "#050805",
      extra: { speed: 0.8, density: 0.5 },
    },
    previewColors: ["#050805", "#0a1f0a", "#22c55e"],
  },
  {
    id: "rain",
    name: "Rain (Legacy)",
    category: "legacy",
    description: "Gentle rainfall streaks",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "rain",
      color: "#64748b",
      secondaryColor: "#94a3b8",
      opacity: 0.3,
      reducedMotionFallback: "#0f172a",
      extra: { speed: 0.7, density: 0.4 },
    },
    previewColors: ["#0f172a", "#1e293b", "#64748b"],
  },
  {
    id: "pulse",
    name: "Pulse (Legacy)",
    category: "legacy",
    description: "Slow radial breathing glow",
    config: {
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "pulse",
      color: "#3b82f6",
      secondaryColor: "#1d4ed8",
      opacity: 0.3,
      reducedMotionFallback: "#0b1426",
      extra: { speed: 0.5 },
    },
    previewColors: ["#0b1426", "#1e3a8a", "#3b82f6"],
  },
];
