import type {
  AiStatus,
  AppInfo,
  AppSettings,
  DockIconConfig,
  DockIconCommitResult,
  ModelCatalog,
  SendMessageResult,
  ToolChange,
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
import type {
  CacheStats,
  CrawlerInstallation,
  CrawlerStatus,
  ExaBudgetStatus,
  ExaConnection,
  ExaUsageSummary,
  ResourceProfile,
  SearchConnection,
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
  RememberDuration,
  RememberScope,
  RuntimeGrant,
} from "@/types/application-kernel";
import type { ToolDefinition, ToolState, ToolSummary, ToolVersion } from "@/types/tool";
import { Channel } from "@tauri-apps/api/core";
import type { AgentTurnEvent } from "./events";
import { subscribeConversationQueue as subscribeConversationQueueChannel } from "./events";
import { subscribeConversationSync as subscribeConversationSyncChannel } from "./events";
import { invoke } from "./invoke";
import { isTauriRuntime } from "./runtime";
import { isToolRecord, toToolDefinition, toToolSummary, type ToolRecord } from "./tools";

export type WindowExpandDirection =
  | "right"
  | "left"
  | "down"
  | "up"
  | "balanced"
  | "automatic";

export interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowOrchestratorInspect {
  bounds: WindowBounds;
  maximized: boolean;
  fullscreen: boolean;
  minimized: boolean;
  scaleFactor: number;
  animating: boolean;
  restoreEligible: boolean;
  workArea: WindowBounds;
  canExpand: boolean;
}

export type WindowExpansionDecision =
  | {
      decision: "expand";
      reason: string;
      toolId: string;
      from: WindowBounds;
      to: WindowBounds;
      direction: string;
      restoreEligible: boolean;
    }
  | {
      decision: "no_op";
      reason: string;
      toolId?: string | null;
      from: WindowBounds;
      restoreEligible: boolean;
    }
  | {
      decision: "cannot_satisfy";
      reason: string;
      toolId?: string | null;
      from: WindowBounds;
      bestEffort?: WindowBounds | null;
      restoreEligible: boolean;
    };

export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getBootstrapStatus: () => invoke<import("@/types/bootstrap").BootstrapStatus>("get_bootstrap_status"),
  retryOpenDatabase: () =>
    invoke<import("@/types/bootstrap").BootstrapStatus>("retry_open_database"),
  getStorageSummary: () =>
    invoke<{
      chatCount: number;
      toolCount: number;
      projectCount: number;
      databaseBytes: number | null;
      backupBytes: number | null;
      mediaBytes: number | null;
      profileReady: boolean;
    }>("get_storage_summary"),
  getDatabaseHealth: () =>
    invoke<{
      status: string;
      quickCheck: string;
      foreignKeyCheck: string;
      schemaVersion: string | null;
      byteSize: number | null;
    }>("get_database_health"),
  createProfileBackup: () =>
    invoke<{
      path: string;
      byteSize: number;
      schemaVersion: string | null;
      format: string;
    }>("create_profile_backup"),
  previewRestoreBackup: (path: string) =>
    invoke<{
      format: string;
      schemaVersion: number;
      applicationVersion: string;
      createdAt: string;
      latestMigration: string | null;
      databaseBytes: number;
      mediaCount: number;
      attachmentCount: number;
      integrityOk: boolean;
      warnings: string[];
    }>("preview_restore_backup", { path }),
  listManagedBackups: () =>
    invoke<Array<{ path: string; byteSize: number }>>("list_managed_backups"),
  restoreProfileBackup: (path: string, confirm: boolean) =>
    invoke<{
      safetyBackupPath: string;
      restoredFrom: string;
      restartRecommended: boolean;
    }>("restore_profile_backup", { path, confirm }),
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
      plan?: {
        planId: string;
        displayName: string;
        hostedAiEnabled: boolean;
        allowanceAmount: number;
        source: string;
      };
      availablePlans?: Array<{
        planId: string;
        displayName: string;
        hostedAiEnabled: boolean;
        allowanceAmount: number;
        source: string;
      }>;
    }>("store_hosted_auth_session", { sessionJson }),
  clearHostedAuthSession: () =>
    invoke<{
      signedIn: boolean;
      adapterReady: boolean;
      userId?: string | null;
      expiresAt?: number | null;
      configured: boolean;
      plan?: {
        planId: string;
        displayName: string;
        hostedAiEnabled: boolean;
        allowanceAmount: number;
        source: string;
      };
      availablePlans?: Array<{
        planId: string;
        displayName: string;
        hostedAiEnabled: boolean;
        allowanceAmount: number;
        source: string;
      }>;
    }>("clear_hosted_auth_session"),
  getHostedAuthStatus: () =>
    invoke<{
      signedIn: boolean;
      adapterReady: boolean;
      userId?: string | null;
      expiresAt?: number | null;
      configured: boolean;
      plan?: {
        planId: string;
        displayName: string;
        hostedAiEnabled: boolean;
        allowanceAmount: number;
        source: string;
      };
      availablePlans?: Array<{
        planId: string;
        displayName: string;
        hostedAiEnabled: boolean;
        allowanceAmount: number;
        source: string;
      }>;
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
      name?: string;
      mimeType?: string;
      byteSize?: number;
    }> | null;
    /** Typed form fields — Rust seals trust; never rely on text markers. */
    structuredUserInput?: {
      formId: string;
      applicationId?: string | null;
      surfaceId?: string | null;
      fields: Record<string, unknown>;
    } | null;
    /** Per-send Channel handler — authoritative for text/action/error/operation. */
    onEvent?: (event: AgentTurnEvent) => void;
  }) => {
    const { onEvent, ...cmdArgs } = args;
    if (isTauriRuntime()) {
      // Channel is required on the Rust command; binds delivery to this invoke.
      const channel = new Channel<AgentTurnEvent>();
      if (onEvent) {
        channel.onmessage = onEvent;
      }
      return invoke<SendMessageResult>("send_message", {
        ...cmdArgs,
        onEvent: channel,
      });
    }
    // Web / vitest mocks: pass the callback through (no real Channel).
    return invoke<SendMessageResult>("send_message", {
      ...cmdArgs,
      onEvent: onEvent ?? null,
    });
  },
  stageChatAttachment: (input: {
    name: string;
    mimeType: string;
    dataBase64: string;
  }) =>
    invoke<import("@/types/attachments").StagedAttachment>(
      "stage_chat_attachment",
      { input },
    ),
  cancelChatAttachment: (attachmentId: string) =>
    invoke<void>("cancel_chat_attachment", { attachmentId }),
  getChatAttachmentSrc: (attachmentId: string, conversationId: string) =>
    invoke<{ id: string; url: string }>("get_chat_attachment_src", {
      attachmentId,
      conversationId,
    }),
  runAttachmentGc: () =>
    invoke<{
      expired: number;
      missingDurable: number;
      promotedOrphans: number;
    }>("run_attachment_gc"),
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
  setWorkspaceAppearance: (input: {
    wallpaperJson?: string | null;
    interfaceTransparency?: number | null;
  }) => {
    const payload: {
      wallpaperJson?: string | null;
      interfaceTransparency?: number | null;
    } = {};
    if (input.wallpaperJson !== undefined) {
      // null means clear (same as ""); omit the key to leave wallpaper unchanged.
      payload.wallpaperJson = input.wallpaperJson ?? "";
    }
    if (input.interfaceTransparency !== undefined) {
      payload.interfaceTransparency = input.interfaceTransparency;
    }
    return invoke<AppSettings>("set_workspace_appearance", { input: payload });
  },
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
  commitDockIconPreference: (preference: DockIconConfig) =>
    invoke<DockIconCommitResult>("commit_dock_icon_preference", { preference }),
  getDockIconStatus: () =>
    invoke<DockIconCommitResult>("get_dock_icon_status"),
  applyPersistedDockIcon: () =>
    invoke<DockIconCommitResult>("apply_persisted_dock_icon"),
  clearConversations: () => invoke<void>("clear_conversations"),
  clearTools: () => invoke<void>("clear_tools"),
  openToolWindow: (toolId: string, size?: { width?: number; height?: number }) =>
    invoke<void>("open_tool_window", {
      toolId,
      width: size?.width,
      height: size?.height,
    }),
  windowOrchestratorInspect: () =>
    invoke<WindowOrchestratorInspect>("window_orchestrator_inspect"),
  windowOrchestratorExpand: (args: {
    toolId: string;
    minUsefulWidth: number;
    minUsefulHeight: number;
    direction?: WindowExpandDirection;
    reducedMotion?: boolean;
  }) =>
    invoke<WindowExpansionDecision>("window_orchestrator_expand", {
      toolId: args.toolId,
      minUsefulWidth: args.minUsefulWidth,
      minUsefulHeight: args.minUsefulHeight,
      direction: args.direction,
      reducedMotion: args.reducedMotion,
    }),
  windowOrchestratorRestore: (reducedMotion?: boolean) =>
    invoke<WindowExpansionDecision>("window_orchestrator_restore", {
      reducedMotion,
    }),
  windowOrchestratorCancel: () => invoke<void>("window_orchestrator_cancel"),
  openExternalUrl: (url: string) => invoke<void>("open_external_url", { url }),
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
  saveSurfaceState: (surfaceId: string, stateJson: ToolState, expectedStateRevision?: number | null) =>
    invoke<number>("save_surface_state_cmd", { surfaceId, stateJson, expectedStateRevision: expectedStateRevision ?? null }),
  getSurfaceState: (surfaceId: string) =>
    invoke<ToolState>("get_surface_state_cmd", { surfaceId }),
  /** Returns both state and stateRevision atomically. Use instead of getSurface()+getSurfaceState()
   *  to avoid a TOCTOU race where definition revision and state revision diverge. */
  getSurfaceStateWithRevision: (surfaceId: string) =>
    invoke<{ state: ToolState; stateRevision: number }>("get_surface_state_with_revision_cmd", { surfaceId }),
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
  deleteDraft: (
    surfaceId: string,
    componentId: string,
    windowId?: string | null,
  ) =>
    invoke<void>("delete_draft_cmd", {
      surfaceId,
      componentId,
      windowId: windowId ?? null,
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
  routeBack: (applicationId: string) =>
    invoke<import("@/types/runtime-v2").NavigateResult>("route_back_cmd", {
      applicationId,
    }),
  routeForward: (applicationId: string) =>
    invoke<import("@/types/runtime-v2").NavigateResult>("route_forward_cmd", {
      applicationId,
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
  diffBranch: (branchId: string) =>
    invoke<import("@/types/runtime-v2").BranchDiffRecord>("diff_branch_cmd", {
      branchId,
    }),
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
  listSnapshots: (conversationId: string) =>
    invoke<Record<string, unknown>[]>("list_snapshots_cmd", { conversationId }),
  getSnapshot: (snapshotId: string) =>
    invoke<Record<string, unknown>>("get_snapshot_cmd", { snapshotId }),
  deleteSnapshot: (snapshotId: string) =>
    invoke<void>("delete_snapshot_cmd", { snapshotId }),
  listAgentQueue: (conversationId: string) =>
    invoke<Record<string, unknown>[]>("list_agent_queue_cmd", {
      conversationId,
    }),
  /**
   * Conversation-scoped Channel for queue mutations. Keep the Channel handler
   * alive while ConversationQueue is mounted; drop/unlisten on unmount.
   */
  subscribeConversationQueue: (args: {
    conversationId: string;
    onEvent: (event: import("./events").QueueChangedEvent) => void;
  }) =>
    subscribeConversationQueueChannel(args.conversationId, args.onEvent),
  /**
   * Conversation-scoped Channel for Sync/Conflict. Keep alive while the chat
   * (or tool window for that conversation) is open.
   */
  subscribeConversationSync: (args: {
    conversationId: string;
    onEvent: (event: import("./events").AgentTurnEvent) => void;
  }) =>
    subscribeConversationSyncChannel(args.conversationId, args.onEvent),
  cancelQueueItem: (itemId: string) =>
    invoke<Record<string, unknown>>("cancel_queue_item_cmd", { itemId }),
  removeQueueItem: (itemId: string) =>
    invoke<void>("remove_queue_item_cmd", { itemId }),
  listDiagnostics: (conversationId: string, limit?: number) =>
    invoke<unknown[]>("list_diagnostics_cmd", {
      conversationId,
      limit: limit ?? null,
    }),
  listTurnTimeline: (
    conversationId: string,
    turnId?: string | null,
    limit?: number,
  ) =>
    invoke<Record<string, unknown>[]>("list_turn_timeline_cmd", {
      conversationId,
      turnId: turnId ?? null,
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
  kernelGetProposal: (proposalId: string) =>
    invoke<Record<string, unknown>>("kernel_get_proposal", { proposalId }),
  kernelListPendingProposals: (conversationId?: string | null) =>
    invoke<Record<string, unknown>[]>("kernel_list_pending_proposals", {
      conversationId: conversationId ?? null,
    }),
  kernelDecideProposal: (proposalId: string, approve: boolean) =>
    invoke<Record<string, unknown>>("kernel_decide_proposal", { proposalId, approve }),
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
  kernelEnsureToolManifest: (toolId: string, toolName: string, surfaceId: string) =>
    invoke<import("@/types/application-kernel").ManifestRecord>(
      "kernel_ensure_tool_manifest",
      { toolId, toolName, surfaceId },
    ),
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

  getOnboardingState: () =>
    invoke<import("@/lib/onboarding/types").OnboardingState>("get_onboarding_state"),
  upsertTutorialProgress: (
    input: import("@/lib/onboarding/types").UpsertTutorialProgressInput,
  ) =>
    invoke<import("@/lib/onboarding/types").TutorialProgress>("upsert_tutorial_progress", {
      tutorialId: input.tutorialId,
      tutorialVersion: input.tutorialVersion,
      status: input.status,
      currentStepId: input.currentStepId ?? null,
      completedStepIds: input.completedStepIds ?? null,
    }),
  resetTutorialProgress: (tutorialId?: string | null) =>
    invoke<number>("reset_tutorial_progress", {
      tutorialId: tutorialId ?? null,
    }),
  seedTutorialSample: () =>
    invoke<import("@/lib/onboarding/types").TutorialSampleSeed>("seed_tutorial_sample"),
  cleanupTutorialSample: () => invoke<number>("cleanup_tutorial_sample"),
};
