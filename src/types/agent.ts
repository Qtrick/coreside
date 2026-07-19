import { z } from "zod";
import { ToolDefinitionSchema } from "./tool";
import { SourceCitationSchema } from "./search";

export const ToolCallRequestSchema = z.object({
  capability: z.string(),
  arguments: z.record(z.unknown()).optional().default({}),
});
export type ToolCallRequest = z.infer<typeof ToolCallRequestSchema>;

export const ThemePreferenceSchema = z.enum(["system", "light", "dark"]);
export type ThemePreference = z.infer<typeof ThemePreferenceSchema>;

/** Dock tile: follow OS, or lock to one of the two brand variants. */
export const DockIconPreferenceSchema = z.enum(["auto", "dark", "light"]);
export type DockIconPreference = z.infer<typeof DockIconPreferenceSchema>;

export const ResponseTypeSchema = z.enum([
  "message",
  "tool_change",
  "tool_use",
  "settings_change",
  "noop",
]);
export type ResponseType = z.infer<typeof ResponseTypeSchema>;

export const ToolChangeActionSchema = z.enum(["create", "update", "replace"]);
export type ToolChangeAction = z.infer<typeof ToolChangeActionSchema>;

export const ToolChangeSchema = z.object({
  action: ToolChangeActionSchema,
  targetToolId: z.string().nullable().optional(),
  tool: ToolDefinitionSchema,
  changeSummary: z.string().optional().default(""),
});

export type ToolChange = z.infer<typeof ToolChangeSchema>;

export const AccentVariantsSchema = z.object({
  light: z.string(),
  dark: z.string(),
});

export type AccentVariants = z.infer<typeof AccentVariantsSchema>;

export const WallpaperKindSchema = z.enum([
  "none",
  "matrix",
  "aurora",
  "particles",
  "rain",
  "pulse",
]);
export type WallpaperKind = z.infer<typeof WallpaperKindSchema>;

export const WallpaperConfigSchema = z.object({
  kind: WallpaperKindSchema.default("none"),
  color: z.string().optional(),
  secondaryColor: z.string().optional(),
  speed: z.number().optional(),
  density: z.number().optional(),
  opacity: z.number().optional(),
});
export type WallpaperConfig = z.infer<typeof WallpaperConfigSchema>;

export const DEFAULT_WALLPAPER: WallpaperConfig = { kind: "none" };

export const SettingsChangeSchema = z.object({
  theme: ThemePreferenceSchema.optional(),
  accentPrimary: AccentVariantsSchema.optional(),
  accentSecondary: AccentVariantsSchema.optional(),
  background: AccentVariantsSchema.optional(),
  surface: AccentVariantsSchema.optional(),
  surfaceMuted: AccentVariantsSchema.optional(),
  border: AccentVariantsSchema.optional(),
  textPrimary: AccentVariantsSchema.optional(),
  textSecondary: AccentVariantsSchema.optional(),
  wallpaper: WallpaperConfigSchema.optional(),
  changeSummary: z.string().optional(),
});

export type SettingsChange = z.infer<typeof SettingsChangeSchema>;

export const AgentDiagnosticsSchema = z
  .object({
    promptVersion: z.string().optional(),
    provider: z.string().optional(),
    model: z.string().optional(),
    latencyMs: z.number().optional(),
    parseWarnings: z.array(z.string()).optional(),
    rawExcerpt: z.string().optional(),
  })
  .passthrough()
  .optional();

export type AgentDiagnostics = z.infer<typeof AgentDiagnosticsSchema>;

export const AgentResponseSchema = z.object({
  schemaVersion: z.string().default("1"),
  assistantMessage: z.string(),
  responseType: ResponseTypeSchema,
  toolChange: ToolChangeSchema.nullable().optional(),
  settingsChange: SettingsChangeSchema.nullable().optional(),
  toolCalls: z.array(ToolCallRequestSchema).nullable().optional(),
  citations: z.array(SourceCitationSchema).nullable().optional(),
  diagnostics: AgentDiagnosticsSchema,
});

export type AgentResponse = z.infer<typeof AgentResponseSchema>;

export const SendMessageResultSchema = z.object({
  messageId: z.string(),
  assistantMessage: z.string(),
  responseType: ResponseTypeSchema,
  toolChange: ToolChangeSchema.nullable().optional(),
  settingsChange: SettingsChangeSchema.nullable().optional(),
  diagnostics: AgentDiagnosticsSchema,
  userMessageId: z.string().nullable().optional(),
  runtimeV2: z.record(z.unknown()).nullable().optional(),
  queued: z.boolean().nullable().optional(),
  queueItemId: z.string().nullable().optional(),
});

export type SendMessageResult = z.infer<typeof SendMessageResultSchema>;

export const AiStatusSchema = z.object({
  provider: z.string(),
  model: z.string(),
  keyDetected: z.boolean(),
  status: z.enum(["ready", "missing_key", "error", "unconfigured"]),
  message: z.string().optional().nullable(),
  baseUrl: z.string().optional(),
  envPath: z.string().optional().nullable(),
  /** `connection` | `env` | `none` */
  source: z.enum(["connection", "env", "none"]).optional().default("none"),
  activeConnectionId: z.string().optional().nullable(),
});

export type AiStatus = z.infer<typeof AiStatusSchema>;

export const AppInfoSchema = z.object({
  name: z.string(),
  version: z.string(),
  description: z.string().optional().nullable(),
});

export type AppInfo = z.infer<typeof AppInfoSchema>;

export const AppSettingsSchema = z.object({
  theme: ThemePreferenceSchema.default("system"),
  sidebarCollapsed: z.boolean().optional().default(false),
  /** `"auto"` or a concrete model id such as `gemini-3.5-flash`. */
  preferredModel: z.string().optional().default("auto"),
  /** `auto` follows OS; `dark` / `light` lock the dock tile. */
  dockIcon: DockIconPreferenceSchema.optional().default("auto"),
  accentPrimaryLight: z.string().optional().default("#2f8f63"),
  accentPrimaryDark: z.string().optional().default("#69c994"),
  accentSecondaryLight: z.string().optional().default("#d38b3d"),
  accentSecondaryDark: z.string().optional().default("#e0a158"),
  backgroundLight: z.string().optional().default("#f5f6f1"),
  backgroundDark: z.string().optional().default("#141714"),
  surfaceLight: z.string().optional().default("#ffffff"),
  surfaceDark: z.string().optional().default("#1c201c"),
  surfaceMutedLight: z.string().optional().default("#ecefe8"),
  surfaceMutedDark: z.string().optional().default("#242a24"),
  borderLight: z.string().optional().default("#d8d8d4"),
  borderDark: z.string().optional().default("#3a3a3a"),
  textPrimaryLight: z.string().optional().default("#1d211c"),
  textPrimaryDark: z.string().optional().default("#eef2ec"),
  textSecondaryLight: z.string().optional().default("#687066"),
  textSecondaryDark: z.string().optional().default("#a6afa3"),
  wallpaper: WallpaperConfigSchema.default(DEFAULT_WALLPAPER),
  wallpaperJson: z.string().optional().nullable(),
  /** Base Setting — sanitized Action Log (default off). Legacy boolean. */
  actionLogEnabled: z.boolean().optional().default(false),
  /** Base Setting — `off` | `always` | `intelligent`. */
  actionLogMode: z
    .enum(["off", "always", "intelligent"])
    .optional()
    .default("off"),
  safeSearch: z.enum(["strict", "standard", "off"]).optional().default("standard"),
});

export type AppSettings = z.infer<typeof AppSettingsSchema>;

export const ModelOptionSchema = z.object({
  id: z.string(),
  label: z.string(),
  description: z.string().optional().nullable(),
});

export type ModelOption = z.infer<typeof ModelOptionSchema>;

export const ModelCatalogSchema = z.object({
  provider: z.string(),
  selected: z.string(),
  autoResolvesTo: z.string(),
  options: z.array(ModelOptionSchema),
});

export type ModelCatalog = z.infer<typeof ModelCatalogSchema>;
