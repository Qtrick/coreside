import { z } from "zod";
import { ToolDefinitionSchema } from "./tool";

export const ResponseTypeSchema = z.enum(["message", "tool_change", "noop"]);
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
  diagnostics: AgentDiagnosticsSchema,
});

export type AgentResponse = z.infer<typeof AgentResponseSchema>;

export const SendMessageResultSchema = z.object({
  messageId: z.string(),
  assistantMessage: z.string(),
  responseType: ResponseTypeSchema,
  toolChange: ToolChangeSchema.nullable().optional(),
  diagnostics: AgentDiagnosticsSchema,
});

export type SendMessageResult = z.infer<typeof SendMessageResultSchema>;

export const AiStatusSchema = z.object({
  provider: z.string(),
  model: z.string(),
  keyDetected: z.boolean(),
  status: z.enum(["ready", "missing_key", "error", "unconfigured"]),
  message: z.string().optional().nullable(),
});

export type AiStatus = z.infer<typeof AiStatusSchema>;

export const AppInfoSchema = z.object({
  name: z.string(),
  version: z.string(),
  description: z.string().optional().nullable(),
});

export type AppInfo = z.infer<typeof AppInfoSchema>;

export const ThemePreferenceSchema = z.enum(["system", "light", "dark"]);
export type ThemePreference = z.infer<typeof ThemePreferenceSchema>;

export const AppSettingsSchema = z.object({
  theme: ThemePreferenceSchema.default("system"),
  sidebarCollapsed: z.boolean().optional().default(false),
});

export type AppSettings = z.infer<typeof AppSettingsSchema>;
