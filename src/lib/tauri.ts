import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AiStatus,
  AppInfo,
  AppSettings,
  DockIconPreference,
  ModelCatalog,
  SendMessageResult,
  ThemePreference,
  ToolChange,
  WallpaperConfig,
} from "@/types/agent";
import { DEFAULT_WALLPAPER, WallpaperKindSchema } from "@/types/agent";
import type { ChatMessage, Conversation } from "@/types/messages";
import type {
  AddedSetting,
  UpsertAddedSettingInput,
  ValidateChangeTargetsResult,
} from "@/types/settings";
import type {
  ProviderConnection,
  ProviderHint,
  UpsertProviderConnectionInput,
} from "@/types/providers";
import type {
  CreateProjectInput,
  DeleteProjectMode,
  Project,
  ProjectContextHit,
  UpdateProjectInput,
} from "@/types/project";
import type {
  CacheStats,
  CrawlerInstallation,
  CrawlerStatus,
  ExaBudgetStatus,
  ExaConnection,
  ExaUsageSummary,
  ResourceProfile,
  SearchConnection,
  SafeSearchLevel,
  SearchProfileView,
  SearchSessionDetail,
  SearchSessionSummary,
  SearchUsageProfile,
} from "@/types/search";
import type {
  ImportMediaInput,
  MediaAsset,
  MediaAssetSrc,
  MediaAssetUsage,
} from "@/types/media";
import type {
  ActionOutcome,
  ApprovalDecisionResult,
  ApprovalRequest,
  ApplicationVersion,
  AuditEvent,
  BuildFailure,
  ClientActionRequest,
  ManifestRecord,
  RecoveryState,
  RememberDuration,
  RememberScope,
  RuntimeGrant,
} from "@/types/application-kernel";
import type {
  ToolDefinition,
  ToolState,
  ToolSummary,
  ToolVersion,
} from "@/types/tool";

export type AgentTurnEvent =
  | { kind: "action"; conversationId: string; label: string }
  | { kind: "text"; conversationId: string; text: string }
  | { kind: "error"; conversationId: string; message: string }
  | {
      kind: "operation";
      conversationId: string;
      operationId: string;
      status: string;
    }
  | {
      kind: "sync";
      conversationId?: string | null;
      surfaceIds: string[];
      revision?: number | null;
      syncKind: string;
    }
  | {
      kind: "conflict";
      conversationId?: string | null;
      message: string;
      conflicts: string[];
    };

const agentTurnHandlers = new Set<(event: AgentTurnEvent) => void>();
let agentTurnUnlisten: (() => void) | null = null;

/** Subscribe to live agent-turn events. Returns an unsubscribe fn. */
export async function listenAgentTurn(
  handler: (event: AgentTurnEvent) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  agentTurnHandlers.add(handler);
  if (!agentTurnUnlisten) {
    agentTurnUnlisten = await listen<AgentTurnEvent>("agent-turn", (event) => {
      for (const h of agentTurnHandlers) {
        h(event.payload);
      }
    });
  }
  return () => {
    agentTurnHandlers.delete(handler);
    if (agentTurnHandlers.size === 0 && agentTurnUnlisten) {
      agentTurnUnlisten();
      agentTurnUnlisten = null;
    }
  };
}

export class TauriCommandError extends Error {
  readonly code?: string;

  constructor(message: string, code?: string) {
    super(message);
    this.name = "TauriCommandError";
    this.code = code;
  }
}

function formatInvokeError(error: unknown): { message: string; code?: string } {
  if (typeof error === "string") {
    return { message: error || "Command failed" };
  }
  if (error instanceof Error) {
    return { message: error.message || "Command failed" };
  }
  if (error && typeof error === "object") {
    const obj = error as Record<string, unknown>;
    const code = typeof obj.code === "string" ? obj.code : undefined;
    if (typeof obj.message === "string" && obj.message.trim()) {
      return { message: obj.message, code };
    }
    // Tauri sometimes nests the payload.
    const nested = obj.error;
    if (nested && typeof nested === "object") {
      const inner = nested as Record<string, unknown>;
      const nestedCode =
        typeof inner.code === "string" ? inner.code : code;
      if (typeof inner.message === "string" && inner.message.trim()) {
        return { message: inner.message, code: nestedCode };
      }
    }
    try {
      return { message: JSON.stringify(error), code };
    } catch {
      return { message: "Command failed", code };
    }
  }
  return { message: "Command failed" };
}

function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return mockInvoke<T>(command, args);
  }
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    const { message, code } = formatInvokeError(error);
    throw new TauriCommandError(message, code);
  }
}

/** Backend ToolRecord shape from Rust IPC. */
type ToolRecord = {
  id: string;
  workspaceId?: string;
  name: string;
  description?: string;
  layout?: unknown;
  definition: ToolDefinition;
  currentVersion: number;
  createdAt?: string;
  updatedAt?: string;
};

function isToolRecord(value: unknown): value is ToolRecord {
  return (
    typeof value === "object" &&
    value !== null &&
    "definition" in value &&
    "currentVersion" in value
  );
}

function toToolDefinition(record: ToolRecord): ToolDefinition {
  return {
    ...record.definition,
    version: record.currentVersion,
  };
}

function toToolSummary(record: ToolRecord): ToolSummary {
  return {
    id: record.id,
    name: record.name,
    description: record.description ?? record.definition.description ?? "",
    version: record.currentVersion,
    updatedAt: record.updatedAt ?? null,
    createdAt: record.createdAt ?? null,
  };
}

function isProtectedId(id: string): boolean {
  return id.trim().startsWith("core.");
}

const HEX_COLOR = /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})$/;

function normalizeMockHex(raw: string): string | null {
  const s = raw.trim();
  if (!HEX_COLOR.test(s)) return null;
  if (s.length === 4) {
    return `#${s[1]}${s[1]}${s[2]}${s[2]}${s[3]}${s[3]}`.toLowerCase();
  }
  return s.toLowerCase();
}

function sanitizeMockWallpaper(value: unknown): WallpaperConfig {
  let raw: unknown = value;
  if (typeof value === "string") {
    try {
      raw = JSON.parse(value);
    } catch {
      return { ...DEFAULT_WALLPAPER };
    }
  }
  if (!raw || typeof raw !== "object") return { ...DEFAULT_WALLPAPER };
  const obj = raw as Record<string, unknown>;
  const kind = WallpaperKindSchema.safeParse(obj.kind);
  if (!kind.success) return { ...DEFAULT_WALLPAPER };
  const color =
    typeof obj.color === "string" ? normalizeMockHex(obj.color) : undefined;
  const secondaryColor =
    typeof obj.secondaryColor === "string"
      ? normalizeMockHex(obj.secondaryColor)
      : undefined;
  const clamp = (n: unknown, min: number, max: number) =>
    typeof n === "number" && Number.isFinite(n)
      ? Math.min(max, Math.max(min, n))
      : undefined;
  return {
    kind: kind.data,
    color: color ?? undefined,
    secondaryColor: secondaryColor ?? undefined,
    speed: clamp(obj.speed, 0.25, 3),
    density: clamp(obj.density, 0.1, 1),
    opacity: clamp(obj.opacity, 0.05, 0.85),
  };
}

/* ─── In-memory mocks for vitest / web preview ─── */

const now = () => new Date().toISOString();

const mockDb = {
  conversations: [] as Conversation[],
  projects: [] as Project[],
  messages: new Map<string, ChatMessage[]>(),
  tools: [] as ToolDefinition[],
  toolState: new Map<string, ToolState>(),
  toolVersions: new Map<string, ToolVersion[]>(),
  addedSettings: [] as AddedSetting[],
  settings: {
    theme: "system" as ThemePreference,
    sidebarCollapsed: false as boolean,
    preferredModel: "auto",
    dockIcon: "auto" as DockIconPreference,
    accentPrimaryLight: "#2f8f63",
    accentPrimaryDark: "#69c994",
    accentSecondaryLight: "#d38b3d",
    accentSecondaryDark: "#e0a158",
    backgroundLight: "#f5f6f1",
    backgroundDark: "#141714",
    surfaceLight: "#ffffff",
    surfaceDark: "#1c201c",
    surfaceMutedLight: "#ecefe8",
    surfaceMutedDark: "#242a24",
    borderLight: "#d8d8d4",
    borderDark: "#3a3a3a",
    textPrimaryLight: "#1d211c",
    textPrimaryDark: "#eef2ec",
    textSecondaryLight: "#687066",
    textSecondaryDark: "#a6afa3",
    wallpaper: { ...DEFAULT_WALLPAPER } as WallpaperConfig,
    wallpaperJson: null as string | null,
    actionLogEnabled: false as boolean,
    actionLogMode: "off" as "off" | "always" | "intelligent",
    safeSearch: "standard" as SafeSearchLevel,
    developerMode: false as boolean,
  } satisfies AppSettings,
  aiConfigured: false,
  providerConnections: [] as ProviderConnection[],
  mediaAssets: [] as MediaAsset[],
  surfaceState: new Map<string, Record<string, unknown>>(),
  surfaceDrafts: new Map<string, import("@/types/runtime-v2").SurfaceDraft>(),
  routeState: new Map<string, import("@/types/runtime-v2").RouteState>(),
  continuity: new Map<string, import("@/types/runtime-v2").ContinuitySnapshot>(),
  contextLedger: [] as import("@/types/runtime-v2").ContextLedgerEntry[],
  kernelManifests: [] as ManifestRecord[],
  kernelPendingApprovals: [] as ApprovalRequest[],
  kernelRuntimeGrants: [] as RuntimeGrant[],
  kernelAuditEvents: [] as AuditEvent[],
  kernelBuildFailures: [] as BuildFailure[],
  searchSessions: [] as Array<{
    id: string;
    conversationId?: string | null;
    projectId?: string | null;
    searchType: string;
    query: string;
    provider: string;
    createdAt: string;
    results: Array<{
      id: string;
      sessionId: string;
      resultType: string;
      title?: string | null;
      url?: string | null;
      displayDomain?: string | null;
      snippet?: string | null;
      thumbnailUrl?: string | null;
      metadataJson?: string | null;
      createdAt: string;
    }>;
  }>,
};

function titleFromContent(content: string): string {
  const trimmed = content.trim().replace(/\s+/g, " ");
  if (!trimmed) return "New chat";
  return trimmed.length > 48 ? `${trimmed.slice(0, 45)}…` : trimmed;
}

async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  switch (command) {
    case "get_app_info":
      return {
        name: "Coreside",
        version: "0.1.0",
        description:
          "An AI-native personal software environment that begins as a chatbot and builds tools inside itself.",
      } satisfies AppInfo as T;

    case "get_ai_status": {
      const hasByok = mockDb.providerConnections.some((c) => c.isActive && c.hasKey);
      const configured = mockDb.aiConfigured;
      if (hasByok) {
        const active = mockDb.providerConnections.find((c) => c.isActive)!;
        return {
          provider: active.provider,
          model: active.modelDefault || "auto",
          keyDetected: true,
          status: "ready",
          message: "Connected with a secure provider credential.",
          source: "connection",
          activeConnectionId: active.id,
          accessMode: "user_byok",
          consumerDisplayName: "AI Providers",
          userFacingStatus: "Connected",
          disclosure: {
            showProviderIdentity: true,
            showModelIdentity: true,
            showCredentialSource: true,
            showProviderCatalog: true,
            allowModelSelection: true,
            allowProviderManagement: true,
            allowConnectionTest: true,
            allowDeveloperDetails: Boolean(mockDb.settings.developerMode),
          },
        } satisfies AiStatus as T;
      }
      if (configured) {
        const dev = Boolean(mockDb.settings.developerMode);
        return {
          provider: dev ? "gemini" : "",
          model: dev ? "gemini-3.5-flash" : "",
          keyDetected: true,
          status: "ready",
          message: null,
          source: "env",
          activeConnectionId: null,
          accessMode: "developer_environment",
          consumerDisplayName: "AI Access",
          userFacingStatus: "Ready",
          disclosure: {
            showProviderIdentity: dev,
            showModelIdentity: dev,
            showCredentialSource: dev,
            showProviderCatalog: dev,
            allowModelSelection: true,
            allowProviderManagement: true,
            allowConnectionTest: dev,
            allowDeveloperDetails: dev,
          },
        } satisfies AiStatus as T;
      }
      return {
        provider: "",
        model: "",
        keyDetected: false,
        status: "missing_key",
        message: "Connect your own AI provider or configure local AI in Settings.",
        source: "none",
        activeConnectionId: null,
        accessMode: "unavailable",
        consumerDisplayName: "AI Access",
        userFacingStatus: "Unavailable",
        disclosure: {
          showProviderIdentity: false,
          showModelIdentity: false,
          showCredentialSource: false,
          showProviderCatalog: false,
          allowModelSelection: false,
          allowProviderManagement: true,
          allowConnectionTest: false,
          allowDeveloperDetails: false,
        },
      } satisfies AiStatus as T;
    }

    case "list_provider_connections":
      return mockDb.providerConnections as T;

    case "provider_key_hints":
      return [
        {
          id: "gemini",
          label: "Google Gemini",
          keyPlaceholder: "Usually starts with AIza…",
          defaultModel: "gemini-3.5-flash",
          docsUrl: "https://aistudio.google.com/apikey",
          supportsBaseUrl: false,
        },
        {
          id: "openai",
          label: "OpenAI",
          keyPlaceholder: "Usually starts with sk-… or sk-proj-…",
          defaultModel: "gpt-4.1-mini",
          docsUrl: "https://platform.openai.com/api-keys",
          supportsBaseUrl: false,
        },
        {
          id: "anthropic",
          label: "Anthropic",
          keyPlaceholder: "Usually starts with sk-ant-…",
          defaultModel: "claude-sonnet-4-5",
          docsUrl: "https://console.anthropic.com/settings/keys",
          supportsBaseUrl: false,
        },
        {
          id: "openrouter",
          label: "OpenRouter",
          keyPlaceholder: "Usually starts with sk-or-v1-…",
          defaultModel: "google/gemini-2.5-flash",
          docsUrl: "https://openrouter.ai/keys",
          supportsBaseUrl: false,
        },
        {
          id: "compatible",
          label: "Custom OpenAI-compatible",
          keyPlaceholder: "Enter the API key required by this endpoint",
          defaultModel: "gpt-4.1-mini",
          docsUrl: null,
          supportsBaseUrl: true,
        },
      ] satisfies ProviderHint[] as T;

    case "store_hosted_auth_session":
    case "clear_hosted_auth_session":
    case "get_hosted_auth_status":
      return {
        signedIn: false,
        adapterReady: false,
        userId: null,
        expiresAt: null,
        configured: false,
      } as T;

    case "upsert_provider_connection": {
      const input = (args?.input ?? args) as UpsertProviderConnectionInput;
      const id = input.id?.trim() || `conn-${Date.now()}`;
      const existing = mockDb.providerConnections.find((c) => c.id === id);
      const stamp = now();
      const setActive = input.setActive ?? true;
      if (setActive) {
        mockDb.providerConnections = mockDb.providerConnections.map((c) => ({
          ...c,
          isActive: false,
        }));
      }
      const row: ProviderConnection = {
        id,
        provider: input.provider,
        label: input.label,
        baseUrl: input.baseUrl ?? null,
        modelDefault: input.modelDefault ?? null,
        isActive: setActive || existing?.isActive || false,
        hasKey: Boolean(input.apiKey?.trim()) || Boolean(existing?.hasKey),
        lastStatus: "connected",
        lastTestedAt: stamp,
        createdAt: existing?.createdAt ?? stamp,
        updatedAt: stamp,
      };
      mockDb.providerConnections = [
        row,
        ...mockDb.providerConnections.filter((c) => c.id !== id),
      ];
      mockDb.aiConfigured = true;
      return row as T;
    }

    case "delete_provider_connection": {
      const connectionId = String(args?.connectionId ?? "");
      mockDb.providerConnections = mockDb.providerConnections.filter(
        (c) => c.id !== connectionId,
      );
      mockDb.aiConfigured = mockDb.providerConnections.some((c) => c.hasKey);
      return undefined as T;
    }

    case "set_active_provider_connection": {
      const connectionId = String(args?.connectionId ?? "");
      mockDb.providerConnections = mockDb.providerConnections.map((c) => ({
        ...c,
        isActive: c.id === connectionId,
      }));
      const active = mockDb.providerConnections.find((c) => c.id === connectionId);
      if (!active) throw new Error("Provider connection not found");
      mockDb.aiConfigured = true;
      return active as T;
    }

    case "test_provider_connection":
      return { ok: true, message: "Mock connection ok" } as T;

    case "get_model_catalog":
      return {
        provider: "gemini",
        selected: mockDb.settings.preferredModel ?? "auto",
        autoResolvesTo: "gemini-3.5-flash",
        options: [
          {
            id: "auto",
            label: "Auto",
            description: "Uses the active provider default (gemini-3.5-flash)",
          },
          {
            id: "gemini-3.5-flash",
            label: "Gemini 3.5 Flash",
            description: "Recommended — fast agentic Flash",
          },
          {
            id: "gemini-3.1-flash-lite",
            label: "Gemini 3.1 Flash-Lite",
            description: "Lowest latency / cost",
          },
          {
            id: "gemini-2.5-flash",
            label: "Gemini 2.5 Flash",
            description: "Previous Flash generation",
          },
          {
            id: "gemini-2.5-pro",
            label: "Gemini 2.5 Pro",
            description: "Higher capability, slower",
          },
        ],
      } satisfies ModelCatalog as T;

    case "test_ai_connection":
      if (!mockDb.aiConfigured) {
        throw new TauriCommandError(
          "AI is not configured. Connect a provider with your own API key.",
          "missing_key",
        );
      }
      return { ok: true, message: "Connection successful" } as T;

    case "list_conversations":
      return [...mockDb.conversations].sort((a, b) =>
        b.updatedAt.localeCompare(a.updatedAt),
      ) as T;

    case "create_conversation": {
      const conversation: Conversation = {
        id: crypto.randomUUID(),
        title: "New chat",
        workspaceId: "ws-personal-default",
        projectId: null,
        pinned: false,
        archived: false,
        createdAt: now(),
        updatedAt: now(),
      };
      mockDb.conversations.unshift(conversation);
      mockDb.messages.set(conversation.id, []);
      return conversation as T;
    }

    case "list_projects_cmd": {
      const includeArchived = Boolean(args?.includeArchived);
      return mockDb.projects.filter(
        (p) => includeArchived || !p.archived,
      ) as T;
    }

    case "get_project_cmd": {
      const projectId = String(args?.projectId ?? "");
      const project = mockDb.projects.find((p) => p.id === projectId);
      if (!project) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      return project as T;
    }

    case "create_project_cmd": {
      const input = (args?.input ?? args) as CreateProjectInput;
      const stamp = now();
      const project: Project = {
        id: crypto.randomUUID(),
        name: input.name.trim(),
        description: input.description ?? null,
        iconKey: input.iconKey ?? "folder",
        instructions: input.instructions ?? null,
        summary: null,
        summaryUpdatedAt: null,
        archived: false,
        pinned: input.pinned ?? false,
        wallpaperJson: null,
        createdAt: stamp,
        updatedAt: stamp,
        lastOpenedAt: null,
      };
      mockDb.projects.unshift(project);
      return project as T;
    }

    case "update_project_cmd": {
      const projectId = String(args?.projectId ?? "");
      const input = (args?.input ?? {}) as UpdateProjectInput;
      const index = mockDb.projects.findIndex((p) => p.id === projectId);
      if (index < 0) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      const existing = mockDb.projects[index];
      const updated: Project = {
        ...existing,
        name: input.name?.trim() || existing.name,
        description:
          input.description !== undefined
            ? input.description
            : existing.description,
        iconKey:
          input.iconKey !== undefined ? input.iconKey : existing.iconKey,
        instructions:
          input.instructions !== undefined
            ? input.instructions
            : existing.instructions,
        pinned: input.pinned ?? existing.pinned,
        archived: input.archived ?? existing.archived,
        updatedAt: now(),
      };
      mockDb.projects[index] = updated;
      return updated as T;
    }

    case "archive_project_cmd":
    case "restore_project_cmd": {
      const projectId = String(args?.projectId ?? "");
      const index = mockDb.projects.findIndex((p) => p.id === projectId);
      if (index < 0) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      const archived = command === "archive_project_cmd";
      mockDb.projects[index] = {
        ...mockDb.projects[index],
        archived,
        updatedAt: now(),
      };
      return mockDb.projects[index] as T;
    }

    case "delete_project_cmd": {
      const input = (args?.input ?? args) as {
        projectId?: string;
        mode?: DeleteProjectMode;
      };
      const projectId = String(input.projectId ?? "");
      const mode = input.mode ?? "keepChats";
      mockDb.projects = mockDb.projects.filter((p) => p.id !== projectId);
      if (mode === "deleteChats") {
        const toDelete = mockDb.conversations
          .filter((c) => c.projectId === projectId)
          .map((c) => c.id);
        mockDb.conversations = mockDb.conversations.filter(
          (c) => c.projectId !== projectId,
        );
        for (const id of toDelete) mockDb.messages.delete(id);
      } else {
        mockDb.conversations = mockDb.conversations.map((c) =>
          c.projectId === projectId ? { ...c, projectId: null } : c,
        );
      }
      return undefined as T;
    }

    case "create_conversation_in_project": {
      const projectId = String(args?.projectId ?? "");
      const title =
        typeof args?.title === "string" && args.title.trim()
          ? args.title.trim()
          : "New chat";
      if (!mockDb.projects.some((p) => p.id === projectId)) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      const conversation: Conversation = {
        id: crypto.randomUUID(),
        title,
        workspaceId: "ws-personal-default",
        projectId,
        pinned: false,
        archived: false,
        createdAt: now(),
        updatedAt: now(),
      };
      mockDb.conversations.unshift(conversation);
      mockDb.messages.set(conversation.id, []);
      return conversation as T;
    }

    case "assign_chats_to_project": {
      const input = (args?.input ?? args) as {
        projectId?: string;
        conversationIds?: string[];
      };
      const projectId = String(input.projectId ?? "");
      const ids = input.conversationIds ?? [];
      if (!mockDb.projects.some((p) => p.id === projectId)) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      const updated: Conversation[] = [];
      mockDb.conversations = mockDb.conversations.map((c) => {
        if (!ids.includes(c.id)) return c;
        const next = { ...c, projectId, updatedAt: now() };
        updated.push(next);
        return next;
      });
      return updated as T;
    }

    case "assign_conversation_to_project_cmd": {
      const conversationId = String(args?.conversationId ?? "");
      const projectId = String(args?.projectId ?? "");
      const index = mockDb.conversations.findIndex((c) => c.id === conversationId);
      if (index < 0) {
        throw new TauriCommandError(
          `Conversation not found: ${conversationId}`,
          "not_found",
        );
      }
      mockDb.conversations[index] = {
        ...mockDb.conversations[index],
        projectId,
        updatedAt: now(),
      };
      return mockDb.conversations[index] as T;
    }

    case "remove_chat_from_project": {
      const conversationId = String(args?.conversationId ?? "");
      const index = mockDb.conversations.findIndex((c) => c.id === conversationId);
      if (index < 0) {
        throw new TauriCommandError(
          `Conversation not found: ${conversationId}`,
          "not_found",
        );
      }
      mockDb.conversations[index] = {
        ...mockDb.conversations[index],
        projectId: null,
        updatedAt: now(),
      };
      return mockDb.conversations[index] as T;
    }

    case "list_project_conversations_cmd": {
      const projectId = String(args?.projectId ?? "");
      return mockDb.conversations
        .filter((c) => c.projectId === projectId)
        .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)) as T;
    }

    case "list_unassigned_conversations_cmd":
      return mockDb.conversations
        .filter((c) => !c.projectId)
        .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)) as T;

    case "search_project_context_cmd":
      return [] as ProjectContextHit[] as T;

    case "refresh_project_summary":
      return "Project summary refreshed (mock)" as T;

    case "rebuild_project_index_cmd":
      return 0 as T;

    case "rename_conversation_cmd": {
      const conversationId = String(args?.conversationId ?? "");
      const title = String(args?.title ?? "").trim();
      const index = mockDb.conversations.findIndex((c) => c.id === conversationId);
      if (index < 0) {
        throw new TauriCommandError(
          `Conversation not found: ${conversationId}`,
          "not_found",
        );
      }
      mockDb.conversations[index] = {
        ...mockDb.conversations[index],
        title: title || mockDb.conversations[index].title,
        updatedAt: now(),
      };
      return mockDb.conversations[index] as T;
    }

    case "duplicate_conversation_cmd": {
      const conversationId = String(args?.conversationId ?? "");
      const source = mockDb.conversations.find((c) => c.id === conversationId);
      if (!source) {
        throw new TauriCommandError(
          `Conversation not found: ${conversationId}`,
          "not_found",
        );
      }
      const title =
        typeof args?.title === "string" && args.title.trim()
          ? args.title.trim()
          : `${source.title} (copy)`;
      const copy: Conversation = {
        ...source,
        id: crypto.randomUUID(),
        title,
        createdAt: now(),
        updatedAt: now(),
      };
      mockDb.conversations.unshift(copy);
      const msgs = mockDb.messages.get(conversationId) ?? [];
      mockDb.messages.set(
        copy.id,
        msgs.map((m) => ({
          ...m,
          id: crypto.randomUUID(),
          conversationId: copy.id,
          createdAt: now(),
        })),
      );
      return copy as T;
    }

    case "export_project":
      return {
        ok: true,
        path: String(
          (args as { input?: { destinationPath?: string } })?.input
            ?.destinationPath ?? "/tmp/project.json",
        ),
        message: "Project exported (mock)",
      } as T;

    case "list_media_assets_cmd": {
      const projectId = args?.projectId as string | undefined;
      const limit = Number(args?.limit ?? 50);
      const rows = projectId
        ? mockDb.mediaAssets.filter((a) => a.projectId === projectId)
        : mockDb.mediaAssets;
      return rows.slice(0, limit) as T;
    }

    case "get_media_asset_cmd": {
      const assetId = String(args?.assetId ?? "");
      const asset = mockDb.mediaAssets.find((a) => a.id === assetId);
      if (!asset) {
        throw new TauriCommandError(`Media asset not found: ${assetId}`, "not_found");
      }
      return asset as T;
    }

    case "get_media_asset_src_cmd": {
      const assetId = String(args?.assetId ?? "");
      const asset = mockDb.mediaAssets.find((a) => a.id === assetId);
      if (!asset) {
        throw new TauriCommandError(`Media asset not found: ${assetId}`, "not_found");
      }
      return {
        absolutePath: `/tmp/coreside-media/${asset.localFilename}`,
        mimeType: asset.mimeType,
      } satisfies MediaAssetSrc as T;
    }

    case "get_media_asset_thumb_src_cmd": {
      const assetId = String(args?.assetId ?? "");
      const asset = mockDb.mediaAssets.find((a) => a.id === assetId);
      if (!asset?.thumbnailFilename) {
        return null as T;
      }
      return {
        absolutePath: `/tmp/coreside-media/${asset.thumbnailFilename}`,
        mimeType: "image/jpeg",
      } satisfies MediaAssetSrc as T;
    }

    case "media_asset_usage_cmd": {
      const assetId = String(args?.assetId ?? "");
      const usages: MediaAssetUsage[] = [];
      for (const project of mockDb.projects) {
        if (!project.wallpaperJson?.includes(assetId)) continue;
        usages.push({ projectId: project.id, projectName: project.name });
      }
      return usages as T;
    }

    case "delete_media_asset_cmd": {
      const assetId = String(args?.assetId ?? "");
      const before = mockDb.mediaAssets.length;
      mockDb.mediaAssets = mockDb.mediaAssets.filter((a) => a.id !== assetId);
      if (mockDb.mediaAssets.length === before) {
        throw new TauriCommandError(`Media asset not found: ${assetId}`, "not_found");
      }
      return undefined as T;
    }

    case "import_media_asset_cmd": {
      const input = (args?.input ?? args) as ImportMediaInput;
      const stamp = now();
      const asset: MediaAsset = {
        id: crypto.randomUUID(),
        projectId: input.projectId ?? null,
        category: input.category ?? "image",
        title: input.title?.trim() || "Imported asset",
        localFilename: `${crypto.randomUUID()}.png`,
        mimeType: "image/png",
        byteSize: 1024,
        width: 800,
        height: 600,
        durationMs: null,
        contentHash: crypto.randomUUID(),
        sourceUrl: input.url,
        sourcePageUrl: input.sourcePageUrl ?? null,
        creator: input.creator ?? null,
        license: input.license ?? null,
        attribution: input.attribution ?? null,
        validationStatus: "ok",
        createdAt: stamp,
        lastUsedAt: null,
        thumbnailFilename: null,
      };
      mockDb.mediaAssets.unshift(asset);
      return asset as T;
    }

    case "touch_media_asset_cmd":
      return undefined as T;

    case "set_project_wallpaper_cmd": {
      const projectId = String(args?.projectId ?? "");
      const index = mockDb.projects.findIndex((p) => p.id === projectId);
      if (index < 0) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      mockDb.projects[index] = {
        ...mockDb.projects[index],
        wallpaperJson:
          typeof args?.wallpaperJson === "string"
            ? args.wallpaperJson
            : null,
        updatedAt: now(),
      };
      return mockDb.projects[index] as T;
    }

    case "touch_project_opened": {
      const projectId = String(args?.projectId ?? "");
      const index = mockDb.projects.findIndex((p) => p.id === projectId);
      if (index < 0) {
        throw new TauriCommandError(`Project not found: ${projectId}`, "not_found");
      }
      mockDb.projects[index] = {
        ...mockDb.projects[index],
        lastOpenedAt: now(),
        updatedAt: now(),
      };
      return mockDb.projects[index] as T;
    }

    case "delete_conversation": {
      const id = String(args?.conversationId ?? "");
      mockDb.conversations = mockDb.conversations.filter((c) => c.id !== id);
      mockDb.messages.delete(id);
      return undefined as T;
    }

    case "get_messages": {
      const id = String(args?.conversationId ?? "");
      return (mockDb.messages.get(id) ?? []) as T;
    }

    case "delete_messages_from": {
      const messageId = String(args?.messageId ?? "");
      for (const [conversationId, list] of mockDb.messages.entries()) {
        const index = list.findIndex((m) => m.id === messageId);
        if (index < 0) continue;
        const removed = list.length - index;
        mockDb.messages.set(conversationId, list.slice(0, index));
        return removed as T;
      }
      throw new TauriCommandError(`Message not found: ${messageId}`, "not_found");
    }

    case "send_message": {
      const conversationId = String(args?.conversationId ?? "");
      const content = String(args?.content ?? "");
      const mentions = Array.isArray(args?.mentions) ? args.mentions : [];
      const attachments = Array.isArray(args?.attachments) ? args.attachments : [];
      const messages = mockDb.messages.get(conversationId) ?? [];
      const userMessage: ChatMessage = {
        id: crypto.randomUUID(),
        conversationId,
        role: "user",
        content,
        createdAt: now(),
        status: "ok",
        metadata:
          mentions.length > 0 || attachments.length > 0
            ? {
                ...(mentions.length > 0
                  ? {
                      mentions: mentions.map(
                        (m: { toolId?: string; label?: string | null }) => ({
                          toolId: String(m?.toolId ?? ""),
                          label: m?.label ?? null,
                        }),
                      ),
                    }
                  : {}),
                ...(attachments.length > 0
                  ? { attachments }
                  : {}),
              }
            : null,
      };
      messages.push(userMessage);

      const conversation = mockDb.conversations.find((c) => c.id === conversationId);
      if (conversation && conversation.title === "New chat") {
        conversation.title = titleFromContent(content);
        conversation.updatedAt = now();
      }

      const assistantId = crypto.randomUUID();
      const lower = content.toLowerCase();
      let result: SendMessageResult;

      if (lower.includes("quiz")) {
        const tool: ToolDefinition = {
          id: "sample-quiz",
          name: "Sample Quiz",
          description: "A short quiz generated for preview.",
          layout: { type: "single-column" },
          version: 1,
          components: [
            {
              id: "quiz-root",
              type: "quiz",
              props: {
                questions: [
                  {
                    id: "q1",
                    prompt: "What is 2 + 2?",
                    choices: ["3", "4", "5"],
                    correctIndex: 1,
                    explanation: "Two plus two equals four.",
                  },
                  {
                    id: "q2",
                    prompt: "Which color is an emerald accent?",
                    choices: ["Blue", "Green", "Purple"],
                    correctIndex: 1,
                    explanation: "Emerald is a green.",
                  },
                ],
              },
              valueKey: "quiz",
            },
          ],
        };
        result = {
          messageId: assistantId,
          assistantMessage: "I created a sample quiz tool for you.",
          responseType: "tool_change",
          toolChange: {
            action: "create",
            targetToolId: null,
            tool,
            changeSummary: "Create sample quiz",
          },
        };
      } else if (lower.includes("counter") || lower.includes("water")) {
        const tool: ToolDefinition = {
          id: "water-tracker",
          name: "Water Tracker",
          description: "Tracks daily glasses of water.",
          layout: { type: "single-column" },
          version: 1,
          components: [
            {
              id: "card-main",
              type: "card",
              children: [
                {
                  id: "title",
                  type: "heading",
                  props: { text: "Water Tracker", level: 2 },
                },
                {
                  id: "water-count",
                  type: "counter",
                  valueKey: "count",
                  props: { label: "Glasses", min: 0 },
                },
                {
                  id: "actions",
                  type: "buttonGroup",
                  children: [
                    {
                      id: "inc",
                      type: "button",
                      props: { label: "Add glass", variant: "primary" },
                      actions: [{ type: "increment", target: "count", amount: 1 }],
                    },
                    {
                      id: "reset",
                      type: "button",
                      props: { label: "Reset", variant: "secondary" },
                      actions: [{ type: "reset", target: "count", value: 0 }],
                    },
                  ],
                },
              ],
            },
          ],
        };
        result = {
          messageId: assistantId,
          assistantMessage: "I created a simple water tracker for you.",
          responseType: "tool_change",
          toolChange: {
            action: "create",
            targetToolId: null,
            tool,
            changeSummary: "Create water tracker",
          },
        };
      } else if (
        lower.includes("theme") ||
        lower.includes("dark mode") ||
        lower.includes("light mode") ||
        lower.includes("accent") ||
        lower.includes("background") ||
        lower.includes("wallpaper") ||
        lower.includes("matrix")
      ) {
        const wantDark =
          lower.includes("dark") ||
          lower.includes("matrix") ||
          (!lower.includes("light") && !lower.includes("system"));
        const theme = wantDark
          ? "dark"
          : lower.includes("system")
            ? "system"
            : "light";
        mockDb.settings.theme = theme as ThemePreference;
        if (lower.includes("blue") || lower.includes("accent")) {
          mockDb.settings.accentPrimaryLight = "#2f6f8f";
          mockDb.settings.accentPrimaryDark = "#6ab0d4";
        }
        if (lower.includes("background") || lower.includes("canvas")) {
          mockDb.settings.backgroundLight = "#f0f4f8";
          mockDb.settings.backgroundDark = "#0f1418";
          mockDb.settings.surfaceLight = "#ffffff";
          mockDb.settings.surfaceDark = "#1a2228";
          mockDb.settings.surfaceMutedLight = "#e4ebf2";
          mockDb.settings.surfaceMutedDark = "#243038";
        }
        if (lower.includes("matrix") || lower.includes("wallpaper")) {
          mockDb.settings.backgroundLight = "#f5f6f1";
          mockDb.settings.backgroundDark = "#050805";
          mockDb.settings.surfaceLight = "#ffffff";
          mockDb.settings.surfaceDark = "#121812";
          mockDb.settings.wallpaper = {
            kind: "matrix",
            color: "#33ff66",
            speed: 1.15,
            density: 0.7,
            opacity: 0.42,
          };
        }
        result = {
          messageId: assistantId,
          assistantMessage: lower.includes("matrix")
            ? "Enabled a Matrix-style live wallpaper."
            : `Updated appearance to ${theme}.`,
          responseType: "settings_change",
          settingsChange: {
            theme,
            ...(lower.includes("blue") || lower.includes("accent")
              ? {
                  accentPrimary: {
                    light: "#2f6f8f",
                    dark: "#6ab0d4",
                  },
                }
              : {}),
            ...(lower.includes("background") || lower.includes("canvas")
              ? {
                  background: { light: "#f0f4f8", dark: "#0f1418" },
                  surface: { light: "#ffffff", dark: "#1a2228" },
                  surfaceMuted: { light: "#e4ebf2", dark: "#243038" },
                }
              : {}),
            ...(lower.includes("matrix") || lower.includes("wallpaper")
              ? {
                  background: { light: "#f5f6f1", dark: "#050805" },
                  surface: { light: "#ffffff", dark: "#121812" },
                  wallpaper: {
                    kind: "matrix" as const,
                    color: "#33ff66",
                    speed: 1.15,
                    density: 0.7,
                    opacity: 0.42,
                  },
                }
              : {}),
            changeSummary: lower.includes("matrix")
              ? "Matrix live wallpaper"
              : `Set theme to ${theme}`,
          },
        };
      } else {
        result = {
          messageId: assistantId,
          assistantMessage:
            "I'm here to help. Ask me to create a tool — for example, a water tracker or a quiz.",
          responseType: "message",
          toolChange: null,
        };
      }

      const assistantMessage: ChatMessage = {
        id: assistantId,
        conversationId,
        role: "assistant",
        content: result.assistantMessage,
        createdAt: now(),
        status: "ok",
        metadata: result.toolChange
          ? {
              toolChange: result.toolChange,
              pending: true,
              toolChangeStatus: "pending",
            }
          : null,
      };
      messages.push(assistantMessage);
      mockDb.messages.set(conversationId, messages);
      return result as T;
    }

    case "cancel_request":
      return undefined as T;

    case "discard_tool_change": {
      const messageId = String(args?.messageId ?? "");
      for (const [, messages] of mockDb.messages) {
        const msg = messages.find((m) => m.id === messageId);
        if (msg && msg.metadata) {
          msg.metadata = {
            ...msg.metadata,
            pending: false,
            toolChangeStatus: "discarded",
            discarded: true,
          };
          return msg as T;
        }
      }
      return undefined as T;
    }

    case "apply_tool_change": {
      const toolChange = args?.toolChange as ToolChange;
      const tool = toolChange.tool;
      if (
        isProtectedId(tool.id) ||
        (toolChange.targetToolId && isProtectedId(toolChange.targetToolId))
      ) {
        throw new TauriCommandError(
          `Protected core resource cannot be modified: ${tool.id}`,
          "forbidden",
        );
      }
      const existingIndex = mockDb.tools.findIndex((t) => t.id === tool.id);
      const nextVersion =
        existingIndex >= 0 ? (mockDb.tools[existingIndex].version ?? 1) + 1 : 1;
      const saved: ToolDefinition = { ...tool, version: nextVersion };
      if (existingIndex >= 0) mockDb.tools[existingIndex] = saved;
      else mockDb.tools.push(saved);

      const versions = mockDb.toolVersions.get(tool.id) ?? [];
      versions.push({
        id: crypto.randomUUID(),
        toolId: tool.id,
        version: nextVersion,
        changeSummary: toolChange.changeSummary ?? "",
        createdAt: now(),
        definition: saved,
      });
      mockDb.toolVersions.set(tool.id, versions);
      if (!mockDb.toolState.has(tool.id)) {
        mockDb.toolState.set(tool.id, {});
      }

      const messageId = String(args?.messageId ?? "");
      for (const [, messages] of mockDb.messages) {
        const msg = messages.find((m) => m.id === messageId);
        if (msg && msg.metadata) {
          msg.metadata = {
            ...msg.metadata,
            pending: false,
            toolChangeStatus: "applied",
            appliedToolId: tool.id,
          };
        }
      }

      return saved as T;
    }

    case "list_tools":
      return mockDb.tools.map((t) => ({
        id: t.id,
        name: t.name,
        description: t.description,
        version: t.version ?? 1,
        updatedAt: now(),
        createdAt: now(),
      })) satisfies ToolSummary[] as T;

    case "get_tool": {
      const id = String(args?.toolId ?? "");
      const tool = mockDb.tools.find((t) => t.id === id);
      if (!tool) throw new TauriCommandError(`Tool not found: ${id}`);
      return tool as T;
    }

    case "get_tool_versions": {
      const id = String(args?.toolId ?? "");
      return (mockDb.toolVersions.get(id) ?? []) as T;
    }

    case "undo_tool_change": {
      const id = String(args?.toolId ?? "");
      const versions = mockDb.toolVersions.get(id) ?? [];
      if (versions.length <= 1) {
        throw new TauriCommandError("Nothing to undo");
      }
      versions.pop();
      mockDb.toolVersions.set(id, versions);
      const previous = versions[versions.length - 1];
      const toolIndex = mockDb.tools.findIndex((t) => t.id === id);
      if (toolIndex < 0 || !previous?.definition) {
        throw new TauriCommandError(`Tool not found: ${id}`);
      }
      const restored: ToolDefinition = {
        ...previous.definition,
        id,
        version: previous.version,
      };
      mockDb.tools[toolIndex] = restored;
      return restored as T;
    }

    case "save_tool_state": {
      const id = String(args?.toolId ?? "");
      mockDb.toolState.set(id, (args?.state as ToolState) ?? {});
      return undefined as T;
    }

    case "get_tool_state": {
      const id = String(args?.toolId ?? "");
      return (mockDb.toolState.get(id) ?? {}) as T;
    }

    case "get_settings":
      return { ...mockDb.settings } as T;

    case "set_setting": {
      const key = String(args?.key ?? "");
      const value = args?.value;
      if (key === "theme" && typeof value === "string") {
        mockDb.settings.theme = value as ThemePreference;
      }
      if (key === "sidebarCollapsed" && typeof value === "boolean") {
        mockDb.settings.sidebarCollapsed = value;
      }
      if (key === "preferredModel" && typeof value === "string") {
        mockDb.settings.preferredModel = value.trim() || "auto";
      }
      if (key === "dockIcon" && typeof value === "string") {
        const next = value.trim().toLowerCase();
        mockDb.settings.dockIcon =
          next === "dark" || next === "light" ? next : "auto";
      }
      if (key === "accentPrimaryLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.accentPrimaryLight = hex;
      }
      if (key === "accentPrimaryDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.accentPrimaryDark = hex;
      }
      if (key === "accentSecondaryLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.accentSecondaryLight = hex;
      }
      if (key === "accentSecondaryDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.accentSecondaryDark = hex;
      }
      if (key === "backgroundLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.backgroundLight = hex;
      }
      if (key === "backgroundDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.backgroundDark = hex;
      }
      if (key === "surfaceLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.surfaceLight = hex;
      }
      if (key === "surfaceDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.surfaceDark = hex;
      }
      if (key === "surfaceMutedLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.surfaceMutedLight = hex;
      }
      if (key === "surfaceMutedDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.surfaceMutedDark = hex;
      }
      if (key === "borderLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.borderLight = hex;
      }
      if (key === "borderDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.borderDark = hex;
      }
      if (key === "textPrimaryLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.textPrimaryLight = hex;
      }
      if (key === "textPrimaryDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.textPrimaryDark = hex;
      }
      if (key === "textSecondaryLight" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.textSecondaryLight = hex;
      }
      if (key === "textSecondaryDark" && typeof value === "string") {
        const hex = normalizeMockHex(value);
        if (hex) mockDb.settings.textSecondaryDark = hex;
      }
      if (key === "wallpaper") {
        mockDb.settings.wallpaper = sanitizeMockWallpaper(value);
      }
      if (key === "wallpaperJson") {
        mockDb.settings.wallpaperJson =
          typeof value === "string" && value.trim() ? value : null;
      }
      if (key === "actionLogEnabled" && typeof value === "boolean") {
        mockDb.settings.actionLogEnabled = value;
        mockDb.settings.actionLogMode = value ? "always" : "off";
      }
      if (key === "actionLogMode" && typeof value === "string") {
        const mode =
          value === "always" || value === "intelligent" || value === "off"
            ? value
            : "off";
        mockDb.settings.actionLogMode = mode;
        mockDb.settings.actionLogEnabled = mode !== "off";
      }
      if (key === "safeSearch" && typeof value === "string") {
        const level = value.trim().toLowerCase();
        if (level === "strict" || level === "standard" || level === "off") {
          mockDb.settings.safeSearch = level;
        }
      }
      if (key === "developerMode" && typeof value === "boolean") {
        mockDb.settings.developerMode = value;
      }
      return { ...mockDb.settings } as T;
    }

    case "list_added_settings": {
      const owner = args?.ownerToolId;
      const all = mockDb.addedSettings;
      if (typeof owner === "string" && owner) {
        return all.filter((s) => s.ownerToolId === owner) as T;
      }
      return [...all] as T;
    }

    case "upsert_added_setting": {
      const input = (args?.input ?? args) as UpsertAddedSettingInput;
      if (!input?.id || isProtectedId(input.id)) {
        throw new TauriCommandError(
          `Added Setting ids cannot start with 'core.': ${input?.id ?? ""}`,
          "invalid",
        );
      }
      const existing = mockDb.addedSettings.findIndex((s) => s.id === input.id);
      const record: AddedSetting = {
        id: input.id,
        ownerToolId: input.ownerToolId ?? null,
        label: input.label,
        description: input.description ?? "",
        settingType: input.settingType,
        defaultValue: input.defaultValue ?? null,
        currentValue: input.currentValue ?? input.defaultValue ?? null,
        constraints: input.constraints ?? {},
        version: existing >= 0 ? mockDb.addedSettings[existing].version + 1 : 1,
        createdAt:
          existing >= 0 ? mockDb.addedSettings[existing].createdAt : now(),
        updatedAt: now(),
      };
      if (existing >= 0) mockDb.addedSettings[existing] = record;
      else mockDb.addedSettings.push(record);
      return record as T;
    }

    case "delete_added_setting": {
      const id = String(args?.id ?? "");
      if (isProtectedId(id)) {
        throw new TauriCommandError(
          `Protected core resource cannot be modified: ${id}`,
          "invalid",
        );
      }
      const before = mockDb.addedSettings.length;
      mockDb.addedSettings = mockDb.addedSettings.filter((s) => s.id !== id);
      if (mockDb.addedSettings.length === before) {
        throw new TauriCommandError(`Added setting not found: ${id}`, "not_found");
      }
      return undefined as T;
    }

    case "validate_change_targets": {
      const ids = (args?.ids as string[] | undefined) ?? [];
      const rejected = ids.filter(isProtectedId);
      return {
        ok: rejected.length === 0,
        rejected,
      } satisfies ValidateChangeTargetsResult as T;
    }

    case "set_dock_icon":
    case "set_dock_icon_for_os_appearance":
      return undefined as T;

    case "clear_conversations":
      mockDb.conversations = [];
      mockDb.messages.clear();
      return undefined as T;

    case "clear_tools":
      mockDb.tools = [];
      mockDb.toolState.clear();
      mockDb.toolVersions.clear();
      return undefined as T;

    case "open_tool_window":
      if (typeof window !== "undefined") {
        const toolId = String(args?.toolId ?? "");
        window.open(`/#/tool/${toolId}`, `coreside-tool-${toolId}`);
      }
      return undefined as T;

    case "list_automations":
      return [] as T;
    case "delete_automation":
      return undefined as T;
    case "list_automation_runs":
    case "list_workspace_backgrounds":
      return [] as T;
    case "run_automation_now":
      return "Automation run completed" as T;
    case "upsert_automation":
    case "set_automation_enabled":
      return {
        id: "auto-mock",
        name: "Mock automation",
        enabled: true,
        requiresAi: false,
        consecutiveFailures: 0,
        trigger: { type: "interval", intervalMinutes: 300 },
      } as T;
    case "list_export_formats":
      return [
        {
          id: "coreside-tool",
          label: "Coreside Tool Package",
          extension: "coreside-tool.json",
          available: true,
          description: "Definition and safe state",
        },
        {
          id: "html",
          label: "Standalone HTML",
          extension: "html",
          available: true,
          description: "Offline HTML",
        },
        {
          id: "png",
          label: "PNG Snapshot",
          extension: "png",
          available: true,
          description: "Image snapshot",
        },
      ] as T;
    case "export_tool":
      return {
        ok: true,
        path: String((args as { destinationPath?: string })?.destinationPath ?? "/tmp/export"),
        format: String((args as { format?: string })?.format ?? "json"),
        message: "Export saved",
      } as T;

    case "get_search_connection":
      return {
        provider: "hybrid",
        hasKey: false,
        source: "none",
        engineReady: false,
        engineReason:
          "Local research engine needs setup. Run npm run crawl4ai:setup.",
        exaConfigured: false,
        exaSource: "none",
      } as T;
    case "configure_search_connection":
      return {
        provider: "hybrid",
        hasKey: false,
        source: "none",
        engineReady: false,
        engineReason:
          "Local research engine needs setup. Run npm run crawl4ai:setup.",
        exaConfigured: false,
        exaSource: "none",
      } as T;
    case "get_exa_connection":
      return { configured: false, source: "none", account: "coreside.search.exa" } as T;
    case "configure_exa_connection":
      return {
        configured: true,
        source: "keyring",
        account: "coreside.search.exa",
      } as T;
    case "delete_exa_connection":
      return undefined as T;
    case "test_exa_connection":
      throw new TauriCommandError("Exa API key not configured", "not_configured");
    case "get_exa_usage":
      return {
        monthKey: "2026-07",
        requestCount: 0,
        resultCount: 0,
        actualCostTotal: 0,
        estimatedCostTotal: 0,
        cacheHits: 0,
      } as T;
    case "list_exa_usage":
      return [] as T;
    case "get_exa_budget":
      return {
        monthKey: "2026-07",
        spentUsd: 0,
        budgetUsd: null,
        remainingUsd: null,
        percentUsed: null,
        threshold: "ok",
        note: "Local budget is optional and separate from Exa account credits.",
      } as T;
    case "set_exa_budget": {
      const monthly = (args as { input?: { monthlyBudgetUsd?: number | null } })
        ?.input?.monthlyBudgetUsd;
      return {
        monthKey: "2026-07",
        spentUsd: 0,
        budgetUsd: monthly ?? null,
        remainingUsd: monthly ?? null,
        percentUsed: monthly != null ? 0 : null,
        threshold: "ok",
        note: "Local budget is optional and separate from Exa account credits.",
      } as T;
    }
    case "get_search_profile":
      return {
        profile: "saver",
        searchType: "fast",
        numResults: 3,
        maxCrawlPages: 2,
        maxRefinements: 0,
      } as T;
    case "set_search_profile":
      return {
        profile: String((args as { input?: { profile?: string } })?.input?.profile ?? "saver"),
        searchType: "fast",
        numResults: 3,
        maxCrawlPages: 2,
        maxRefinements: 0,
      } as T;
    case "delete_search_connection":
      return undefined as T;
    case "test_search_connection":
      throw new TauriCommandError(
        "Local research engine needs setup",
        "needs_setup",
      );
    case "get_crawler_status":
      return {
        installation: "needsSetup",
        running: false,
        pythonPath: null,
        reason: "Local research engine needs setup.",
        resourceProfile: "balanced",
        sidecarVersion: null,
        crawl4aiVersion: null,
      } as T;
    case "get_crawler_installation":
      return {
        state: "needsSetup",
        pythonPath: null,
        serviceRoot: "services/crawl4ai",
        crawl4aiImportOk: false,
        reason: "Crawl4AI Python venv not found. Run npm run crawl4ai:setup.",
      } as T;
    case "cleanup_crawler_cache":
      return { ok: true, deletedBytes: 0 } as T;
    case "get_crawler_cache_stats":
      return {
        root: null,
        sizeBytes: 0,
        quotaBytes: 1_000_000_000,
        usageRatio: 0,
        overQuota: false,
      } as T;
    case "set_web_research_resource_profile": {
      const input = (args?.input ?? args) as { profile?: string };
      const profile = String(input.profile ?? "balanced").toLowerCase();
      if (profile === "eco" || profile === "performance") return profile as T;
      return "balanced" as T;
    }

    case "stage_chat_attachment": {
      const input = (args?.input ?? args) as {
        name?: string;
        mimeType?: string;
        dataBase64?: string;
      };
      return {
        id: crypto.randomUUID(),
        name: String(input.name ?? "file"),
        mimeType: String(input.mimeType ?? "application/octet-stream"),
        byteSize: Math.floor((String(input.dataBase64 ?? "").length * 3) / 4),
        localFilename: `${crypto.randomUUID()}_${String(input.name ?? "file")}`,
      } as T;
    }

    case "get_chat_attachment_src":
      return `/tmp/coreside-attachments/${String(args?.localFilename ?? "file")}` as T;

    case "list_search_sessions_cmd": {
      const input = (args?.input ?? {}) as {
        conversationId?: string | null;
        projectId?: string | null;
        searchType?: string | null;
        limit?: number;
      };
      let rows = [...mockDb.searchSessions];
      if (input.conversationId) {
        rows = rows.filter((s) => s.conversationId === input.conversationId);
      }
      if (input.projectId) {
        rows = rows.filter((s) => s.projectId === input.projectId);
      }
      if (input.searchType) {
        rows = rows.filter((s) => s.searchType === input.searchType);
      }
      const limit = input.limit ?? 50;
      return rows.slice(0, limit).map((s) => ({
        id: s.id,
        conversationId: s.conversationId,
        projectId: s.projectId,
        searchType: s.searchType,
        query: s.query,
        provider: s.provider,
        createdAt: s.createdAt,
        resultCount: s.results.length,
      })) as T;
    }

    case "get_search_session_cmd": {
      const sessionId = String(args?.sessionId ?? "");
      const session = mockDb.searchSessions.find((s) => s.id === sessionId);
      if (!session) {
        throw new TauriCommandError(`Search session not found: ${sessionId}`, "not_found");
      }
      return {
        session: {
          id: session.id,
          conversationId: session.conversationId,
          projectId: session.projectId,
          searchType: session.searchType,
          query: session.query,
          provider: session.provider,
          createdAt: session.createdAt,
          resultCount: session.results.length,
        },
        results: session.results,
      } as T;
    }

    case "clear_search_history_cmd": {
      const input = (args?.input ?? {}) as {
        conversationId?: string | null;
        projectId?: string | null;
      };
      const before = mockDb.searchSessions.length;
      const cid = input.conversationId ?? null;
      const pid = input.projectId ?? null;
      if (!cid && !pid) {
        mockDb.searchSessions = [];
      } else {
        mockDb.searchSessions = mockDb.searchSessions.filter((s) => {
          if (cid && pid) {
            return !(s.conversationId === cid && s.projectId === pid);
          }
          if (cid) return s.conversationId !== cid;
          return s.projectId !== pid;
        });
      }
      return (before - mockDb.searchSessions.length) as T;
    }

    case "get_surface_state_cmd": {
      const surfaceId = String(args?.surfaceId ?? "");
      return (mockDb.surfaceState.get(surfaceId) ?? {}) as T;
    }

    case "save_surface_state_cmd": {
      const surfaceId = String(args?.surfaceId ?? "");
      mockDb.surfaceState.set(
        surfaceId,
        (args?.stateJson as Record<string, unknown>) ?? {},
      );
      return undefined as T;
    }

    case "get_draft_cmd": {
      const surfaceId = String(args?.surfaceId ?? "");
      const componentId = String(args?.componentId ?? "");
      const windowId = String(args?.windowId ?? "main");
      const key = `${surfaceId}:${componentId}:${windowId}`;
      return (mockDb.surfaceDrafts.get(key) ?? null) as T;
    }

    case "save_draft_cmd": {
      const input = (args?.args ?? args) as {
        surfaceId?: string;
        componentId?: string;
        windowId?: string;
        baseRevision?: number;
        draft?: unknown;
        formId?: string | null;
        persistencePolicy?: string;
        force?: boolean;
      };
      const surfaceId = String(input.surfaceId ?? "");
      const componentId = String(input.componentId ?? "");
      const windowId = String(input.windowId ?? "main");
      const baseRevision = Number(input.baseRevision ?? 0);
      const key = `${surfaceId}:${componentId}:${windowId}`;
      const existing = mockDb.surfaceDrafts.get(key);
      if (existing && !input.force && existing.baseRevision >= baseRevision) {
        throw new TauriCommandError(
          `draft revision conflict: stored ${existing.baseRevision}, requested ${baseRevision}`,
          "draft_conflict",
        );
      }
      const stamp = now();
      const row: import("@/types/runtime-v2").SurfaceDraft = {
        id: existing?.id ?? crypto.randomUUID(),
        surfaceId,
        componentId,
        formId: input.formId ?? null,
        windowId,
        baseRevision,
        draft: input.draft ?? {},
        persistencePolicy: input.persistencePolicy ?? "session",
        updatedAt: stamp,
        createdAt: existing?.createdAt ?? stamp,
      };
      mockDb.surfaceDrafts.set(key, row);
      return row as T;
    }

    case "schedule_patches_cmd":
    case "flush_patch_scheduler_cmd":
      return [] as T;

    case "get_route_state_cmd": {
      const applicationId = String(args?.applicationId ?? "");
      const windowId = String(args?.windowId ?? "main");
      const key = `${applicationId}:${windowId}`;
      const existing = mockDb.routeState.get(key);
      if (!existing) {
        throw new TauriCommandError(
          `route ${applicationId}/${windowId} not found`,
          "not_found",
        );
      }
      return existing as T;
    }

    case "set_route_state_cmd": {
      const input = (args?.args ?? args) as {
        applicationId?: string;
        windowId?: string;
        currentRouteId?: string | null;
        routeParams?: Record<string, unknown>;
        history?: unknown[];
        historyIndex?: number;
      };
      const applicationId = String(input.applicationId ?? "");
      const windowId = String(input.windowId ?? "main");
      const key = `${applicationId}:${windowId}`;
      const stamp = now();
      const row: import("@/types/runtime-v2").RouteState = {
        id: mockDb.routeState.get(key)?.id ?? crypto.randomUUID(),
        applicationId,
        windowId,
        currentRouteId: input.currentRouteId ?? null,
        routeParams: input.routeParams ?? {},
        history: input.history ?? [],
        historyIndex: Number(input.historyIndex ?? 0),
        updatedAt: stamp,
      };
      mockDb.routeState.set(key, row);
      return row as T;
    }

    case "navigate_route_cmd": {
      const input = (args?.args ?? args) as {
        applicationId?: string;
        windowId?: string;
        routeId?: string;
        routeParams?: Record<string, unknown>;
        pushHistory?: boolean;
      };
      const applicationId = String(input.applicationId ?? "");
      const windowId = String(input.windowId ?? "main");
      const routeId = String(input.routeId ?? "");
      const routeParams = input.routeParams ?? {};
      const key = `${applicationId}:${windowId}`;
      const existing = mockDb.routeState.get(key);
      if (
        existing &&
        existing.currentRouteId === routeId &&
        JSON.stringify(existing.routeParams) === JSON.stringify(routeParams)
      ) {
        return { state: existing, changed: false } as T;
      }
      const history = existing?.history ? [...existing.history] : [];
      let historyIndex = existing?.historyIndex ?? 0;
      if (input.pushHistory !== false) {
        history.push({ routeId, params: routeParams });
        historyIndex = history.length - 1;
      }
      const stamp = now();
      const state: import("@/types/runtime-v2").RouteState = {
        id: existing?.id ?? crypto.randomUUID(),
        applicationId,
        windowId,
        currentRouteId: routeId,
        routeParams,
        history,
        historyIndex,
        updatedAt: stamp,
      };
      mockDb.routeState.set(key, state);
      return { state, changed: true } as T;
    }

    case "append_context_ledger_cmd": {
      const input = (args?.args ?? args) as {
        conversationId?: string;
        projectId?: string | null;
        branchId?: string | null;
        entryType?: string;
        visibility?: string;
        payload?: unknown;
        summary?: string;
        expirationClass?: string | null;
      };
      const stamp = now();
      const row: import("@/types/runtime-v2").ContextLedgerEntry = {
        id: crypto.randomUUID(),
        conversationId: String(input.conversationId ?? ""),
        projectId: input.projectId ?? null,
        branchId: input.branchId ?? null,
        entryType: String(input.entryType ?? "interaction"),
        visibility: String(input.visibility ?? "model_context_only"),
        payload: input.payload ?? {},
        summary: String(input.summary ?? ""),
        expirationClass: String(input.expirationClass ?? "session"),
        createdAt: stamp,
      };
      mockDb.contextLedger.unshift(row);
      return row as T;
    }

    case "list_context_ledger_cmd": {
      const conversationId = String(args?.conversationId ?? "");
      const limit = Number(args?.limit ?? 50);
      return mockDb.contextLedger
        .filter((e) => e.conversationId === conversationId)
        .slice(0, limit) as T;
    }

    case "get_provider_profile_cmd": {
      const providerId = String(args?.providerId ?? "gemini");
      const modelId = String(args?.modelId ?? "*");
      const stamp = now();
      return {
        id: crypto.randomUUID(),
        providerId,
        modelId,
        profile: "native_streaming_operations",
        capabilities: { supportsApplicationChanges: true },
        lastTestedAt: null,
        benchmark: null,
        createdAt: stamp,
        updatedAt: stamp,
      } as T;
    }

    case "get_continuity_cmd": {
      const surfaceId = String(args?.surfaceId ?? "");
      const windowId = String(args?.windowId ?? "main");
      const key = `${surfaceId}:${windowId}`;
      const existing = mockDb.continuity.get(key);
      if (!existing) {
        throw new TauriCommandError(
          `continuity ${surfaceId}/${windowId} not found`,
          "not_found",
        );
      }
      return existing as T;
    }

    case "save_continuity_cmd":
    case "suspend_surface_cmd": {
      const input =
        command === "suspend_surface_cmd"
          ? {
              surfaceId: String(args?.surfaceId ?? ""),
              windowId: String(args?.windowId ?? "main"),
              focus: {},
              scroll: {},
              media: {},
              suspensionState: "suspended",
            }
          : ((args?.args ?? args) as {
              surfaceId?: string;
              windowId?: string;
              focus?: Record<string, unknown>;
              scroll?: Record<string, unknown>;
              media?: Record<string, unknown>;
              suspensionState?: string;
            });
      const surfaceId = String(input.surfaceId ?? "");
      const windowId = String(input.windowId ?? "main");
      const key = `${surfaceId}:${windowId}`;
      const stamp = now();
      const row: import("@/types/runtime-v2").ContinuitySnapshot = {
        id: mockDb.continuity.get(key)?.id ?? crypto.randomUUID(),
        surfaceId,
        windowId,
        focus: input.focus ?? {},
        scroll: input.scroll ?? {},
        media: input.media ?? {},
        suspensionState:
          input.suspensionState === "background"
            ? "background"
            : input.suspensionState === "suspended"
              ? "suspended"
              : "active",
        updatedAt: stamp,
      };
      mockDb.continuity.set(key, row);
      return row as T;
    }

    case "kernel_list_registered_actions":
      return { actions: [] } as T;

    case "kernel_invoke_registered_action": {
      const request = (args?.request ?? args) as ClientActionRequest;
      return {
        status: "ok",
        data: { action: request.actionName },
      } satisfies ActionOutcome as T;
    }

    case "kernel_list_pending_approvals":
      return mockDb.kernelPendingApprovals.filter(
        (a) => a.status === "pending",
      ) as T;

    case "kernel_decide_approval": {
      const approvalId = String(args?.approvalId ?? "");
      const approve = Boolean(args?.approve);
      const index = mockDb.kernelPendingApprovals.findIndex(
        (a) => a.id === approvalId,
      );
      if (index < 0) {
        throw new TauriCommandError("Approval not found", "not_found");
      }
      const approval = {
        ...mockDb.kernelPendingApprovals[index],
        status: approve ? "approved" : "denied",
        decidedAt: now(),
      };
      mockDb.kernelPendingApprovals[index] = approval;
      return { approval, grant: null } satisfies ApprovalDecisionResult as T;
    }

    case "kernel_list_runtime_grants": {
      const applicationId = args?.applicationId as string | undefined;
      return mockDb.kernelRuntimeGrants.filter(
        (g) =>
          g.status === "active" &&
          (!applicationId || g.applicationId === applicationId),
      ) as T;
    }

    case "kernel_revoke_runtime_grant": {
      const grantId = String(args?.grantId ?? "");
      mockDb.kernelRuntimeGrants = mockDb.kernelRuntimeGrants.map((g) =>
        g.id === grantId ? { ...g, status: "revoked", revokedAt: now() } : g,
      );
      return undefined as T;
    }

    case "kernel_list_audit_events":
      return mockDb.kernelAuditEvents.slice(0, Number(args?.limit ?? 100)) as T;

    case "kernel_clear_audit_events":
      mockDb.kernelAuditEvents = [];
      return 0 as T;

    case "kernel_set_application_lifecycle": {
      const applicationId = String(args?.applicationId ?? "");
      const enabled = Boolean(args?.enabled);
      const record = mockDb.kernelManifests.find(
        (m) => m.applicationId === applicationId,
      );
      if (!record) {
        throw new TauriCommandError("Application not found", "not_found");
      }
      const updated: ManifestRecord = {
        ...record,
        disabled: !enabled,
        lifecycleState: enabled ? "active" : "disabled",
        updatedAt: now(),
      };
      mockDb.kernelManifests = mockDb.kernelManifests.map((m) =>
        m.applicationId === applicationId ? updated : m,
      );
      return updated as T;
    }

    case "kernel_record_build_failure": {
      const applicationId = String(args?.applicationId ?? "");
      const message = String(args?.message ?? "Build failed");
      const row: BuildFailure = {
        id: crypto.randomUUID(),
        applicationId,
        safeMessage: message,
        retryable: args?.retryable !== false,
        requestRef:
          typeof args?.requestRef === "string" ? args.requestRef : null,
        createdAt: now(),
      };
      mockDb.kernelBuildFailures.unshift(row);
      return row as T;
    }

    case "kernel_clear_build_failure": {
      const applicationId = String(args?.applicationId ?? "");
      const before = mockDb.kernelBuildFailures.length;
      mockDb.kernelBuildFailures = mockDb.kernelBuildFailures.filter(
        (f) => f.applicationId !== applicationId,
      );
      return (before - mockDb.kernelBuildFailures.length) as T;
    }

    case "kernel_list_build_failures": {
      const applicationId = String(args?.applicationId ?? "");
      return mockDb.kernelBuildFailures.filter(
        (f) => f.applicationId === applicationId,
      ) as T;
    }

    case "kernel_list_application_versions":
      return [] as ApplicationVersion[] as T;

    case "kernel_list_manifests":
      return mockDb.kernelManifests as T;

    case "kernel_get_manifest": {
      const applicationId = String(args?.applicationId ?? "");
      const record = mockDb.kernelManifests.find(
        (m) => m.applicationId === applicationId,
      );
      if (!record) {
        throw new TauriCommandError("Application not found", "not_found");
      }
      return record as T;
    }

    case "kernel_get_recovery_state":
      return {
        recoveryMode: false,
        disableUserSurfaces: false,
        disableCustomLayouts: false,
        disableCapabilityPacks: false,
        uncleanShutdown: false,
        lastFailure: null,
        updatedAt: now(),
      } satisfies RecoveryState as T;

    case "kernel_restore_last_known_good": {
      const applicationId = String(args?.applicationId ?? "");
      const record = mockDb.kernelManifests.find(
        (m) => m.applicationId === applicationId,
      );
      if (!record) {
        throw new TauriCommandError("Application not found", "not_found");
      }
      const updated: ManifestRecord = {
        ...record,
        disabled: false,
        lifecycleState: "restored",
        healthState: "healthy",
        updatedAt: now(),
      };
      mockDb.kernelManifests = mockDb.kernelManifests.map((m) =>
        m.applicationId === applicationId ? updated : m,
      );
      return updated as T;
    }

    case "kernel_application_summary": {
      const applicationId = String(args?.applicationId ?? "");
      const record = mockDb.kernelManifests.find(
        (m) => m.applicationId === applicationId,
      );
      if (!record) {
        throw new TauriCommandError("Application not found", "not_found");
      }
      return {
        applicationId,
        name: record.manifest.name,
        version: record.currentVersion,
        lastKnownGood: record.lastKnownGoodVersion,
        health: record.healthState,
        lifecycle: record.lifecycleState,
        permissions: record.manifest.permissions ?? [],
        recordCount: 0,
      } as T;
    }

    case "kernel_set_recovery_mode":
    case "kernel_set_recovery_flags":
    case "kernel_clear_recovery":
    case "kernel_enter_safe_startup":
      return {
        recoveryMode: false,
        disableUserSurfaces: false,
        disableCustomLayouts: false,
        disableCapabilityPacks: false,
        uncleanShutdown: false,
        lastFailure: null,
        updatedAt: now(),
      } satisfies RecoveryState as T;

    default:
      throw new TauriCommandError(`Unknown command: ${command}`);
  }
}

/** Test helper to seed mock AI configuration. */
export function __setMockAiConfigured(configured: boolean): void {
  mockDb.aiConfigured = configured;
}

/** Test helper to reset mock database. */
export function __resetMockDb(): void {
  mockDb.conversations = [];
  mockDb.projects = [];
  mockDb.messages.clear();
  mockDb.tools = [];
  mockDb.toolState.clear();
  mockDb.toolVersions.clear();
  mockDb.addedSettings = [];
  mockDb.settings = {
    theme: "system",
    sidebarCollapsed: false,
    preferredModel: "auto",
    dockIcon: "auto",
    accentPrimaryLight: "#2f8f63",
    accentPrimaryDark: "#69c994",
    accentSecondaryLight: "#d38b3d",
    accentSecondaryDark: "#e0a158",
    backgroundLight: "#f5f6f1",
    backgroundDark: "#141714",
    surfaceLight: "#ffffff",
    surfaceDark: "#1c201c",
    surfaceMutedLight: "#ecefe8",
    surfaceMutedDark: "#242a24",
    borderLight: "#d8d8d4",
    borderDark: "#3a3a3a",
    textPrimaryLight: "#1d211c",
    textPrimaryDark: "#eef2ec",
    textSecondaryLight: "#687066",
    textSecondaryDark: "#a6afa3",
    wallpaper: { ...DEFAULT_WALLPAPER },
    wallpaperJson: null,
    actionLogEnabled: false,
    actionLogMode: "off",
    safeSearch: "standard" as SafeSearchLevel,
    developerMode: false,
  };
  mockDb.aiConfigured = false;
  mockDb.providerConnections = [];
  mockDb.mediaAssets = [];
  mockDb.surfaceState.clear();
  mockDb.surfaceDrafts.clear();
  mockDb.routeState.clear();
  mockDb.continuity.clear();
  mockDb.contextLedger = [];
  mockDb.kernelManifests = [];
  mockDb.kernelPendingApprovals = [];
  mockDb.kernelRuntimeGrants = [];
  mockDb.kernelAuditEvents = [];
  mockDb.kernelBuildFailures = [];
}

export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getAiStatus: () => invoke<AiStatus>("get_ai_status"),
  getModelCatalog: () => invoke<ModelCatalog>("get_model_catalog"),
  testAiConnection: () =>
    invoke<{ ok: boolean; message?: string }>("test_ai_connection"),
  listProviderConnections: () =>
    invoke<ProviderConnection[]>("list_provider_connections"),
  upsertProviderConnection: (input: UpsertProviderConnectionInput) =>
    invoke<ProviderConnection>("upsert_provider_connection", { input }),
  deleteProviderConnection: (connectionId: string) =>
    invoke<void>("delete_provider_connection", { connectionId }),
  setActiveProviderConnection: (connectionId: string) =>
    invoke<ProviderConnection>("set_active_provider_connection", {
      connectionId,
    }),
  testProviderConnection: (connectionId?: string | null) =>
    invoke<{ ok: boolean; message?: string }>("test_provider_connection", {
      connectionId: connectionId ?? null,
    }),
  providerKeyHints: () => invoke<ProviderHint[]>("provider_key_hints"),
  storeHostedAuthSession: (sessionJson: string) =>
    invoke<{
      signedIn: boolean;
      adapterReady: boolean;
      userId?: string | null;
      expiresAt?: number | null;
      configured: boolean;
    }>("store_hosted_auth_session", { sessionJson }),
  clearHostedAuthSession: () =>
    invoke<{
      signedIn: boolean;
      adapterReady: boolean;
      userId?: string | null;
      expiresAt?: number | null;
      configured: boolean;
    }>("clear_hosted_auth_session"),
  getHostedAuthStatus: () =>
    invoke<{
      signedIn: boolean;
      adapterReady: boolean;
      userId?: string | null;
      expiresAt?: number | null;
      configured: boolean;
    }>("get_hosted_auth_status"),
  listConversations: () => invoke<Conversation[]>("list_conversations"),
  createConversation: () => invoke<Conversation>("create_conversation"),
  deleteConversation: (conversationId: string) =>
    invoke<void>("delete_conversation", { conversationId }),
  getMessages: (conversationId: string) =>
    invoke<ChatMessage[]>("get_messages", { conversationId }),
  deleteMessagesFrom: (messageId: string) =>
    invoke<number>("delete_messages_from", { messageId }),
  sendMessage: (args: {
    conversationId: string;
    content: string;
    activeToolId?: string | null;
    model?: string | null;
    mentions?: Array<{ toolId: string; label?: string | null }> | null;
    attachments?: Array<{
      id: string;
      name: string;
      mimeType: string;
      byteSize: number;
      localFilename: string;
    }> | null;
  }) => invoke<SendMessageResult>("send_message", args),
  stageChatAttachment: (input: {
    name: string;
    mimeType: string;
    dataBase64: string;
  }) =>
    invoke<import("@/types/attachments").StagedAttachment>(
      "stage_chat_attachment",
      { input },
    ),
  getChatAttachmentSrc: (localFilename: string) =>
    invoke<string>("get_chat_attachment_src", { localFilename }),
  cancelRequest: (conversationId?: string) =>
    invoke<void>("cancel_request", { conversationId }),
  discardToolChange: (messageId: string) =>
    invoke<ChatMessage>("discard_tool_change", { messageId }),
  discardKernelProposal: (messageId: string) =>
    invoke<ChatMessage>("discard_kernel_proposal", { messageId }),
  setKernelProposalStatus: (messageId: string, status: "applied" | "discarded") =>
    invoke<ChatMessage>("set_kernel_proposal_status", { messageId, status }),
  applyToolChange: async (args: {
    conversationId: string;
    messageId: string;
    toolChange: ToolChange;
  }) => {
    const result = await invoke<ToolRecord | ToolDefinition>("apply_tool_change", args);
    return isToolRecord(result) ? toToolDefinition(result) : result;
  },
  listTools: async () => {
    const result = await invoke<ToolRecord[] | ToolSummary[]>("list_tools");
    if (Array.isArray(result) && result.length > 0 && isToolRecord(result[0])) {
      return (result as ToolRecord[]).map(toToolSummary);
    }
    return result as ToolSummary[];
  },
  getTool: async (toolId: string) => {
    const result = await invoke<ToolRecord | ToolDefinition>("get_tool", { toolId });
    return isToolRecord(result) ? toToolDefinition(result) : result;
  },
  getToolVersions: (toolId: string) =>
    invoke<ToolVersion[]>("get_tool_versions", { toolId }),
  undoToolChange: async (toolId: string) => {
    const result = await invoke<ToolRecord | ToolDefinition>("undo_tool_change", {
      toolId,
    });
    return isToolRecord(result) ? toToolDefinition(result) : result;
  },
  saveToolState: (toolId: string, state: ToolState) =>
    invoke<void>("save_tool_state", { toolId, state }),
  getToolState: (toolId: string) =>
    invoke<ToolState>("get_tool_state", { toolId }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSetting: (key: string, value: unknown) =>
    invoke<AppSettings>("set_setting", { key, value }),
  listAddedSettings: (ownerToolId?: string | null) =>
    invoke<AddedSetting[]>("list_added_settings", {
      ownerToolId: ownerToolId ?? null,
    }),
  upsertAddedSetting: (input: UpsertAddedSettingInput) =>
    invoke<AddedSetting>("upsert_added_setting", { input }),
  deleteAddedSetting: (id: string) =>
    invoke<void>("delete_added_setting", { id }),
  validateChangeTargets: (ids: string[]) =>
    invoke<ValidateChangeTargetsResult>("validate_change_targets", { ids }),
  setDockIcon: (preference: DockIconPreference, osIsDark: boolean) =>
    invoke<void>("set_dock_icon", { preference, osIsDark }),
  setDockIconForOsAppearance: (isDark: boolean) =>
    invoke<void>("set_dock_icon_for_os_appearance", { isDark }),
  clearConversations: () => invoke<void>("clear_conversations"),
  clearTools: () => invoke<void>("clear_tools"),
  openToolWindow: (toolId: string) =>
    invoke<void>("open_tool_window", { toolId }),
  listAutomations: () => invoke<Record<string, unknown>[]>("list_automations"),
  upsertAutomation: (input: unknown) =>
    invoke<Record<string, unknown>>("upsert_automation", { input }),
  setAutomationEnabled: (automationId: string, enabled: boolean) =>
    invoke<Record<string, unknown>>("set_automation_enabled", {
      automationId,
      enabled,
    }),
  deleteAutomation: (automationId: string) =>
    invoke<void>("delete_automation", { automationId }),
  listAutomationRuns: (automationId: string) =>
    invoke<Record<string, unknown>[]>("list_automation_runs", { automationId }),
  runAutomationNow: (automationId: string) =>
    invoke<string>("run_automation_now", { automationId }),
  listWorkspaceBackgrounds: (workspaceId?: string | null) =>
    invoke<Record<string, unknown>[]>("list_workspace_backgrounds", {
      workspaceId: workspaceId ?? null,
    }),
  upsertWorkspaceBackground: (input: unknown) =>
    invoke<Record<string, unknown>>("upsert_workspace_background", { input }),
  listExportFormats: (toolId: string) =>
    invoke<
      Array<{
        id: string;
        label: string;
        extension: string;
        available: boolean;
        description: string;
      }>
    >("list_export_formats", { toolId }),
  exportTool: (input: {
    toolId: string;
    format: string;
    destinationPath: string;
  }) =>
    invoke<{ ok: boolean; path: string; format: string; message: string }>(
      "export_tool",
      { input },
    ),

  listProjects: (includeArchived = false) =>
    invoke<Project[]>("list_projects_cmd", { includeArchived }),
  getProject: (projectId: string) =>
    invoke<Project>("get_project_cmd", { projectId }),
  createProject: (input: CreateProjectInput) =>
    invoke<Project>("create_project_cmd", { input }),
  updateProject: (projectId: string, input: UpdateProjectInput) =>
    invoke<Project>("update_project_cmd", { projectId, input }),
  archiveProject: (projectId: string) =>
    invoke<Project>("archive_project_cmd", { projectId }),
  restoreProject: (projectId: string) =>
    invoke<Project>("restore_project_cmd", { projectId }),
  deleteProject: (projectId: string, mode: DeleteProjectMode = "keepChats") =>
    invoke<void>("delete_project_cmd", {
      input: { projectId, mode },
    }),
  createConversationInProject: (
    projectId: string,
    title?: string | null,
    workspaceId?: string | null,
  ) =>
    invoke<Conversation>("create_conversation_in_project", {
      projectId,
      title: title ?? null,
      workspaceId: workspaceId ?? null,
    }),
  assignChatsToProject: (projectId: string, conversationIds: string[]) =>
    invoke<Conversation[]>("assign_chats_to_project", {
      input: { projectId, conversationIds },
    }),
  assignConversationToProject: (
    conversationId: string,
    projectId: string,
  ) =>
    invoke<Conversation>("assign_conversation_to_project_cmd", {
      conversationId,
      projectId,
    }),
  removeChatFromProject: (conversationId: string) =>
    invoke<Conversation>("remove_chat_from_project", { conversationId }),
  listProjectConversations: (projectId: string) =>
    invoke<Conversation[]>("list_project_conversations_cmd", { projectId }),
  listUnassignedConversations: (workspaceId?: string | null) =>
    invoke<Conversation[]>("list_unassigned_conversations_cmd", {
      workspaceId: workspaceId ?? null,
    }),
  searchProjectContext: (
    projectId: string,
    query: string,
    limit = 10,
  ) =>
    invoke<ProjectContextHit[]>("search_project_context_cmd", {
      input: { projectId, query, limit },
    }),
  refreshProjectSummary: (projectId: string) =>
    invoke<string>("refresh_project_summary", { projectId }),
  rebuildProjectIndex: (projectId: string) =>
    invoke<number>("rebuild_project_index_cmd", { projectId }),
  renameConversation: (conversationId: string, title: string) =>
    invoke<Conversation>("rename_conversation_cmd", { conversationId, title }),
  duplicateConversation: (conversationId: string, title?: string | null) =>
    invoke<Conversation>("duplicate_conversation_cmd", {
      conversationId,
      title: title ?? null,
    }),
  exportProject: (projectId: string, destinationPath: string) =>
    invoke<{ ok: boolean; path: string; message: string }>("export_project", {
      input: { projectId, destinationPath },
    }),
  setProjectWallpaper: (projectId: string, wallpaperJson?: string | null) =>
    invoke<Project>("set_project_wallpaper_cmd", {
      projectId,
      wallpaperJson: wallpaperJson ?? null,
    }),
  touchProjectOpened: (projectId: string) =>
    invoke<Project>("touch_project_opened", { projectId }),

  getSearchConnection: () =>
    invoke<SearchConnection>("get_search_connection"),
  configureSearchConnection: (input?: {
    provider?: string;
    apiKey?: string;
  }) =>
    invoke<SearchConnection>("configure_search_connection", {
      input: input ?? {},
    }),
  deleteSearchConnection: () => invoke<void>("delete_search_connection"),
  testSearchConnection: () => invoke<string>("test_search_connection"),
  getCrawlerStatus: () => invoke<CrawlerStatus>("get_crawler_status"),
  getCrawlerInstallation: () =>
    invoke<CrawlerInstallation>("get_crawler_installation"),
  cleanupCrawlerCache: () =>
    invoke<Record<string, unknown>>("cleanup_crawler_cache"),
  getCrawlerCacheStats: () =>
    invoke<CacheStats>("get_crawler_cache_stats"),
  setWebResearchResourceProfile: (profile: ResourceProfile) =>
    invoke<string>("set_web_research_resource_profile", {
      input: { profile },
    }),

  getExaConnection: () => invoke<ExaConnection>("get_exa_connection"),
  configureExaConnection: (apiKey: string) =>
    invoke<ExaConnection>("configure_exa_connection", { input: { apiKey } }),
  deleteExaConnection: () => invoke<void>("delete_exa_connection"),
  testExaConnection: () => invoke<string>("test_exa_connection"),
  getExaUsage: (monthKey?: string | null) =>
    invoke<ExaUsageSummary>("get_exa_usage", { monthKey: monthKey ?? null }),
  getExaBudget: () => invoke<ExaBudgetStatus>("get_exa_budget"),
  setExaBudget: (input: {
    monthlyBudgetUsd?: number | null;
    softPercent?: number;
    criticalPercent?: number;
    hardPercent?: number;
  }) =>
    invoke<ExaBudgetStatus>("set_exa_budget", {
      input: {
        monthlyBudgetUsd: input.monthlyBudgetUsd ?? null,
        softPercent: input.softPercent ?? null,
        criticalPercent: input.criticalPercent ?? null,
        hardPercent: input.hardPercent ?? null,
      },
    }),
  getSearchProfile: () => invoke<SearchProfileView>("get_search_profile"),
  setSearchProfile: (profile: SearchUsageProfile) =>
    invoke<SearchProfileView>("set_search_profile", { input: { profile } }),
  listSearchSessions: (input?: {
    conversationId?: string | null;
    projectId?: string | null;
    searchType?: string | null;
    limit?: number;
  }) =>
    invoke<SearchSessionSummary[]>("list_search_sessions_cmd", {
      input: input
        ? {
            conversationId: input.conversationId ?? null,
            projectId: input.projectId ?? null,
            searchType: input.searchType ?? null,
            limit: input.limit ?? 50,
          }
        : null,
    }),
  getSearchSession: (sessionId: string) =>
    invoke<SearchSessionDetail>("get_search_session_cmd", { sessionId }),
  clearSearchHistory: (input?: {
    conversationId?: string | null;
    projectId?: string | null;
  }) =>
    invoke<number>("clear_search_history_cmd", {
      input: input
        ? {
            conversationId: input.conversationId ?? null,
            projectId: input.projectId ?? null,
          }
        : null,
    }),

  listMediaAssets: (projectId?: string | null, limit = 50) =>
    invoke<MediaAsset[]>("list_media_assets_cmd", {
      projectId: projectId ?? null,
      limit,
    }),
  getMediaAsset: (assetId: string) =>
    invoke<MediaAsset>("get_media_asset_cmd", { assetId }),
  getMediaAssetSrc: (assetId: string) =>
    invoke<MediaAssetSrc>("get_media_asset_src_cmd", { assetId }),
  getMediaAssetThumbSrc: (assetId: string) =>
    invoke<MediaAssetSrc | null>("get_media_asset_thumb_src_cmd", { assetId }),
  mediaAssetUsage: (assetId: string) =>
    invoke<MediaAssetUsage[]>("media_asset_usage_cmd", { assetId }),
  deleteMediaAsset: (assetId: string) =>
    invoke<void>("delete_media_asset_cmd", { assetId }),
  importMediaAsset: (input: ImportMediaInput) =>
    invoke<MediaAsset>("import_media_asset_cmd", { input }),
  touchMediaAsset: (assetId: string) =>
    invoke<void>("touch_media_asset_cmd", { assetId }),

  // Runtime V2
  listCapabilityPacks: () =>
    invoke<Array<Record<string, unknown>>>("list_capability_packs"),
  listConversationSurfaces: (conversationId: string) =>
    invoke<import("@/types/runtime-v2").SurfaceRecord[]>(
      "list_conversation_surfaces",
      { conversationId },
    ),
  getSurface: (surfaceId: string) =>
    invoke<import("@/types/runtime-v2").SurfaceRecord>("get_surface_cmd", {
      surfaceId,
    }),
  createInlineSurface: (args: {
    conversationId: string;
    messageId?: string | null;
    projectId?: string | null;
    name: string;
    definition: unknown;
    capabilityPacks?: string[];
  }) =>
    invoke<import("@/types/runtime-v2").SurfaceRecord>(
      "create_inline_surface_cmd",
      {
        args: {
          conversationId: args.conversationId,
          messageId: args.messageId ?? null,
          projectId: args.projectId ?? null,
          name: args.name,
          definition: args.definition,
          capabilityPacks: args.capabilityPacks ?? [],
        },
      },
    ),
  updateSurface: (args: {
    surfaceId: string;
    definition: unknown;
    changeSummary?: string;
    baseRevision?: number | null;
  }) =>
    invoke<import("@/types/runtime-v2").SurfaceRecord>("update_surface_cmd", {
      args,
    }),
  promoteSurface: (surfaceId: string) =>
    invoke<import("@/types/runtime-v2").SurfaceRecord>("promote_surface_cmd", {
      surfaceId,
    }),
  saveSurfaceState: (surfaceId: string, stateJson: ToolState) =>
    invoke<void>("save_surface_state_cmd", { surfaceId, stateJson }),
  getSurfaceState: (surfaceId: string) =>
    invoke<ToolState>("get_surface_state_cmd", { surfaceId }),
  getDraft: (surfaceId: string, componentId: string, windowId?: string | null) =>
    invoke<import("@/types/runtime-v2").SurfaceDraft | null>("get_draft_cmd", {
      surfaceId,
      componentId,
      windowId: windowId ?? null,
    }),
  saveDraft: (args: {
    surfaceId: string;
    componentId: string;
    windowId?: string | null;
    baseRevision: number;
    draft: unknown;
    formId?: string | null;
    persistencePolicy?: string;
    force?: boolean;
  }) =>
    invoke<import("@/types/runtime-v2").SurfaceDraft>("save_draft_cmd", {
      args: {
        ...args,
        windowId: args.windowId ?? "main",
      },
    }),
  schedulePatches: (args: {
    conversationId?: string | null;
    turnId?: string | null;
    surfaceId?: string | null;
    priority: string;
    operations: import("@/types/runtime-v2").AppOperation[];
    sourceType: string;
    fromAgent?: boolean;
    applyImmediately?: boolean;
    approvalGranted?: boolean;
  }) =>
    invoke<import("@/types/runtime-v2").ScheduledPatch[]>("schedule_patches_cmd", {
      args,
    }),
  flushPatchScheduler: (args?: {
    conversationId?: string | null;
    sourceType?: string;
    approvalGranted?: boolean;
  }) =>
    invoke<Record<string, unknown>[]>("flush_patch_scheduler_cmd", {
      conversationId: args?.conversationId ?? null,
      sourceType: args?.sourceType ?? "user",
      approvalGranted: args?.approvalGranted ?? true,
    }),
  getRouteState: (applicationId: string, windowId?: string | null) =>
    invoke<import("@/types/runtime-v2").RouteState>("get_route_state_cmd", {
      applicationId,
      windowId: windowId ?? null,
    }),
  setRouteState: (args: {
    applicationId: string;
    windowId?: string | null;
    currentRouteId?: string | null;
    routeParams: Record<string, unknown>;
    history: unknown[];
    historyIndex: number;
  }) =>
    invoke<import("@/types/runtime-v2").RouteState>("set_route_state_cmd", {
      args: {
        ...args,
        windowId: args.windowId ?? "main",
      },
    }),
  navigateRoute: (args: {
    applicationId: string;
    windowId?: string | null;
    routeId: string;
    routeParams?: Record<string, unknown>;
    pushHistory?: boolean;
  }) =>
    invoke<import("@/types/runtime-v2").NavigateResult>("navigate_route_cmd", {
      args: {
        ...args,
        windowId: args.windowId ?? "main",
        routeParams: args.routeParams ?? {},
      },
    }),
  appendContextLedger: (args: {
    conversationId: string;
    projectId?: string | null;
    branchId?: string | null;
    entryType: string;
    visibility?: string;
    payload: unknown;
    summary: string;
    expirationClass?: string | null;
  }) =>
    invoke<import("@/types/runtime-v2").ContextLedgerEntry>(
      "append_context_ledger_cmd",
      { args },
    ),
  listContextLedger: (
    conversationId: string,
    projectId?: string | null,
    limit?: number,
  ) =>
    invoke<import("@/types/runtime-v2").ContextLedgerEntry[]>(
      "list_context_ledger_cmd",
      {
        conversationId,
        projectId: projectId ?? null,
        limit: limit ?? null,
      },
    ),
  getProviderProfile: (providerId: string, modelId?: string | null) =>
    invoke<import("@/types/runtime-v2").ProviderConformanceRecord>(
      "get_provider_profile_cmd",
      {
        providerId,
        modelId: modelId ?? null,
      },
    ),
  getContinuity: (surfaceId: string, windowId?: string | null) =>
    invoke<import("@/types/runtime-v2").ContinuitySnapshot>("get_continuity_cmd", {
      surfaceId,
      windowId: windowId ?? null,
    }),
  saveContinuity: (args: {
    surfaceId: string;
    windowId?: string | null;
    focus: Record<string, unknown>;
    scroll: Record<string, unknown>;
    media: Record<string, unknown>;
    suspensionState?: string;
  }) =>
    invoke<import("@/types/runtime-v2").ContinuitySnapshot>("save_continuity_cmd", {
      args: {
        ...args,
        windowId: args.windowId ?? "main",
      },
    }),
  suspendSurface: (surfaceId: string, windowId?: string | null) =>
    invoke<import("@/types/runtime-v2").ContinuitySnapshot>("suspend_surface_cmd", {
      surfaceId,
      windowId: windowId ?? null,
    }),
  applyOperations: (args: {
    conversationId?: string | null;
    projectId?: string | null;
    turnId?: string | null;
    summary?: string;
    operations: import("@/types/runtime-v2").AppOperation[];
    silent?: boolean;
  }) => invoke<Record<string, unknown>>("apply_operations_cmd", { args }),
  undoTransaction: (transactionId: string) =>
    invoke<Record<string, unknown>>("undo_transaction_cmd", { transactionId }),
  listTransactions: (conversationId: string, limit?: number) =>
    invoke<Record<string, unknown>[]>("list_transactions_cmd", {
      conversationId,
      limit: limit ?? null,
    }),
  branchConversation: (args: {
    sourceConversationId: string;
    sourceMessageId: string;
    branchName?: string;
  }) => invoke<unknown>("branch_conversation_cmd", { args }),
  listBranches: (conversationId: string) =>
    invoke<Record<string, unknown>[]>("list_branches_cmd", { conversationId }),
  createSnapshot: (
    conversationId: string,
    projectId?: string | null,
    description?: string,
  ) =>
    invoke<Record<string, unknown>>("create_snapshot_cmd", {
      conversationId,
      projectId: projectId ?? null,
      description: description ?? null,
    }),
  getSnapshot: (snapshotId: string) =>
    invoke<Record<string, unknown>>("get_snapshot_cmd", { snapshotId }),
  deleteSnapshot: (snapshotId: string) =>
    invoke<void>("delete_snapshot_cmd", { snapshotId }),
  listAgentQueue: (conversationId: string) =>
    invoke<Record<string, unknown>[]>("list_agent_queue_cmd", {
      conversationId,
    }),
  cancelQueueItem: (itemId: string) =>
    invoke<Record<string, unknown>>("cancel_queue_item_cmd", { itemId }),
  removeQueueItem: (itemId: string) =>
    invoke<void>("remove_queue_item_cmd", { itemId }),
  listDiagnostics: (conversationId: string, limit?: number) =>
    invoke<unknown[]>("list_diagnostics_cmd", {
      conversationId,
      limit: limit ?? null,
    }),
  storeDiagnostics: (
    conversationId: string | null,
    turnId: string | null,
    payload: unknown,
  ) =>
    invoke<string>("store_diagnostics_cmd", {
      conversationId,
      turnId,
      payload,
    }),
  runtimeV2Limits: () => invoke<Record<string, unknown>>("runtime_v2_limits"),

  // Application Kernel
  kernelCapabilityCatalog: () => invoke<Record<string, unknown>>("kernel_capability_catalog"),
  kernelApplyChange: (request: Record<string, unknown>) =>
    invoke<Record<string, unknown>>("kernel_apply_change", { request }),
  kernelCompileIntent: (intent: Record<string, unknown>) =>
    invoke<Record<string, unknown>>("kernel_compile_intent", { intent }),
  kernelListManifests: () =>
    invoke<import("@/types/application-kernel").ManifestRecord[]>("kernel_list_manifests"),
  kernelGetManifest: (applicationId: string) =>
    invoke<import("@/types/application-kernel").ManifestRecord>("kernel_get_manifest", {
      applicationId,
    }),
  kernelRestoreLastKnownGood: (applicationId: string) =>
    invoke<import("@/types/application-kernel").ManifestRecord>(
      "kernel_restore_last_known_good",
      { applicationId },
    ),
  kernelGetRecoveryState: () =>
    invoke<import("@/types/application-kernel").RecoveryState>("kernel_get_recovery_state"),
  kernelSetRecoveryMode: (enabled: boolean) =>
    invoke<import("@/types/application-kernel").RecoveryState>("kernel_set_recovery_mode", {
      enabled,
    }),
  kernelClearRecovery: () =>
    invoke<import("@/types/application-kernel").RecoveryState>("kernel_clear_recovery"),
  kernelSetRecoveryFlags: (flags: {
    disableUserSurfaces?: boolean | null;
    disableCustomLayouts?: boolean | null;
    disableCapabilityPacks?: boolean | null;
  }) =>
    invoke<import("@/types/application-kernel").RecoveryState>("kernel_set_recovery_flags", {
      disableUserSurfaces: flags.disableUserSurfaces ?? null,
      disableCustomLayouts: flags.disableCustomLayouts ?? null,
      disableCapabilityPacks: flags.disableCapabilityPacks ?? null,
    }),
  kernelEnterSafeStartup: (reason: string) =>
    invoke<import("@/types/application-kernel").RecoveryState>("kernel_enter_safe_startup", {
      reason,
    }),
  kernelUnifiedSearch: (query: string, limit?: number) =>
    invoke<import("@/types/application-kernel").UnifiedSearchHit[]>("kernel_unified_search", {
      query,
      limit: limit ?? null,
    }),
  kernelExportPackage: (applicationId: string) =>
    invoke<Record<string, unknown>>("kernel_export_package", { applicationId }),
  kernelPreviewPackage: (bytes: number[]) =>
    invoke<import("@/types/application-kernel").PackagePreview>("kernel_preview_package", {
      bytes,
    }),
  kernelImportPackage: (bytes: number[], approve: boolean, remintIds?: boolean) =>
    invoke<import("@/types/application-kernel").ApplicationManifest>("kernel_import_package", {
      bytes,
      approve,
      remintIds: remintIds ?? true,
    }),
  kernelApplicationSummary: (applicationId: string) =>
    invoke<Record<string, unknown>>("kernel_application_summary", { applicationId }),
  kernelGrantPermission: (applicationId: string, permission: string) =>
    invoke<Record<string, unknown>>("kernel_grant_permission", {
      applicationId,
      permission,
      scope: null,
    }),
  kernelRevokePermission: (applicationId: string, permission: string) =>
    invoke<void>("kernel_revoke_permission", { applicationId, permission }),
  kernelGarbageCollect: () => invoke<number>("kernel_garbage_collect"),
  kernelVisualChecks: (width: number) =>
    invoke<Record<string, unknown>>("kernel_visual_checks", { width }),

  kernelListRegisteredActions: () =>
    invoke<Record<string, unknown>>("kernel_list_registered_actions"),
  kernelInvokeRegisteredAction: (request: ClientActionRequest) =>
    invoke<ActionOutcome>("kernel_invoke_registered_action", { request }),
  kernelListPendingApprovals: () =>
    invoke<ApprovalRequest[]>("kernel_list_pending_approvals"),
  kernelDecideApproval: (
    approvalId: string,
    approve: boolean,
    rememberScope?: RememberScope | null,
    rememberDuration?: RememberDuration | null,
  ) =>
    invoke<ApprovalDecisionResult>("kernel_decide_approval", {
      approvalId,
      approve,
      rememberScope: rememberScope ?? null,
      rememberDuration: rememberDuration ?? null,
    }),
  kernelListRuntimeGrants: (applicationId?: string | null) =>
    invoke<RuntimeGrant[]>("kernel_list_runtime_grants", {
      applicationId: applicationId ?? null,
    }),
  kernelRevokeRuntimeGrant: (grantId: string) =>
    invoke<void>("kernel_revoke_runtime_grant", { grantId }),
  kernelListAuditEvents: (applicationId?: string | null, limit?: number) =>
    invoke<AuditEvent[]>("kernel_list_audit_events", {
      applicationId: applicationId ?? null,
      limit: limit ?? null,
    }),
  kernelClearAuditEvents: () => invoke<number>("kernel_clear_audit_events"),
  kernelSetApplicationLifecycle: (applicationId: string, enabled: boolean) =>
    invoke<ManifestRecord>("kernel_set_application_lifecycle", {
      applicationId,
      enabled,
    }),
  kernelRecordBuildFailure: (
    applicationId: string,
    message: string,
    retryable?: boolean,
    requestRef?: string | null,
  ) =>
    invoke<BuildFailure>("kernel_record_build_failure", {
      applicationId,
      message,
      retryable: retryable ?? null,
      requestRef: requestRef ?? null,
    }),
  kernelClearBuildFailure: (applicationId: string) =>
    invoke<number>("kernel_clear_build_failure", { applicationId }),
  kernelListBuildFailures: (applicationId: string) =>
    invoke<BuildFailure[]>("kernel_list_build_failures", { applicationId }),
  kernelListApplicationVersions: (applicationId: string) =>
    invoke<ApplicationVersion[]>("kernel_list_application_versions", {
      applicationId,
    }),
};
