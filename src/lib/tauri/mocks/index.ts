import { DEFAULT_WALLPAPER, WallpaperKindSchema } from "@/types/agent";
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
import type { SafeSearchLevel } from "@/types/search";
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
  RuntimeGrant,
} from "@/types/application-kernel";
import type {
  ToolDefinition,
  ToolState,
  ToolSummary,
  ToolVersion,
} from "@/types/tool";
import { TauriCommandError } from "../errors";

/** Commands intentionally mock-only (no Rust registration). Keep empty unless justified. */
export const MOCK_ONLY_COMMANDS = new Set<string>([]);

/** Distinctive fixture phrase — must not appear in the production main JS chunk. */
export const MOCK_FIXTURE_MARKER = "A short quiz generated for preview.";

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

let mockTutorialProgress: import("@/lib/onboarding/types").TutorialProgress[] = [];

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
    interfaceTransparency: 20,
    adaptiveWindowSizing: "smart" as "smart" | "ask" | "off",
    chatToolSplitRatio: 0.5,
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

export async function mockInvoke<T>(
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

    case "get_bootstrap_status":
      return { status: "ready" } as T;

    case "retry_open_database":
      return { status: "ready" } as T;

    case "get_storage_summary":
      return {
        chatCount: mockDb.conversations.length,
        toolCount: mockDb.tools.length,
        projectCount: mockDb.projects?.length ?? 0,
        databaseBytes: 4096,
        backupBytes: 8192,
        mediaBytes: 0,
        profileReady: true,
      } as T;

    case "get_database_health":
      return {
        status: "healthy",
        quickCheck: "ok",
        foreignKeyCheck: "ok",
        schemaVersion: "015_registered_actions",
        byteSize: 4096,
      } as T;

    case "create_profile_backup":
      return {
        path: "coreside-profile-mock.coreside-backup",
        byteSize: 4096,
        schemaVersion: "015_registered_actions",
        format: "coreside-backup",
      } as T;

    case "preview_restore_backup":
      return {
        format: "coreside-backup",
        schemaVersion: 1,
        applicationVersion: "0.1.0",
        createdAt: new Date().toISOString(),
        latestMigration: "015_registered_actions",
        databaseBytes: 4096,
        mediaCount: 0,
        attachmentCount: 0,
        integrityOk: true,
        warnings: ["Media files were not included in this backup."],
      } as T;

    case "list_managed_backups":
      return [
        { path: "coreside-profile-mock.coreside-backup", byteSize: 4096 },
      ] as T;

    case "restore_profile_backup":
      return {
        safetyBackupPath: "coreside-safety-before-restore-mock.coreside-backup",
        restoredFrom: "coreside-profile-mock.coreside-backup",
        restartRecommended: true,
      } as T;

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
        url: `asset://localhost/mock-media/${asset.localFilename}`,
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
        url: `asset://localhost/mock-media/${asset.thumbnailFilename}`,
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
      const structuredUserInput =
        args?.structuredUserInput && typeof args.structuredUserInput === "object"
          ? (args.structuredUserInput as {
              formId?: string;
              applicationId?: string | null;
              surfaceId?: string | null;
              fields?: Record<string, unknown>;
            })
          : null;
      const messages = mockDb.messages.get(conversationId) ?? [];
      const userMessage: ChatMessage = {
        id: crypto.randomUUID(),
        conversationId,
        role: "user",
        content,
        createdAt: now(),
        status: "ok",
        metadata:
          mentions.length > 0 ||
          attachments.length > 0 ||
          structuredUserInput
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
                ...(attachments.length > 0 ? { attachments } : {}),
                ...(structuredUserInput
                  ? {
                      // Mock only — production trust is sealed in Rust.
                      structuredUserInput: {
                        submissionId: `mock-sui-${crypto.randomUUID()}`,
                        formId: String(structuredUserInput.formId ?? "form"),
                        applicationId: structuredUserInput.applicationId ?? null,
                        surfaceId: structuredUserInput.surfaceId ?? null,
                        conversationId,
                        fields: structuredUserInput.fields ?? {},
                        trustClass: "localUserGesture",
                        contentHash: "mock",
                        instructionEligibility: "localUserContent",
                      },
                      structuredTrustSource: "typed_part",
                    }
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
          description: MOCK_FIXTURE_MARKER,
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
      } else if (lower.includes("progressive op preview")) {
        result = {
          messageId: assistantId,
          assistantMessage: "Progressive op preview complete",
          responseType: "message",
          toolChange: null,
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
      // Optional Channel callback (web/vitest): simulate scoped text delivery.
      const onEvent = args?.onEvent;
      if (typeof onEvent === "function") {
        if (lower.includes("progressive op preview")) {
          // Mirror Rust mock: Operation preview before final text.
          onEvent({
            kind: "operation",
            conversationId,
            operationId: "op-progressive-preview",
            status: "preview",
          });
        }
        onEvent({
          kind: "text",
          conversationId,
          text: result.assistantMessage,
          turnId: `mock-turn-${assistantMessage.id}`,
          sequence: 1,
          delta: result.assistantMessage,
        });
      }
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
      if (
        key === "wallpaper" ||
        key === "wallpaperJson" ||
        key === "wallpaper_json" ||
        key === "interfaceTransparency" ||
        key === "interface_transparency"
      ) {
        throw new TauriCommandError(
          "Use set_workspace_appearance; wallpaper and interface transparency cannot be changed via set_setting.",
          "forbidden",
        );
      }
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
        const raw =
          typeof value === "string"
            ? value.trim()
            : value == null
              ? ""
              : String(value).trim();
        // Mirror Rust clear path: empty or legacy `{kind:"none"}` → null.
        let legacyNone = false;
        if (raw) {
          try {
            const parsed = JSON.parse(raw) as {
              kind?: unknown;
              schemaVersion?: unknown;
              type?: unknown;
            };
            legacyNone =
              parsed &&
              typeof parsed === "object" &&
              parsed.kind === "none" &&
              parsed.schemaVersion == null &&
              parsed.type == null;
          } catch {
            legacyNone = false;
          }
        }
        mockDb.settings.wallpaperJson = raw && !legacyNone ? raw : null;
      }
      if (key === "interfaceTransparency") {
        const n = typeof value === "number" ? value : Number(value);
        if (Number.isFinite(n)) {
          mockDb.settings.interfaceTransparency = Math.min(
            60,
            Math.max(0, Math.round(n)),
          );
        }
      }
      if (key === "adaptiveWindowSizing" && typeof value === "string") {
        const mode = value.trim().toLowerCase();
        if (mode === "smart" || mode === "ask" || mode === "off") {
          mockDb.settings.adaptiveWindowSizing = mode;
        }
      }
      if (key === "chatToolSplitRatio") {
        const n = typeof value === "number" ? value : Number(value);
        if (Number.isFinite(n)) {
          mockDb.settings.chatToolSplitRatio = Math.min(0.72, Math.max(0.28, n));
        }
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

    case "set_workspace_appearance": {
      const input = (args?.input ?? args) as {
        wallpaperJson?: string | null;
        interfaceTransparency?: number | null;
      };
      if (
        input?.wallpaperJson === undefined &&
        input?.interfaceTransparency === undefined
      ) {
        throw new TauriCommandError(
          "set_workspace_appearance requires wallpaperJson and/or interfaceTransparency",
          "invalid",
        );
      }
      if (input.wallpaperJson !== undefined) {
        // null / empty clears the pair (match api.setWorkspaceAppearance).
        const raw = String(input.wallpaperJson ?? "").trim();
        if (!raw) {
          mockDb.settings.wallpaperJson = null;
          mockDb.settings.wallpaper = { ...DEFAULT_WALLPAPER };
        } else {
          let parsed: {
            kind?: unknown;
            schemaVersion?: unknown;
            type?: unknown;
          };
          try {
            parsed = JSON.parse(raw) as typeof parsed;
          } catch {
            throw new TauriCommandError(
              "wallpaperJson must be valid JSON",
              "invalid",
            );
          }
          if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
            throw new TauriCommandError(
              "wallpaperJson must be a JSON object",
              "invalid",
            );
          }
          if (
            parsed.schemaVersion === "1" &&
            typeof parsed.type === "string"
          ) {
            mockDb.settings.wallpaperJson = raw;
            mockDb.settings.wallpaper = { ...DEFAULT_WALLPAPER };
          } else if (typeof parsed.kind === "string") {
            if (parsed.kind === "none") {
              mockDb.settings.wallpaperJson = null;
              mockDb.settings.wallpaper = { ...DEFAULT_WALLPAPER };
            } else {
              mockDb.settings.wallpaper = sanitizeMockWallpaper(parsed as never);
              mockDb.settings.wallpaperJson = null;
            }
          } else {
            throw new TauriCommandError(
              "wallpaperJson must include schemaVersion/type or a legacy kind",
              "invalid",
            );
          }
        }
      }
      if (
        input.interfaceTransparency !== undefined &&
        input.interfaceTransparency !== null
      ) {
        const n = Number(input.interfaceTransparency);
        if (!Number.isFinite(n) || n < 0 || n > 60) {
          throw new TauriCommandError(
            "interfaceTransparency must be between 0 and 60",
            "invalid",
          );
        }
        mockDb.settings.interfaceTransparency = Math.min(
          60,
          Math.max(0, Math.round(n)),
        );
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

    case "window_orchestrator_inspect":
      return {
        bounds: { x: 0, y: 0, width: 1280, height: 800 },
        maximized: false,
        fullscreen: false,
        minimized: false,
        scaleFactor: 1,
        animating: false,
        restoreEligible: false,
        workArea: { x: 0, y: 0, width: 1440, height: 900 },
        canExpand: true,
      } as T;

    case "window_orchestrator_expand":
      return {
        decision: "no_op",
        reason: "web_preview",
        toolId: args?.toolId ?? null,
        from: { x: 0, y: 0, width: 1280, height: 800 },
        restoreEligible: false,
      } as T;

    case "window_orchestrator_restore":
      return {
        decision: "no_op",
        reason: "web_preview",
        from: { x: 0, y: 0, width: 1280, height: 800 },
        restoreEligible: false,
      } as T;

    case "window_orchestrator_cancel":
      return undefined as T;

    case "open_external_url": {
      const raw = String(args?.url ?? "").trim();
      // Web preview only — production always opens via the Rust command.
      // Still apply static host blocks so the mock cannot open private targets.
      let parsed: URL;
      try {
        parsed = new URL(raw);
      } catch {
        throw Object.assign(new Error("That link cannot be opened safely."), {
          code: "invalid_url",
        });
      }
      const scheme = parsed.protocol.toLowerCase();
      const host = parsed.hostname.toLowerCase();
      const blockedHost =
        !host ||
        !!parsed.username ||
        !!parsed.password ||
        (scheme !== "http:" && scheme !== "https:") ||
        host === "localhost" ||
        host === "127.0.0.1" ||
        host === "0.0.0.0" ||
        host === "::1" ||
        host === "[::1]" ||
        host === "metadata.google.internal" ||
        host === "metadata.goog" ||
        host.endsWith(".localhost") ||
        host.endsWith(".local") ||
        host.endsWith(".internal") ||
        /^(10|127)\.\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(host) ||
        /^192\.168\.\d{1,3}\.\d{1,3}$/.test(host) ||
        /^169\.254\.\d{1,3}\.\d{1,3}$/.test(host) ||
        /^172\.(1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}$/.test(host) ||
        /^100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.\d{1,3}\.\d{1,3}$/.test(host);
      if (blockedHost) {
        throw Object.assign(new Error("That link cannot be opened safely."), {
          code: "invalid_url",
        });
      }
      if (typeof window !== "undefined") {
        window.open(raw, "_blank", "noopener,noreferrer");
      }
      return undefined as T;
    }

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
      } as T;
    }

    case "cancel_chat_attachment":
      return undefined as T;

    case "get_chat_attachment_src":
      return {
        id: String(args?.attachmentId ?? "file"),
        url: `coreside-asset://localhost/attachment/${String(args?.conversationId ?? "conv")}/${String(args?.attachmentId ?? "file")}`,
      } as T;

    case "run_attachment_gc":
      return {
        expired: 0,
        missingDurable: 0,
        promotedOrphans: 0,
      } as T;

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

    case "delete_draft_cmd": {
      const surfaceId = String(args?.surfaceId ?? "");
      const componentId = String(args?.componentId ?? "");
      const windowId = String(args?.windowId ?? "main");
      mockDb.surfaceDrafts.delete(`${surfaceId}:${componentId}:${windowId}`);
      return undefined as T;
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

    // ponytail: web-preview stubs for History panel; real data lives in Rust/SQLite
    case "list_branches_cmd":
    case "list_snapshots_cmd":
    case "list_transactions_cmd":
    case "list_diagnostics_cmd":
    case "list_turn_timeline_cmd":
    case "list_agent_queue_cmd":
      return [] as T;
    case "subscribe_conversation_queue":
    case "subscribe_conversation_sync":
      // Channel registration is a no-op in web/vitest mocks.
      return undefined as T;
    case "create_snapshot_cmd": {
      const a = (args ?? {}) as {
        conversationId?: string;
        projectId?: string | null;
        description?: string | null;
      };
      return {
        id: `snap-mock-${Date.now()}`,
        conversationId: a.conversationId ?? "conv-mock",
        projectId: a.projectId ?? null,
        description: a.description || "Mock snapshot",
        payload: { readOnly: true },
        createdAt: now(),
      } as T;
    }
    case "get_snapshot_cmd":
      return {
        id: String((args as { snapshotId?: string })?.snapshotId ?? "snap-mock"),
        conversationId: "conv-mock",
        projectId: null,
        description: "Mock snapshot",
        payload: { readOnly: true, messages: [], transactions: [] },
        createdAt: now(),
      } as T;
    case "branch_conversation_cmd": {
      const a = (args as { args?: Record<string, unknown> })?.args ?? {};
      return [
        {
          id: `br-mock-${Date.now()}`,
          sourceConversationId: a.sourceConversationId ?? "conv-mock",
          sourceMessageId: a.sourceMessageId ?? null,
          newConversationId: `conv-branch-${Date.now()}`,
          branchName: a.branchName || "Branch",
          createdAt: now(),
        },
        [],
      ] as T;
    }

    case "get_onboarding_state": {
      const essentials = mockTutorialProgress.find(
        (p) => p.tutorialId === "coreside-essentials",
      );
      const essentialsDone =
        essentials?.status === "completed" ||
        essentials?.status === "skipped" ||
        essentials?.status === "superseded" ||
        essentials?.status === "in_progress";
      return {
        welcomeEligible: !essentialsDone && mockDb.conversations.length === 0,
        onboardingDisabled: false,
        isSecondaryWindow: false,
        recoveryMode: false,
        hasMeaningfulActivity: mockDb.conversations.length > 0,
        progress: mockTutorialProgress,
      } as T;
    }

    case "upsert_tutorial_progress": {
      const a = (args ?? {}) as {
        tutorialId?: string;
        tutorialVersion?: number;
        status?: string;
        currentStepId?: string | null;
        completedStepIds?: string[] | null;
      };
      const id = a.tutorialId ?? "coreside-essentials";
      const idx = mockTutorialProgress.findIndex((p) => p.tutorialId === id);
      const prev = idx >= 0 ? mockTutorialProgress[idx] : null;
      const row = {
        tutorialId: id,
        tutorialVersion: a.tutorialVersion ?? 1,
        status: (a.status ?? "not_started") as
          | "not_started"
          | "in_progress"
          | "completed"
          | "skipped"
          | "superseded",
        currentStepId: a.currentStepId ?? null,
        completedStepIds: a.completedStepIds ?? prev?.completedStepIds ?? [],
        startedAt: prev?.startedAt ?? now(),
        updatedAt: now(),
        completedAt: a.status === "completed" ? now() : prev?.completedAt ?? null,
        skippedAt: a.status === "skipped" ? now() : prev?.skippedAt ?? null,
        lastOpenedAt: now(),
      };
      if (idx >= 0) mockTutorialProgress[idx] = row;
      else mockTutorialProgress.push(row);
      return row as T;
    }

    case "reset_tutorial_progress": {
      const id = (args as { tutorialId?: string | null })?.tutorialId;
      if (id) {
        const before = mockTutorialProgress.length;
        mockTutorialProgress = mockTutorialProgress.filter((p) => p.tutorialId !== id);
        return (before - mockTutorialProgress.length) as T;
      }
      const n = mockTutorialProgress.length;
      mockTutorialProgress = [];
      return n as T;
    }

    case "seed_tutorial_sample":
      return {
        conversationId: "conv-tutorial-sample",
        toolId: "tool-tutorial-sample-planner",
        created: true,
      } as T;

    case "cleanup_tutorial_sample":
      return 0 as T;

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
  mockTutorialProgress = [];
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
    interfaceTransparency: 20,
    adaptiveWindowSizing: "smart",
    chatToolSplitRatio: 0.5,
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
  mockDb.searchSessions = [];
}
