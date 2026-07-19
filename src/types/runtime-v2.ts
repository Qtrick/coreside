import { z } from "zod";
import { ToolDefinitionSchema } from "./tool";

export const MessageVisibilitySchema = z.enum(["visible", "silent", "status"]);

export const OperationTargetSchema = z
  .object({
    surfaceId: z.string().optional(),
    surfaceType: z.string().optional(),
    instanceId: z.string().optional(),
    componentId: z.string().optional(),
    parentId: z.string().optional(),
    toolId: z.string().optional(),
    ownerId: z.string().optional(),
    panelId: z.string().optional(),
    conversationId: z.string().optional(),
    projectId: z.string().optional(),
    messageId: z.string().optional(),
    placement: z.string().optional(),
  })
  .passthrough();

export const AppOperationSchema = z.object({
  id: z.string().min(1),
  type: z.string().min(1),
  target: OperationTargetSchema.default({}),
  baseRevision: z.number().int().optional().nullable(),
  transactionGroup: z.string().optional().nullable(),
  idempotencyKey: z.string().optional().nullable(),
  dependsOn: z.array(z.string()).optional().nullable(),
  payload: z.record(z.unknown()).or(z.unknown()).default({}),
  requiresApproval: z.boolean().optional().nullable(),
  destructive: z.boolean().optional().nullable(),
  audience: z
    .enum([
      "current_user",
      "current_surface",
      "current_chat",
      "current_project",
      "future_participants",
    ])
    .optional()
    .nullable(),
});

export type AppOperation = z.infer<typeof AppOperationSchema>;

export const AgentResponseV2Schema = z.object({
  schemaVersion: z.literal("2").or(z.string()),
  turnId: z.string().optional().nullable(),
  assistantMessages: z
    .array(
      z.object({
        id: z.string(),
        content: z.string(),
        visibility: MessageVisibilitySchema.default("visible"),
      }),
    )
    .default([]),
  assistantMessage: z.string().optional().nullable(),
  operations: z.array(AppOperationSchema).default([]),
  silent: z.boolean().optional().nullable(),
  citations: z.array(z.unknown()).optional().nullable(),
  diagnostics: z.record(z.unknown()).optional().nullable(),
});

export type AgentResponseV2 = z.infer<typeof AgentResponseV2Schema>;

export const SurfaceRecordSchema = z.object({
  id: z.string(),
  instanceId: z.string(),
  surfaceType: z.string(),
  placement: z.string(),
  ownerType: z.string(),
  ownerId: z.string().nullable().optional(),
  conversationId: z.string().nullable().optional(),
  projectId: z.string().nullable().optional(),
  toolId: z.string().nullable().optional(),
  messageId: z.string().nullable().optional(),
  name: z.string(),
  definition: z.union([ToolDefinitionSchema, z.record(z.unknown())]),
  currentRevision: z.number().int(),
  lifecycleState: z.string(),
  archived: z.boolean(),
  capabilityPacks: z.array(z.string()).default([]),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type SurfaceRecord = z.infer<typeof SurfaceRecordSchema>;

export const SurfacePlacementSchema = z.enum([
  "chat_inline",
  "tool_canvas",
  "workspace_panel",
  "project_panel",
  "personal_home",
  "secondary_window",
]);

export type SurfacePlacement = z.infer<typeof SurfacePlacementSchema>;

export type SurfaceDraft = {
  id: string;
  surfaceId: string;
  componentId: string;
  formId?: string | null;
  windowId: string;
  baseRevision: number;
  draft: unknown;
  persistencePolicy: string;
  updatedAt: string;
  createdAt: string;
};

export type RouteState = {
  id: string;
  applicationId: string;
  windowId: string;
  currentRouteId: string | null;
  routeParams: Record<string, unknown>;
  history: unknown[];
  historyIndex: number;
  updatedAt: string;
};

export type NavigateResult = {
  state: RouteState;
  changed: boolean;
};

export type ContinuitySnapshot = {
  id: string;
  surfaceId: string;
  windowId: string;
  focus: Record<string, unknown>;
  scroll: Record<string, unknown>;
  media: Record<string, unknown>;
  suspensionState: "active" | "suspended" | "background";
  updatedAt: string;
};

export type ContextLedgerEntry = {
  id: string;
  conversationId: string;
  projectId?: string | null;
  branchId?: string | null;
  entryType: string;
  visibility: string;
  payload: unknown;
  summary: string;
  expirationClass: string;
  createdAt: string;
};

export type ProviderConformanceRecord = {
  id: string;
  providerId: string;
  modelId: string;
  profile: string;
  capabilities: Record<string, unknown>;
  lastTestedAt?: string | null;
  benchmark?: unknown | null;
  createdAt: string;
  updatedAt: string;
};

export type ScheduledPatch = {
  id: string;
  operationId: string;
  transactionId?: string | null;
  turnId?: string | null;
  conversationId?: string | null;
  surfaceId?: string | null;
  priority: string;
  status: string;
  sequenceNumber: number;
  dependsOn: string[];
  payload: unknown;
  createdAt: string;
  appliedAt?: string | null;
  failedAt?: string | null;
};

export type SurfaceDraftConflict = {
  surfaceId: string;
  componentId: string;
  windowId: string;
  storedRevision: number;
  requestedRevision: number;
  userDraft: unknown;
  agentDraft?: unknown | null;
  formId?: string | null;
};
