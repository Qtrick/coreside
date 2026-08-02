import { z } from "zod";
import type { WallpaperConfig, WallpaperKind } from "@/types/agent";
import { DEFAULT_WALLPAPER, WallpaperKindSchema } from "@/types/agent";

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

export const SchemaWallpaperConfigSchema = z.object({
  schemaVersion: z.string().default("1"),
  type: SchemaWallpaperTypeSchema,
  color: z.string().optional().nullable(),
  secondaryColor: z.string().optional().nullable(),
  gradientAngle: z.number().optional().nullable(),
  assetId: z.string().optional().nullable(),
  preset: z.string().optional().nullable(),
  slideAssetIds: z.array(z.string()).optional().nullable(),
  intervalMs: z.number().optional().nullable(),
  muted: z.boolean().optional().nullable(),
  reducedMotionFallback: z.string().optional().nullable(),
  opacity: z.number().optional().nullable(),
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
  return {
    schemaVersion: "1",
    type,
    assetId: input.assetId,
    opacity: input.opacity ?? 0.85,
    muted: type === "video-loop" ? true : undefined,
    reducedMotionFallback: input.reducedMotionFallback ?? "#141714",
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
