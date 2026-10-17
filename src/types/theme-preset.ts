import { z } from "zod";
import { ThemePreferenceSchema, WallpaperConfigSchema } from "@/types/agent";

export const AppearanceSnapshotSchema = z.object({
  schemaVersion: z.literal(1),
  themePreference: ThemePreferenceSchema,
  accentPrimaryLight: z.string(),
  accentPrimaryDark: z.string(),
  accentSecondaryLight: z.string(),
  accentSecondaryDark: z.string(),
  backgroundLight: z.string(),
  backgroundDark: z.string(),
  surfaceLight: z.string(),
  surfaceDark: z.string(),
  surfaceMutedLight: z.string(),
  surfaceMutedDark: z.string(),
  borderLight: z.string(),
  borderDark: z.string(),
  textPrimaryLight: z.string(),
  textPrimaryDark: z.string(),
  textSecondaryLight: z.string(),
  textSecondaryDark: z.string(),
  wallpaper: WallpaperConfigSchema,
  wallpaperJson: z.string().default(""),
  interfaceTransparency: z.number().min(0).max(100),
});

export type AppearanceSnapshot = z.infer<typeof AppearanceSnapshotSchema>;

export const ThemePresetRecordSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  schemaVersion: z.number().int().positive(),
  snapshot: AppearanceSnapshotSchema,
  revision: z.number().int().positive(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type ThemePresetRecord = z.infer<typeof ThemePresetRecordSchema>;

export const ThemePresetCapabilitiesSchema = z.object({
  used: z.number().int().nonnegative(),
  limit: z.number().int().positive(),
  canCreate: z.boolean(),
  planId: z.string(),
  planSource: z.string(),
  freeLimit: z.number().int().positive(),
});

export type ThemePresetCapabilities = z.infer<
  typeof ThemePresetCapabilitiesSchema
>;

export const THEME_PRESET_NAME_MAX = 80;
export const THEME_PRESET_DESCRIPTION_MAX = 280;

export function wallpaperLabelFromSnapshot(snapshot: AppearanceSnapshot): string {
  const json = snapshot.wallpaperJson?.trim();
  if (json) {
    try {
      const parsed = JSON.parse(json) as { type?: string; kind?: string };
      if (typeof parsed.type === "string") return parsed.type.replace(/-/g, " ");
      if (typeof parsed.kind === "string") return parsed.kind;
    } catch {
      /* fall through */
    }
  }
  const kind = snapshot.wallpaper?.kind ?? "none";
  return kind === "none" ? "No wallpaper" : kind;
}
