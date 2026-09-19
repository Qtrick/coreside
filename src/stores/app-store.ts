import { create } from "zustand";
import type {
  AiStatus,
  AppInfo,
  DockIconConfig,
  EffectiveDockPresentation,
  ModelCatalog,
  ThemePreference,
  ToolChange,
  WallpaperConfig,
} from "@/types/agent";
import {
  DEFAULT_DOCK_ICON,
  DEFAULT_WALLPAPER,
  parseDockIconConfig,
  WallpaperKindSchema,
} from "@/types/agent";
import type { ChatMessage, Conversation } from "@/types/messages";
import type {
  ToolDefinition,
  ToolState,
  ToolSummary,
} from "@/types/tool";
import { persistenceScheduler } from "@/lib/persistence-scheduler";
import {
  api,
  TauriCommandError,
  listenAgentTurn,
  shouldApplyAgentTurnSync,
  shouldShowAppConflict,
} from "@/lib/tauri";
import type { AgentTurnEvent } from "@/lib/tauri";
import { MANUAL_DOCK_ICON_SELECTION_ENABLED } from "@/lib/branding/manual-dock-icon-capability";
import type { ToolMention } from "@/lib/mentions";
import { ensureReadableForeground } from "@/lib/readability/contrast";
import type { ActionLogMode } from "@/lib/action-log";
import { validateToolDefinition } from "@/lib/tool-schema";
import { normalizeCanonicalHex } from "@/lib/wallpaper-hex";
import {
  applyPreviewSurfaceOverlay,
  clearPreviewOverlaysForConversation,
  clearPreviewOverlaysMatching,
  type PreviewSurfaceOverlay,
} from "@/lib/preview/surface-overlay";
import {
  DEFAULT_CHAT_VIEW,
  shouldLoadChatMessages,
  shouldNavigateToChatNoOp,
  type AppView,
  type ChatViewState,
} from "@/lib/navigation";
import type {
  CreateProjectInput,
  DeleteProjectMode,
  Project,
  UpdateProjectInput,
} from "@/types/project";
import type { SurfaceDraftConflict } from "@/types/runtime-v2";
import { parseWallpaperJson } from "@/types/wallpaper";
import {
  applyTextDelta,
  createTurnLiveState,
  pendingTurnId,
  type TurnLiveState,
  type TurnLiveStatus,
} from "@/lib/turn-registry";

export type PendingToolChange = {
  conversationId: string;
  messageId: string;
  toolChange: ToolChange;
};

export type PendingSettingsChange = {
  conversationId: string;
  messageId: string;
  settingsChange: import("@/types/agent").SettingsChange;
};

export type PendingKernelProposal = {
  conversationId: string;
  messageId: string;
  proposalId: string;
  summary: string;
  impactSummary: string;
  risk: string;
  operations: unknown[];
};

export type AppConflict = {
  message: string;
  conflicts: string[];
  conversationId?: string | null;
};

type AppStore = {
  bootstrapped: boolean;
  bootError: string | null;
  bootstrapStatus: import("@/types/bootstrap").BootstrapStatus | null;
  appInfo: AppInfo | null;
  aiStatus: AiStatus | null;
  theme: ThemePreference;
  resolvedTheme: "light" | "dark";
  appearance: AppearancePalette;
  wallpaper: WallpaperConfig;
  globalWallpaperJson: string | null;
  interfaceTransparency: number;
  adaptiveWindowSizing: "smart" | "ask" | "off";
  chatToolSplitRatio: number;
  layoutMode: "wide" | "standard" | "compact";
  windowExpandStatus: string | null;
  /** Tool IDs already prompted for Ask-first expand this session. */
  windowExpandAskedToolIds: string[];
  sidebarCollapsed: boolean;
  view: AppView;
  chatViewState: Record<string, ChatViewState>;

  projects: Project[];
  activeProjectId: string | null;
  activeProject: Project | null;
  projectsExpanded: boolean;
  projectConversations: Conversation[];
  projectsLoading: boolean;
  projectsError: string | null;

  createProjectDialogOpen: boolean;
  editProjectDialogOpen: boolean;
  editProjectId: string | null;
  addChatsDialogOpen: boolean;
  addChatsProjectId: string | null;
  deleteProjectDialogOpen: boolean;
  deleteProjectId: string | null;
  renameConversationDialogOpen: boolean;
  renameConversationId: string | null;
  projectActionBusy: boolean;
  projectActionError: string | null;

  exportDialog: { toolId: string; toolName?: string } | null;
  providerSetupOpen: boolean;
  providerSetupEditing: import("@/types/providers").ProviderConnection | null;
  actionLogEnabled: boolean;
  actionLogMode: ActionLogMode;
  preferredModel: string;
  dockIcon: DockIconConfig;
  dockPresentation: EffectiveDockPresentation | null;
  dockStatusLabel: string | null;
  dockDevelopmentFallback: boolean;
  dockAdaptiveCapable: boolean;
  dockIconPending: boolean;
  dockIconError: string | null;
  /** Monotonic epoch so stale commit/apply responses cannot clobber newer choices. */
  dockIconEpoch: number;
  developerMode: boolean;
  modelCatalog: ModelCatalog | null;

  conversations: Conversation[];
  activeConversationId: string | null;
  messages: ChatMessage[];
  messagesLoading: boolean;
  messagesError: string | null;

  tools: ToolSummary[];
  activeToolId: string | null;
  activeTool: ToolDefinition | null;
  toolState: ToolState;
  toolVersionsLoading: boolean;
  /** Speculative Channel preview paint — cleared on Sync / interrupt / error. */
  previewSurfacesByKey: Record<string, PreviewSurfaceOverlay>;

  pendingToolChange: PendingToolChange | null;
  pendingSettingsChange: PendingSettingsChange | null;
  pendingKernelProposal: PendingKernelProposal | null;
  appConflict: AppConflict | null;
  surfaceDraftConflict: SurfaceDraftConflict | null;
  sending: boolean;
  /** Conversation that owns the in-flight turn; UI live state is scoped to this id. */
  sendingConversationId: string | null;
  sendError: string | null;
  agentActions: string[];
  /** Derived view of active-conversation stream text (MessageList compatibility). */
  streamingText: string | null;
  /** Per-turn Channel accumulation — survives navigation away from the chat. */
  turnsById: Record<string, TurnLiveState>;
  activeTurnIdByConversation: Record<string, string>;
  testingConnection: boolean;
  connectionTestMessage: string | null;

  bootstrap: () => Promise<void>;
  setTheme: (theme: ThemePreference) => Promise<void>;
  applyResolvedTheme: (resolved: "light" | "dark") => void;
  toggleSidebar: () => Promise<void>;
  navigateToChat: (id: string) => Promise<void>;
  navigateToProject: (projectId: string) => Promise<void>;
  navigateToProjects: () => void;
  navigateToMedia: () => void;
  navigateToSettings: () => void;
  navigateToAutomations: () => void;
  applyWorkspaceWallpaper: (wallpaperJson: string) => Promise<void>;
  previewWorkspaceWallpaper: (wallpaperJson: string) => void;
  revertWorkspaceWallpaper: () => void;
  applyProjectWallpaper: (projectId: string, wallpaperJson: string) => Promise<void>;
  previewInterfaceTransparency: (value: number) => void;
  commitInterfaceTransparency: (value: number) => Promise<void>;
  setAdaptiveWindowSizing: (mode: "smart" | "ask" | "off") => Promise<void>;
  setChatToolSplitRatio: (ratio: number) => Promise<void>;
  setLayoutMode: (mode: "wide" | "standard" | "compact") => void;
  clearWindowExpandStatus: () => void;
  maybeExpandForTool: (toolId: string) => Promise<void>;
  setChatDraft: (conversationId: string, draft: string) => void;
  setChatScrollTop: (conversationId: string, scrollTop: number) => void;
  setChatActiveToolId: (conversationId: string, toolId: string | null) => void;

  setCreateProjectDialogOpen: (open: boolean) => void;
  setEditProjectDialogOpen: (projectId: string | null) => void;
  setAddChatsDialogOpen: (projectId: string | null) => void;
  setDeleteProjectDialogOpen: (projectId: string | null) => void;
  setRenameConversationDialogOpen: (conversationId: string | null) => void;
  setProjectsExpanded: (expanded: boolean) => void;

  refreshProjects: () => Promise<void>;
  createProject: (input: CreateProjectInput) => Promise<Project | null>;
  openProject: (projectId: string) => Promise<void>;
  createChatInProject: (projectId: string) => Promise<void>;
  assignChats: (projectId: string, conversationIds: string[]) => Promise<void>;
  removeChatFromProject: (conversationId: string) => Promise<void>;
  deleteProject: (projectId: string, mode: DeleteProjectMode) => Promise<void>;
  archiveProject: (projectId: string) => Promise<void>;
  restoreProject: (projectId: string) => Promise<void>;
  updateProject: (
    projectId: string,
    input: UpdateProjectInput,
  ) => Promise<Project | null>;
  renameConversation: (conversationId: string, title: string) => Promise<void>;
  duplicateConversation: (conversationId: string) => Promise<void>;

  openExportDialog: (toolId: string, toolName?: string) => void;
  closeExportDialog: () => void;
  openProviderSetup: (
    editing?: import("@/types/providers").ProviderConnection | null,
  ) => void;
  closeProviderSetup: () => void;
  setPreferredModel: (modelId: string) => Promise<void>;
  setDockIcon: (preference: DockIconConfig) => Promise<void>;
  applyPersistedDockIcon: () => Promise<void>;
  clearDockIconError: () => void;
  setDeveloperMode: (enabled: boolean) => Promise<void>;
  refreshModelCatalog: () => Promise<void>;

  refreshConversations: () => Promise<void>;
  createConversation: () => Promise<void>;
  /** @deprecated Prefer navigateToChat */
  selectConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;
  branchConversation: (sourceMessageId: string, branchName?: string) => Promise<void>;

  refreshTools: () => Promise<void>;
  selectTool: (id: string | null) => Promise<void>;
  closeToolCanvas: () => void;
  openToolWindow: () => Promise<void>;
  undoTool: () => Promise<void>;
  updateToolState: (state: ToolState, persist?: boolean) => Promise<void>;
  flushToolState: (toolId?: string) => Promise<void>;

  sendMessage: (
    content: string,
    mentions?: ToolMention[],
    attachments?: import("@/types/attachments").StagedAttachment[],
    structuredUserInput?: {
      formId: string;
      applicationId?: string | null;
      surfaceId?: string | null;
      fields: Record<string, unknown>;
    } | null,
  ) => Promise<boolean>;
  cancelRequest: () => Promise<void>;
  retryLastFailed: () => Promise<void>;
  editAndResendMessage: (messageId: string, content: string) => Promise<void>;
  applyPendingToolChange: () => Promise<void>;
  discardPendingToolChange: () => Promise<void>;
  applyPendingSettingsChange: () => Promise<void>;
  discardPendingSettingsChange: () => Promise<void>;
  applyPendingKernelProposal: () => Promise<void>;
  discardPendingKernelProposal: () => Promise<void>;
  clearAppConflict: () => void;
  setSurfaceDraftConflict: (conflict: SurfaceDraftConflict | null) => void;
  clearSurfaceDraftConflict: () => void;
  resolveSurfaceDraftConflict: (
    action: "keep" | "apply" | "cancel",
  ) => Promise<void>;
  reloadActiveSurfaces: () => Promise<void>;

  refreshAiStatus: () => Promise<void>;
  testConnection: () => Promise<void>;
  clearConversations: () => Promise<void>;
  clearTools: () => Promise<void>;

  loadToolWindow: (toolId: string) => Promise<void>;
};

let agentTurnSyncAttached = false;
/** Active conversation-scoped Sync Channel unlisten (main / tool window). */
let conversationSyncUnlisten: (() => void) | null = null;
let conversationSyncConversationId: string | null = null;

type SyncListenerGet = () => {
  reloadActiveSurfaces: () => Promise<void>;
  activeConversationId: string | null;
  activeToolId: string | null;
  previewSurfacesByKey: Record<string, PreviewSurfaceOverlay>;
};

type SyncListenerSet = (partial: {
  appConflict?: AppConflict | null;
  previewSurfacesByKey?: Record<string, PreviewSurfaceOverlay>;
}) => void;

/** Shared Sync/Conflict apply path for scoped Channel + residual global bus. */
function applySyncOrConflictEvent(
  get: SyncListenerGet,
  set: SyncListenerSet,
  event: AgentTurnEvent,
) {
  if (
    event.kind === "text"
    || event.kind === "action"
    || event.kind === "error"
    || event.kind === "operation"
    || event.kind === "previewSurface"
  ) {
    return;
  }
  const scope = {
    activeConversationId: get().activeConversationId,
    activeToolId: get().activeToolId,
  };
  if (event.kind === "conflict") {
    // Failed durable commit — drop speculative paint so overlays cannot look applied.
    const previewSurfacesByKey = clearPreviewOverlaysMatching(
      get().previewSurfacesByKey,
      {
        conversationId: event.conversationId,
        toolIds: [],
        surfaceIds: [],
      },
    );
    if (
      !shouldShowAppConflict(
        { conversationId: event.conversationId },
        scope.activeConversationId,
      )
    ) {
      if (previewSurfacesByKey !== get().previewSurfacesByKey) {
        set({ previewSurfacesByKey });
      }
      return;
    }
    set({
      appConflict: {
        message: event.message,
        conflicts: event.conflicts,
        conversationId: event.conversationId ?? null,
      },
      previewSurfacesByKey,
    });
    return;
  }
  if (event.kind === "sync") {
    set({
      previewSurfacesByKey: clearPreviewOverlaysMatching(
        get().previewSurfacesByKey,
        {
          conversationId: event.conversationId,
          toolIds: event.toolIds ?? [],
          surfaceIds: event.surfaceIds ?? [],
        },
      ),
    });
    if (!shouldApplyAgentTurnSync(event, scope)) {
      return;
    }
    void get().reloadActiveSurfaces();
  }
}

/**
 * Keep a conversation-scoped Sync Channel alive while a chat (or tool surface
 * bound to that conversation) is open. Primary Sync path.
 */
function ensureConversationSyncSubscription(
  conversationId: string | null,
  get: SyncListenerGet,
  set: SyncListenerSet,
) {
  const trimmed = (conversationId ?? "").trim();
  if (trimmed === (conversationSyncConversationId ?? "")) {
    return;
  }
  conversationSyncUnlisten?.();
  conversationSyncUnlisten = null;
  conversationSyncConversationId = trimmed || null;
  if (!trimmed) return;
  void api
    .subscribeConversationSync({
      conversationId: trimmed,
      onEvent: (event) => {
        applySyncOrConflictEvent(get, set, event);
      },
    })
    .then((stop) => {
      if (conversationSyncConversationId !== trimmed) {
        stop();
        return;
      }
      conversationSyncUnlisten = stop;
    })
    .catch(() => {
      // Best-effort; residual global listenAgentTurn remains.
    });
}

/** Mark a conversation's active turn terminal; no-op if none. */
function finalizeTurnInState(
  state: {
    turnsById: Record<string, TurnLiveState>;
    activeTurnIdByConversation: Record<string, string>;
    previewSurfacesByKey: Record<string, PreviewSurfaceOverlay>;
  },
  conversationId: string,
  status: TurnLiveStatus,
  error: string | null = null,
): Partial<{
  turnsById: Record<string, TurnLiveState>;
  previewSurfacesByKey: Record<string, PreviewSurfaceOverlay>;
}> {
  const turnId = state.activeTurnIdByConversation[conversationId];
  const previewSurfacesByKey = clearPreviewOverlaysForConversation(
    state.previewSurfacesByKey,
    conversationId,
  );
  const previewPatch =
    previewSurfacesByKey === state.previewSurfacesByKey
      ? {}
      : { previewSurfacesByKey };
  if (!turnId) return previewPatch;
  const turn = state.turnsById[turnId];
  if (!turn) return previewPatch;
  return {
    turnsById: {
      ...state.turnsById,
      [turnId]: {
        ...turn,
        status,
        error: error ?? turn.error,
      },
    },
    ...previewPatch,
  };
}

/**
 * Resolve the registry key for a Channel event. Prefer event.turnId; migrate
 * from a provisional pending:* entry when the real id arrives.
 */
function upsertTurnFromChannel(
  state: {
    turnsById: Record<string, TurnLiveState>;
    activeTurnIdByConversation: Record<string, string>;
  },
  conversationId: string,
  eventTurnId: string | null | undefined,
): {
  turnId: string;
  turn: TurnLiveState;
  turnsById: Record<string, TurnLiveState>;
  activeTurnIdByConversation: Record<string, string>;
} {
  const currentId =
    state.activeTurnIdByConversation[conversationId] ?? pendingTurnId(conversationId);
  const turnId =
    typeof eventTurnId === "string" && eventTurnId.length > 0 ? eventTurnId : currentId;

  let turnsById = state.turnsById;
  let turn = turnsById[turnId];

  if (!turn && turnId !== currentId && turnsById[currentId]) {
    // Migrate provisional pending:* → real turnId from Channel text.
    const pending = turnsById[currentId];
    turnsById = { ...turnsById };
    delete turnsById[currentId];
    turn = { ...pending, turnId };
    turnsById[turnId] = turn;
  } else if (!turn) {
    turn = createTurnLiveState(turnId, conversationId);
    turnsById = { ...turnsById, [turnId]: turn };
  }

  return {
    turnId,
    turn,
    turnsById,
    activeTurnIdByConversation: {
      ...state.activeTurnIdByConversation,
      [conversationId]: turnId,
    },
  };
}

function attachAgentTurnSyncListener(
  get: SyncListenerGet,
  set: SyncListenerSet,
) {
  if (agentTurnSyncAttached) return;
  agentTurnSyncAttached = true;
  // Residual global bus: private kinds ignored; Sync/Conflict only when Rust
  // fell back because no scoped subscriber delivered.
  void listenAgentTurn((event) => {
    applySyncOrConflictEvent(get, set, event);
  });
}

function resolveTheme(theme: ThemePreference): "light" | "dark" {
  if (theme === "system") {
    if (typeof window !== "undefined" && window.matchMedia) {
      return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
    return "light";
  }
  return theme;
}

function mixHover(hex: string): string {
  let h = hex.trim().replace("#", "");
  if (h.length === 3) {
    h = `${h[0]}${h[0]}${h[1]}${h[1]}${h[2]}${h[2]}`;
  }
  if (h.length !== 6 || !/^[0-9a-fA-F]{6}$/.test(h)) return hex;
  const n = (i: number) => parseInt(h.slice(i, i + 2), 16);
  const mix = (c: number) => Math.round((c * 88) / 100);
  const r = mix(n(0));
  const g = mix(n(2));
  const b = mix(n(4));
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

export type AccentPalette = {
  accentPrimaryLight: string;
  accentPrimaryDark: string;
  accentSecondaryLight: string;
  accentSecondaryDark: string;
};

export type SurfacePalette = {
  backgroundLight: string;
  backgroundDark: string;
  surfaceLight: string;
  surfaceDark: string;
  surfaceMutedLight: string;
  surfaceMutedDark: string;
  borderLight: string;
  borderDark: string;
  textPrimaryLight: string;
  textPrimaryDark: string;
  textSecondaryLight: string;
  textSecondaryDark: string;
};

export type AppearancePalette = AccentPalette & SurfacePalette;

const DEFAULT_ACCENTS: AccentPalette = {
  accentPrimaryLight: "#2f8f63",
  accentPrimaryDark: "#69c994",
  accentSecondaryLight: "#d38b3d",
  accentSecondaryDark: "#e0a158",
};

const DEFAULT_SURFACES: SurfacePalette = {
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
};

const DEFAULT_APPEARANCE: AppearancePalette = {
  ...DEFAULT_ACCENTS,
  ...DEFAULT_SURFACES,
};

export function applyAppearanceCssVars(
  resolved: "light" | "dark",
  palette: AppearancePalette,
) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const pick = <T extends string>(light: T, dark: T) =>
    resolved === "dark" ? dark : light;

  const background = pick(palette.backgroundLight, palette.backgroundDark);
  const surface = pick(palette.surfaceLight, palette.surfaceDark);
  // Links and accents are drawn on surfaces — clamp so dark theme gets lighter
  // hues and light theme gets darker hues (e.g. blue gradients stay readable).
  const primary = ensureReadableForeground(
    pick(palette.accentPrimaryLight, palette.accentPrimaryDark),
    surface,
    4.5,
  );
  const secondary = ensureReadableForeground(
    pick(palette.accentSecondaryLight, palette.accentSecondaryDark),
    surface,
    3.5,
  );
  const textPrimary = ensureReadableForeground(
    pick(palette.textPrimaryLight, palette.textPrimaryDark),
    background,
    4.5,
  );
  const textSecondary = ensureReadableForeground(
    pick(palette.textSecondaryLight, palette.textSecondaryDark),
    background,
    4.5,
  );

  root.style.setProperty("--accent-primary", primary);
  root.style.setProperty("--accent-primary-hover", mixHover(primary));
  root.style.setProperty("--accent-secondary", secondary);
  // Keep success in sync with the active accent so leftover green badges don't linger.
  root.style.setProperty("--success", primary);
  root.style.setProperty("--background", background);
  root.style.setProperty("--surface", surface);
  root.style.setProperty(
    "--surface-muted",
    pick(palette.surfaceMutedLight, palette.surfaceMutedDark),
  );
  root.style.setProperty(
    "--border",
    pick(palette.borderLight, palette.borderDark),
  );
  root.style.setProperty("--text-primary", textPrimary);
  root.style.setProperty("--text-secondary", textSecondary);
  root.style.setProperty("--core-accent", primary);
  root.style.setProperty("--core-text-primary", textPrimary);
  root.style.setProperty("--core-text-secondary", textSecondary);
}

/** @deprecated Use applyAppearanceCssVars */
export const applyAccentCssVars = applyAppearanceCssVars;

function wallpaperFromSettings(settings: {
  wallpaper?: WallpaperConfig | null;
}): WallpaperConfig {
  const w = settings.wallpaper;
  if (!w || typeof w !== "object") return { ...DEFAULT_WALLPAPER };
  const kind = WallpaperKindSchema.safeParse(w.kind);
  if (!kind.success) return { ...DEFAULT_WALLPAPER };
  const clampNum = (n: unknown, min: number, max: number) =>
    typeof n === "number" && Number.isFinite(n)
      ? Math.min(max, Math.max(min, n))
      : undefined;
  return {
    kind: kind.data,
    color: typeof w.color === "string" ? w.color : undefined,
    secondaryColor:
      typeof w.secondaryColor === "string" ? w.secondaryColor : undefined,
    speed: clampNum(w.speed, 0.25, 3),
    density: clampNum(w.density, 0.1, 1),
    opacity: clampNum(w.opacity, 0.05, 0.85),
  };
}

/**
 * Optimistic CSS/renderer state from a proposed wallpaperJson payload.
 * Mirrors Rust `resolve_wallpaper_pair`: schema → wallpaperJson + none;
 * legacy kind → wallpaper + cleared wallpaperJson; empty/none → cleared.
 */
function optimisticWallpaperFromJson(wallpaperJson: string): {
  wallpaper: WallpaperConfig;
  globalWallpaperJson: string | null;
} {
  const trimmed = wallpaperJson.trim();
  if (!trimmed) {
    return { wallpaper: { ...DEFAULT_WALLPAPER }, globalWallpaperJson: null };
  }
  const parsed = parseWallpaperJson(trimmed);
  if (parsed.format === "none") {
    return { wallpaper: { ...DEFAULT_WALLPAPER }, globalWallpaperJson: null };
  }
  if (parsed.format === "legacy") {
    return { wallpaper: parsed.config, globalWallpaperJson: null };
  }
  return {
    wallpaper: { ...DEFAULT_WALLPAPER },
    globalWallpaperJson: trimmed,
  };
}

function rememberCommittedWallpaper(input: {
  wallpaper: WallpaperConfig;
  globalWallpaperJson: string | null;
}) {
  committedWallpaper = { ...input.wallpaper };
  committedGlobalWallpaperJson = input.globalWallpaperJson;
}

/** Bootstrap / settings reload: commit truth matches current apply generation. */
function syncCommittedWallpaperFromSettings(input: {
  wallpaper: WallpaperConfig;
  globalWallpaperJson: string | null;
}) {
  rememberCommittedWallpaper(input);
  wallpaperCommittedGen = wallpaperApplyGen;
}

function appearanceFromSettings(settings: Partial<AppearancePalette>): AppearancePalette {
  return {
    accentPrimaryLight:
      settings.accentPrimaryLight ?? DEFAULT_APPEARANCE.accentPrimaryLight,
    accentPrimaryDark:
      settings.accentPrimaryDark ?? DEFAULT_APPEARANCE.accentPrimaryDark,
    accentSecondaryLight:
      settings.accentSecondaryLight ?? DEFAULT_APPEARANCE.accentSecondaryLight,
    accentSecondaryDark:
      settings.accentSecondaryDark ?? DEFAULT_APPEARANCE.accentSecondaryDark,
    backgroundLight:
      settings.backgroundLight ?? DEFAULT_APPEARANCE.backgroundLight,
    backgroundDark:
      settings.backgroundDark ?? DEFAULT_APPEARANCE.backgroundDark,
    surfaceLight: settings.surfaceLight ?? DEFAULT_APPEARANCE.surfaceLight,
    surfaceDark: settings.surfaceDark ?? DEFAULT_APPEARANCE.surfaceDark,
    surfaceMutedLight:
      settings.surfaceMutedLight ?? DEFAULT_APPEARANCE.surfaceMutedLight,
    surfaceMutedDark:
      settings.surfaceMutedDark ?? DEFAULT_APPEARANCE.surfaceMutedDark,
    borderLight: settings.borderLight ?? DEFAULT_APPEARANCE.borderLight,
    borderDark: settings.borderDark ?? DEFAULT_APPEARANCE.borderDark,
    textPrimaryLight:
      settings.textPrimaryLight ?? DEFAULT_APPEARANCE.textPrimaryLight,
    textPrimaryDark:
      settings.textPrimaryDark ?? DEFAULT_APPEARANCE.textPrimaryDark,
    textSecondaryLight:
      settings.textSecondaryLight ?? DEFAULT_APPEARANCE.textSecondaryLight,
    textSecondaryDark:
      settings.textSecondaryDark ?? DEFAULT_APPEARANCE.textSecondaryDark,
  };
}

/** Last successfully persisted transparency — used to roll back failed commits. */
let committedInterfaceTransparency = 20;
/** Monotonic commit generation — superseded in-flight commits must not clobber newer UI. */
let interfaceTransparencyCommitGen = 0;
/**
 * Highest generation that successfully persisted. Stale successes still advance this so a
 * newer failed commit rolls back to DB-truth from an overlapping older write, not the
 * pre-overlap snapshot (same race class as wallpaperCommittedGen).
 */
let interfaceTransparencyCommittedGen = 0;
/**
 * In-flight same-target promise reuse (pointerup + blur / duplicate preset).
 * Different targets start a new commit; only the latest gen may paint Zustand.
 */
let interfaceTransparencyInFlight: {
  value: number;
  promise: Promise<void>;
} | null = null;

/** Last successfully persisted workspace wallpaper — rollback target for failed preview applies. */
let committedWallpaper: WallpaperConfig = { ...DEFAULT_WALLPAPER };
let committedGlobalWallpaperJson: string | null = null;
/** Monotonic apply generation — stale overlapping applies must not clobber newer preview/commit. */
let wallpaperApplyGen = 0;
/**
 * Highest generation that successfully persisted. Stale successes still advance this so a
 * newer failed apply rolls back to DB-truth from an overlapping older write, not the
 * pre-overlap snapshot.
 */
let wallpaperCommittedGen = 0;

export const useAppStore = create<AppStore>((set, get) => ({
  bootstrapped: false,
  bootError: null,
  bootstrapStatus: null,
  appInfo: null,
  aiStatus: null,
  theme: "system",
  resolvedTheme: "light",
  appearance: { ...DEFAULT_APPEARANCE },
  wallpaper: { ...DEFAULT_WALLPAPER },
  globalWallpaperJson: null,
  interfaceTransparency: 20,
  adaptiveWindowSizing: "smart",
  chatToolSplitRatio: 0.5,
  layoutMode: "wide",
  windowExpandStatus: null,
  windowExpandAskedToolIds: [],
  sidebarCollapsed: false,
  view: { ...DEFAULT_CHAT_VIEW },
  chatViewState: {},

  projects: [],
  activeProjectId: null,
  activeProject: null,
  projectsExpanded: true,
  projectConversations: [],
  projectsLoading: false,
  projectsError: null,

  createProjectDialogOpen: false,
  editProjectDialogOpen: false,
  editProjectId: null,
  addChatsDialogOpen: false,
  addChatsProjectId: null,
  deleteProjectDialogOpen: false,
  deleteProjectId: null,
  renameConversationDialogOpen: false,
  renameConversationId: null,
  projectActionBusy: false,
  projectActionError: null,

  exportDialog: null,
  providerSetupOpen: false,
  providerSetupEditing: null,
  actionLogEnabled: false,
  actionLogMode: "off" as const,
  preferredModel: "auto",
  dockIcon: { ...DEFAULT_DOCK_ICON },
  dockPresentation: null,
  dockStatusLabel: null,
  dockDevelopmentFallback: false,
  dockAdaptiveCapable: false,
  dockIconPending: false,
  dockIconError: null,
  dockIconEpoch: 0,
  developerMode: false,
  modelCatalog: null,

  conversations: [],
  activeConversationId: null,
  messages: [],
  messagesLoading: false,
  messagesError: null,

  tools: [],
  activeToolId: null,
  activeTool: null,
  toolState: {},
  toolVersionsLoading: false,
  previewSurfacesByKey: {},

  pendingToolChange: null,
  pendingSettingsChange: null,
  pendingKernelProposal: null,
  appConflict: null,
  surfaceDraftConflict: null,
  sending: false,
  sendingConversationId: null,
  sendError: null,
  agentActions: [],
  streamingText: null,
  turnsById: {},
  activeTurnIdByConversation: {},
  testingConnection: false,
  connectionTestMessage: null,

  bootstrap: async () => {
    try {
      const bootstrapStatus = await api.getBootstrapStatus();
      if (bootstrapStatus.status === "recoveryRequired") {
        set({
          bootstrapped: true,
          bootError: null,
          bootstrapStatus,
          appInfo: await api.getAppInfo().catch(() => null),
        });
        return;
      }

      const [appInfo, aiStatus, settings, conversations, tools, projects] =
        await Promise.all([
          api.getAppInfo(),
          api.getAiStatus(),
          api.getSettings(),
          api.listConversations(),
          api.listTools(),
          api.listProjects(),
        ]);

      // Lazy catalog: only fetch when disclosure allows provider model listing.
      const modelCatalog = aiStatus.disclosure?.showProviderCatalog
        ? await api.getModelCatalog()
        : null;

      const theme = settings.theme ?? "system";
      const resolved = resolveTheme(theme);
      const appearance = appearanceFromSettings(settings);
      const wallpaper = wallpaperFromSettings(settings);
      const interfaceTransparency =
        typeof settings.interfaceTransparency === "number"
          ? Math.min(60, Math.max(0, Math.round(settings.interfaceTransparency)))
          : 20;
      committedInterfaceTransparency = interfaceTransparency;
      interfaceTransparencyCommittedGen = interfaceTransparencyCommitGen;
      syncCommittedWallpaperFromSettings({
        wallpaper,
        globalWallpaperJson: settings.wallpaperJson ?? null,
      });
      set({
        bootstrapped: true,
        bootError: null,
        bootstrapStatus: { status: "ready" },
        appInfo,
        aiStatus,
        theme,
        resolvedTheme: resolved,
        appearance,
        wallpaper,
        globalWallpaperJson: settings.wallpaperJson ?? null,
        interfaceTransparency,
        adaptiveWindowSizing:
          settings.adaptiveWindowSizing === "ask" ||
          settings.adaptiveWindowSizing === "off" ||
          settings.adaptiveWindowSizing === "smart"
            ? settings.adaptiveWindowSizing
            : "smart",
        chatToolSplitRatio:
          typeof settings.chatToolSplitRatio === "number"
            ? Math.min(0.72, Math.max(0.28, settings.chatToolSplitRatio))
            : 0.5,
        sidebarCollapsed: settings.sidebarCollapsed ?? false,
        preferredModel: settings.preferredModel ?? modelCatalog?.selected ?? "auto",
        dockIcon: parseDockIconConfig(settings.dockIcon ?? DEFAULT_DOCK_ICON),
        dockIconError: null,
        developerMode: Boolean(settings.developerMode),
        actionLogEnabled: Boolean(settings.actionLogEnabled) || settings.actionLogMode === "always" || settings.actionLogMode === "intelligent",
        actionLogMode:
          settings.actionLogMode === "always" ||
          settings.actionLogMode === "intelligent" ||
          settings.actionLogMode === "off"
            ? settings.actionLogMode
            : settings.actionLogEnabled
              ? "always"
              : "off",
        modelCatalog,
        conversations,
        tools,
        projects,
      });

      attachAgentTurnSyncListener(get, set);

      if (conversations.length > 0) {
        await get().navigateToChat(conversations[0].id);
      }
    } catch (error) {
      set({
        bootstrapped: true,
        bootError:
          error instanceof Error ? error.message : "Failed to start Coreside",
      });
    }
  },

  setTheme: async (theme) => {
    await api.setSetting("theme", theme);
    const resolved = resolveTheme(theme);
    set({ theme, resolvedTheme: resolved });
  },

  applyResolvedTheme: (resolved) => {
    set({ resolvedTheme: resolved });
  },

  toggleSidebar: async () => {
    const next = !get().sidebarCollapsed;
    set({ sidebarCollapsed: next });
    await api.setSetting("sidebarCollapsed", next);
  },

  setChatDraft: (conversationId, draft) => {
    set((state) => ({
      chatViewState: {
        ...state.chatViewState,
        [conversationId]: {
          ...state.chatViewState[conversationId],
          draft,
        },
      },
    }));
  },

  setChatScrollTop: (conversationId, scrollTop) => {
    set((state) => ({
      chatViewState: {
        ...state.chatViewState,
        [conversationId]: {
          ...state.chatViewState[conversationId],
          scrollTop,
        },
      },
    }));
  },

  setChatActiveToolId: (conversationId, toolId) => {
    set((state) => ({
      chatViewState: {
        ...state.chatViewState,
        [conversationId]: {
          ...state.chatViewState[conversationId],
          activeToolId: toolId,
        },
      },
    }));
  },

  navigateToChat: async (id) => {
    const trimmed = id.trim();
    if (!trimmed) {
      set({
        view: { kind: "chat", conversationId: null },
        activeConversationId: null,
        appConflict: null,
      });
      ensureConversationSyncSubscription(null, get, set);
      return;
    }

    const state = get();
    if (shouldNavigateToChatNoOp(state.view, state.activeConversationId, trimmed)) {
      return;
    }

    const needLoad = shouldLoadChatMessages(
      state.activeConversationId,
      trimmed,
      state.messages,
    );
    const savedToolId = state.chatViewState[trimmed]?.activeToolId ?? null;

    const activeTurnId = state.activeTurnIdByConversation[trimmed];
    const activeTurn = activeTurnId ? state.turnsById[activeTurnId] : undefined;
    const restoreLive =
      activeTurn?.status === "streaming"
        ? {
            streamingText: activeTurn.text || null,
            agentActions: activeTurn.actions,
            sendError: activeTurn.error,
          }
        : {
            // Derived live UI is conversation-scoped; clear when the target
            // chat has no streaming turn (registry keeps background truth).
            streamingText: null,
            agentActions:
              state.sendingConversationId === trimmed ? state.agentActions : [],
          };

    // Drop conflict banners that belong to another conversation.
    const keepConflict =
      state.appConflict &&
      shouldShowAppConflict(
        { conversationId: state.appConflict.conversationId },
        trimmed,
      )
        ? state.appConflict
        : null;

    set({
      view: { kind: "chat", conversationId: trimmed },
      activeConversationId: trimmed,
      activeProjectId:
        state.conversations.find((c) => c.id === trimmed)?.projectId ?? null,
      appConflict: keepConflict,
      ...restoreLive,
      ...(needLoad
        ? {
            messagesLoading: true,
            messagesError: null,
            pendingToolChange: null,
          }
        : {}),
    });

    ensureConversationSyncSubscription(trimmed, get, set);

    if (needLoad) {
      try {
        const messages = await api.getMessages(trimmed);
        let pending: PendingToolChange | null = null;
        let kernelProposal: PendingKernelProposal | null = null;
        for (let i = messages.length - 1; i >= 0; i -= 1) {
          const meta = messages[i].metadata as
            | {
                toolChange?: ToolChange;
                pending?: boolean;
                toolChangeStatus?: string;
                runtimeV2?: {
                  proposalId?: string;
                  risk?: string;
                  impactSummary?: string;
                  summary?: string;
                  operations?: unknown[];
                  status?: string;
                };
                kernelProposalStatus?: string;
              }
            | null
            | undefined;
          const isPending =
            !!meta?.toolChange &&
            (meta.toolChangeStatus === "pending" || meta.pending === true);
          if (isPending && meta?.toolChange && !pending) {
            pending = {
              conversationId: trimmed,
              messageId: messages[i].id,
              toolChange: meta.toolChange,
            };
          }
          const rv = meta?.runtimeV2;
          if (
            !kernelProposal &&
            rv?.proposalId &&
            Array.isArray(rv.operations) &&
            rv.operations.length > 0 &&
            rv.status !== "discarded" &&
            rv.status !== "applied" &&
            meta?.kernelProposalStatus !== "discarded" &&
            meta?.kernelProposalStatus !== "applied"
          ) {
            kernelProposal = {
              conversationId: trimmed,
              messageId: messages[i].id,
              proposalId: rv.proposalId,
              summary: rv.summary ?? "Proposed application change",
              impactSummary: rv.impactSummary ?? "",
              risk: rv.risk ?? "strong",
              operations: rv.operations,
            };
          }
        }
        // The user may have navigated again while this load was in flight.
        // A late response must never overwrite the chat now on screen.
        if (get().activeConversationId !== trimmed) return;
        set({
          messages,
          messagesLoading: false,
          pendingToolChange: pending,
          pendingKernelProposal: kernelProposal,
        });
      } catch (error) {
        if (get().activeConversationId !== trimmed) return;
        set({
          messages: [],
          messagesLoading: false,
          messagesError:
            error instanceof Error
              ? error.message
              : "Failed to load conversation",
        });
      }
    }

    if (savedToolId) {
      await get().selectTool(savedToolId);
    } else {
      get().closeToolCanvas();
    }
  },

  navigateToProject: async (projectId) => {
    set({
      view: { kind: "project", projectId },
      activeProjectId: projectId,
      projectActionError: null,
    });
    try {
      const [project, projectConversations] = await Promise.all([
        api.touchProjectOpened(projectId),
        api.listProjectConversations(projectId),
      ]);
      set((state) => ({
        activeProject: project,
        projectConversations,
        projects: state.projects.map((p) =>
          p.id === project.id ? project : p,
        ),
      }));
    } catch (error) {
      set({
        projectActionError:
          error instanceof Error ? error.message : "Failed to open project",
      });
    }
  },

  navigateToProjects: () => {
    set({ view: { kind: "projects" }, projectActionError: null });
  },

  navigateToMedia: () => {
    set({ view: { kind: "media" } });
  },

  navigateToSettings: () => {
    set({ view: { kind: "settings" } });
  },

  navigateToAutomations: () => {
    set({ view: { kind: "automations" } });
  },

  applyWorkspaceWallpaper: async (wallpaperJson) => {
    // Preview-first (same pattern as transparency): paint local CSS/renderer
    // immediately, persist via atomic IPC, roll back to last committed on failure.
    const preview = optimisticWallpaperFromJson(wallpaperJson);
    const gen = ++wallpaperApplyGen;
    set(preview);
    try {
      const settings = await api.setWorkspaceAppearance({ wallpaperJson });
      const next = {
        wallpaper: wallpaperFromSettings(settings),
        globalWallpaperJson: settings.wallpaperJson ?? null,
      };
      // Record DB truth for any success that isn't older than an already-committed
      // newer gen (overlapping A then B: A may finish after B started).
      if (gen >= wallpaperCommittedGen) {
        wallpaperCommittedGen = gen;
        rememberCommittedWallpaper(next);
      }
      if (gen === wallpaperApplyGen) {
        set(next);
      }
    } catch (err) {
      // Roll back to live module committed — never a start-of-apply snapshot, which
      // can be stale when an overlapping older apply already persisted.
      if (gen === wallpaperApplyGen) {
        set({
          wallpaper: { ...committedWallpaper },
          globalWallpaperJson: committedGlobalWallpaperJson,
        });
      }
      throw err;
    }
  },

  previewWorkspaceWallpaper: (wallpaperJson) => {
    const preview = optimisticWallpaperFromJson(wallpaperJson);
    set(preview);
  },

  revertWorkspaceWallpaper: () => {
    set({
      wallpaper: { ...committedWallpaper },
      globalWallpaperJson: committedGlobalWallpaperJson,
    });
  },

  applyProjectWallpaper: async (projectId, wallpaperJson) => {
    const project = await api.setProjectWallpaper(projectId, wallpaperJson);
    set((state) => ({
      projects: state.projects.map((p) => (p.id === project.id ? project : p)),
      activeProject:
        state.activeProject?.id === project.id ? project : state.activeProject,
    }));
  },

  previewInterfaceTransparency: (value) => {
    const next = Math.min(60, Math.max(0, Math.round(value)));
    set({ interfaceTransparency: next });
  },

  commitInterfaceTransparency: async (value) => {
    const next = Math.min(60, Math.max(0, Math.round(value)));
    // Coalesce pointerup+blur / duplicate preset clicks onto the same value.
    if (next === committedInterfaceTransparency) {
      set({ interfaceTransparency: next });
      return;
    }
    if (interfaceTransparencyInFlight?.value === next) {
      set({ interfaceTransparency: next });
      return interfaceTransparencyInFlight.promise;
    }
    const gen = ++interfaceTransparencyCommitGen;
    set({ interfaceTransparency: next });
    const promise = (async () => {
      try {
        const settings = await api.setWorkspaceAppearance({
          interfaceTransparency: next,
        });
        const saved =
          typeof settings.interfaceTransparency === "number"
            ? Math.min(
                60,
                Math.max(0, Math.round(settings.interfaceTransparency)),
              )
            : next;
        // Record DB truth for any success that isn't older than an already-committed
        // newer gen (overlapping A then B: A may finish after B started).
        if (gen >= interfaceTransparencyCommittedGen) {
          interfaceTransparencyCommittedGen = gen;
          committedInterfaceTransparency = saved;
        }
        // Only the latest generation may paint Zustand over a newer preview.
        if (gen === interfaceTransparencyCommitGen) {
          set({ interfaceTransparency: saved });
        }
      } catch (err) {
        // Roll back to live module committed — never a start-of-commit snapshot.
        if (gen === interfaceTransparencyCommitGen) {
          set({ interfaceTransparency: committedInterfaceTransparency });
        }
        throw err;
      } finally {
        if (interfaceTransparencyInFlight?.value === next) {
          interfaceTransparencyInFlight = null;
        }
      }
    })();
    interfaceTransparencyInFlight = { value: next, promise };
    return promise;
  },

  setAdaptiveWindowSizing: async (mode) => {
    set({
      adaptiveWindowSizing: mode,
      // Mode change resets Ask-first session memory.
      windowExpandAskedToolIds: [],
    });
    await api.setSetting("adaptiveWindowSizing", mode);
  },

  setChatToolSplitRatio: async (ratio) => {
    const next = Math.min(0.72, Math.max(0.28, ratio));
    set({ chatToolSplitRatio: next });
    await api.setSetting("chatToolSplitRatio", next);
  },

  setLayoutMode: (mode) => {
    if (get().layoutMode === mode) return;
    set({ layoutMode: mode });
  },

  clearWindowExpandStatus: () => set({ windowExpandStatus: null }),

  maybeExpandForTool: async (toolId) => {
    const mode = get().adaptiveWindowSizing;
    if (mode === "off") return;
    const reducedMotion =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (mode === "ask") {
      const asked = get().windowExpandAskedToolIds;
      if (asked.includes(toolId)) return;
      set({
        windowExpandStatus: `ask:${toolId}`,
        windowExpandAskedToolIds: [...asked, toolId],
      });
      return;
    }
    try {
      const decision = await api.windowOrchestratorExpand({
        toolId,
        minUsefulWidth: 520,
        minUsefulHeight: 420,
        direction: "right",
        reducedMotion,
      });
      if (decision.decision === "expand") {
        set({
          windowExpandStatus: `Expanded the window to fit the tool.`,
        });
      }
    } catch {
      // Non-blocking — tools still reflow / use compact mode.
    }
  },

  setCreateProjectDialogOpen: (open) =>
    set({ createProjectDialogOpen: open, projectActionError: null }),

  setEditProjectDialogOpen: (projectId) =>
    set({
      editProjectDialogOpen: Boolean(projectId),
      editProjectId: projectId,
      projectActionError: null,
    }),

  setAddChatsDialogOpen: (projectId) =>
    set({
      addChatsDialogOpen: Boolean(projectId),
      addChatsProjectId: projectId,
      projectActionError: null,
    }),

  setDeleteProjectDialogOpen: (projectId) =>
    set({
      deleteProjectDialogOpen: Boolean(projectId),
      deleteProjectId: projectId,
      projectActionError: null,
    }),

  setRenameConversationDialogOpen: (conversationId) =>
    set({
      renameConversationDialogOpen: Boolean(conversationId),
      renameConversationId: conversationId,
      projectActionError: null,
    }),

  setProjectsExpanded: (expanded) => set({ projectsExpanded: expanded }),

  refreshProjects: async () => {
    set({ projectsLoading: true, projectsError: null });
    try {
      const projects = await api.listProjects(true);
      set({ projects, projectsLoading: false });
    } catch (error) {
      set({
        projectsLoading: false,
        projectsError:
          error instanceof Error ? error.message : "Failed to load projects",
      });
    }
  },

  createProject: async (input) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const project = await api.createProject(input);
      set((state) => ({
        projects: [project, ...state.projects],
        createProjectDialogOpen: false,
        projectActionBusy: false,
      }));
      await get().navigateToProject(project.id);
      return project;
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to create project",
      });
      return null;
    }
  },

  openProject: async (projectId) => {
    await get().navigateToProject(projectId);
  },

  createChatInProject: async (projectId) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const conversation = await api.createConversationInProject(projectId);
      set((state) => ({
        conversations: [conversation, ...state.conversations],
        projectConversations: [conversation, ...state.projectConversations],
        projectActionBusy: false,
      }));
      await get().navigateToChat(conversation.id);
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to create chat",
      });
    }
  },

  assignChats: async (projectId, conversationIds) => {
    if (conversationIds.length === 0) return;
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const updated = await api.assignChatsToProject(projectId, conversationIds);
      const updatedMap = new Map(updated.map((c) => [c.id, c]));
      set((state) => ({
        conversations: state.conversations.map(
          (c) => updatedMap.get(c.id) ?? c,
        ),
        projectConversations:
          state.activeProjectId === projectId
            ? [
                ...updated,
                ...state.projectConversations.filter(
                  (c) => !updatedMap.has(c.id),
                ),
              ].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
            : state.projectConversations,
        addChatsDialogOpen: false,
        addChatsProjectId: null,
        projectActionBusy: false,
      }));
      await get().refreshConversations();
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to assign chats",
      });
    }
  },

  removeChatFromProject: async (conversationId) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const updated = await api.removeChatFromProject(conversationId);
      set((state) => ({
        conversations: state.conversations.map((c) =>
          c.id === updated.id ? updated : c,
        ),
        projectConversations: state.projectConversations.filter(
          (c) => c.id !== conversationId,
        ),
        projectActionBusy: false,
      }));
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error
            ? error.message
            : "Failed to remove chat from project",
      });
    }
  },

  deleteProject: async (projectId, mode) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      await api.deleteProject(projectId, mode);
      const conversations = await api.listConversations();
      set((state) => ({
        projects: state.projects.filter((p) => p.id !== projectId),
        conversations,
        deleteProjectDialogOpen: false,
        deleteProjectId: null,
        projectActionBusy: false,
        activeProject:
          state.activeProject?.id === projectId ? null : state.activeProject,
        activeProjectId:
          state.activeProjectId === projectId ? null : state.activeProjectId,
        view:
          state.view.kind === "project" && state.view.projectId === projectId
            ? { kind: "projects" }
            : state.view,
      }));
      if (get().activeConversationId) {
        const stillExists = conversations.some(
          (c) => c.id === get().activeConversationId,
        );
        if (!stillExists) {
          if (conversations[0]) {
            await get().navigateToChat(conversations[0].id);
          } else {
            set({
              activeConversationId: null,
              messages: [],
              view: { kind: "chat", conversationId: null },
            });
          }
        }
      }
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to delete project",
      });
    }
  },

  archiveProject: async (projectId) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const project = await api.archiveProject(projectId);
      set((state) => ({
        projects: state.projects.map((p) => (p.id === project.id ? project : p)),
        activeProject:
          state.activeProject?.id === project.id ? project : state.activeProject,
        projectActionBusy: false,
      }));
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to archive project",
      });
    }
  },

  restoreProject: async (projectId) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const project = await api.restoreProject(projectId);
      set((state) => ({
        projects: state.projects.map((p) => (p.id === project.id ? project : p)),
        projectActionBusy: false,
      }));
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to restore project",
      });
    }
  },

  updateProject: async (projectId, input) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const project = await api.updateProject(projectId, input);
      set((state) => ({
        projects: state.projects.map((p) => (p.id === project.id ? project : p)),
        activeProject:
          state.activeProject?.id === project.id ? project : state.activeProject,
        editProjectDialogOpen: false,
        editProjectId: null,
        projectActionBusy: false,
      }));
      return project;
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to update project",
      });
      return null;
    }
  },

  renameConversation: async (conversationId, title) => {
    const trimmed = title.trim();
    if (!trimmed) return;
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const updated = await api.renameConversation(conversationId, trimmed);
      set((state) => ({
        conversations: state.conversations.map((c) =>
          c.id === updated.id ? updated : c,
        ),
        projectConversations: state.projectConversations.map((c) =>
          c.id === updated.id ? updated : c,
        ),
        renameConversationDialogOpen: false,
        renameConversationId: null,
        projectActionBusy: false,
      }));
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to rename chat",
      });
    }
  },

  duplicateConversation: async (conversationId) => {
    set({ projectActionBusy: true, projectActionError: null });
    try {
      const copy = await api.duplicateConversation(conversationId);
      set((state) => ({
        conversations: [copy, ...state.conversations],
        projectConversations:
          copy.projectId &&
          state.activeProjectId === copy.projectId
            ? [copy, ...state.projectConversations]
            : state.projectConversations,
        projectActionBusy: false,
      }));
      await get().navigateToChat(copy.id);
    } catch (error) {
      set({
        projectActionBusy: false,
        projectActionError:
          error instanceof Error ? error.message : "Failed to duplicate chat",
      });
    }
  },

  openExportDialog: (toolId, toolName) =>
    set({ exportDialog: { toolId, toolName } }),

  closeExportDialog: () => set({ exportDialog: null }),

  openProviderSetup: (editing = null) =>
    set({ providerSetupOpen: true, providerSetupEditing: editing ?? null }),

  closeProviderSetup: () =>
    set({ providerSetupOpen: false, providerSetupEditing: null }),

  setPreferredModel: async (modelId) => {
    const next = modelId.trim() || "auto";
    set({ preferredModel: next });
    const settings = await api.setSetting("preferredModel", next);
    const allowCatalog = Boolean(get().aiStatus?.disclosure?.showProviderCatalog);
    const catalog = allowCatalog ? await api.getModelCatalog() : null;
    set({
      preferredModel: settings.preferredModel ?? next,
      modelCatalog: catalog,
    });
  },

  setDockIcon: async (preference) => {
    const gated =
      !MANUAL_DOCK_ICON_SELECTION_ENABLED && preference.authority === "manual"
        ? { ...DEFAULT_DOCK_ICON }
        : preference;
    const prior = get().dockIcon;
    const epoch = get().dockIconEpoch + 1;
    set({
      dockIcon: gated,
      dockIconPending: true,
      dockIconError: null,
      dockIconEpoch: epoch,
    });
    try {
      const result = await api.commitDockIconPreference(gated);
      if (get().dockIconEpoch !== epoch) return;
      set({
        dockIcon: parseDockIconConfig(result.config),
        dockPresentation: result.effectivePresentation,
        dockStatusLabel: result.statusLabel,
        dockDevelopmentFallback: result.developmentFallback,
        dockAdaptiveCapable: result.adaptiveCapable,
        dockIconPending: false,
        dockIconError: null,
      });
    } catch (error) {
      if (get().dockIconEpoch !== epoch) return;
      const message =
        error instanceof TauriCommandError
          ? error.message
          : error instanceof Error
            ? error.message
            : "Couldn't update the Dock icon.";
      try {
        const current = await api.getDockIconStatus();
        if (get().dockIconEpoch === epoch) {
          set({
            dockIcon: parseDockIconConfig(current.config),
            dockPresentation: current.effectivePresentation,
            dockStatusLabel: current.statusLabel,
            dockDevelopmentFallback: current.developmentFallback,
            dockAdaptiveCapable: current.adaptiveCapable,
            dockIconPending: false,
            dockIconError: message,
          });
          return;
        }
      } catch {
        // Fallback to prior if status query fails
      }
      set({
        dockIcon: prior,
        dockIconPending: false,
        dockIconError: message,
      });
    }
  },

  applyPersistedDockIcon: async () => {
    const epochAtStart = get().dockIconEpoch;
    try {
      const result = await api.applyPersistedDockIcon();
      // Startup apply must not overwrite an in-flight or newer user commit.
      if (get().dockIconPending || get().dockIconEpoch !== epochAtStart) return;
      set({
        dockIcon: parseDockIconConfig(result.config),
        dockPresentation: result.effectivePresentation,
        dockStatusLabel: result.statusLabel,
        dockDevelopmentFallback: result.developmentFallback,
        dockAdaptiveCapable: result.adaptiveCapable,
        dockIconError: null,
      });
    } catch {
      // Non-macOS / web preview: command may no-op or be unavailable.
    }
  },

  clearDockIconError: () => set({ dockIconError: null }),

  setDeveloperMode: async (enabled) => {
    set({ developerMode: enabled });
    await api.setSetting("developerMode", enabled);
    await get().refreshAiStatus();
  },

  refreshModelCatalog: async () => {
    if (!get().aiStatus?.disclosure?.showProviderCatalog) {
      set({ modelCatalog: null });
      return;
    }
    const catalog = await api.getModelCatalog();
    set({
      modelCatalog: catalog,
      preferredModel: catalog.selected || get().preferredModel || "auto",
    });
  },

  refreshConversations: async () => {
    const conversations = await api.listConversations();
    set({ conversations });
  },

  createConversation: async () => {
    const conversation = await api.createConversation();
    set((state) => ({
      conversations: [conversation, ...state.conversations],
      view: { kind: "chat", conversationId: conversation.id },
      activeConversationId: conversation.id,
      messages: [],
      messagesError: null,
      pendingToolChange: null,
      sendError: null,
    }));
  },

  selectConversation: async (id) => {
    await get().navigateToChat(id);
  },

  deleteConversation: async (id) => {
    await api.deleteConversation(id);
    const conversations = get().conversations.filter((c) => c.id !== id);
    set({ conversations });
    if (get().activeConversationId === id) {
      if (conversations[0]) {
        await get().navigateToChat(conversations[0].id);
      } else {
        set({
          activeConversationId: null,
          messages: [],
          pendingToolChange: null,
          view: { kind: "chat", conversationId: null },
        });
      }
    }
  },

  branchConversation: async (sourceMessageId, branchName) => {
    const activeConvId = get().activeConversationId;
    if (!activeConvId) return;
    try {
      const res = (await api.branchConversation({
        sourceConversationId: activeConvId,
        sourceMessageId,
        branchName,
      })) as [{ newConversationId?: string } | null, unknown];
      const newConvId = res?.[0]?.newConversationId;
      await get().refreshConversations();
      if (newConvId) {
        await get().navigateToChat(newConvId);
      }
    } catch (err) {
      console.error("Failed to branch conversation:", err);
    }
  },

  refreshTools: async () => {
    const tools = await api.listTools();
    set({ tools });
  },

  selectTool: async (id) => {
    const prevToolId = get().activeToolId;
    if (prevToolId && prevToolId !== id) {
      void persistenceScheduler.flush(prevToolId, (toolId, s) => api.saveToolState(toolId, s));
    }
    if (!id) {
      set({ activeToolId: null, activeTool: null, toolState: {} });
      return;
    }
    const conversationId = get().activeConversationId;
    if (conversationId) {
      get().setChatActiveToolId(conversationId, id);
    }
    set({
      view: { kind: "chat", conversationId: conversationId ?? null },
      activeToolId: id,
    });
    const [tool, state] = await Promise.all([
      api.getTool(id),
      api.getToolState(id),
    ]);
    // Selecting another tool, or closing the canvas, wins over a late load.
    if (get().activeToolId !== id) return;
    persistenceScheduler.initToolState(id, state ?? {});
    set({
      activeTool: tool,
      toolState: state ?? {},
    });
    void get().maybeExpandForTool(id);
  },

  closeToolCanvas: () => {
    const prevToolId = get().activeToolId;
    if (prevToolId) {
      void persistenceScheduler.flush(prevToolId, (toolId, s) => api.saveToolState(toolId, s));
    }
    const conversationId = get().activeConversationId;
    if (conversationId) {
      get().setChatActiveToolId(conversationId, null);
    }
    set({ activeToolId: null, activeTool: null, toolState: {} });
    const reducedMotion =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    void api.windowOrchestratorRestore(reducedMotion).catch(() => undefined);
  },

  openToolWindow: async () => {
    const id = get().activeToolId;
    if (!id) return;
    await api.openToolWindow(id, { width: 820, height: 680 });
  },

  undoTool: async () => {
    const id = get().activeToolId;
    if (!id) return;
    const tool = await api.undoToolChange(id);
    await get().refreshTools();
    set({ activeTool: tool });
  },

  updateToolState: async (state, persist = true) => {
    const id = get().activeToolId;
    set({ toolState: state });
    if (persist && id) {
      await persistenceScheduler.schedule(
        id,
        state,
        (toolId, s) => api.saveToolState(toolId, s),
        {
          onRollback: (restored) => {
            if (get().activeToolId === id) {
              set({ toolState: restored });
            }
          },
        },
      );
    }
  },

  flushToolState: async (toolId?: string) => {
    const id = toolId ?? get().activeToolId;
    if (id) {
      await persistenceScheduler.flush(id, (tid, s) => api.saveToolState(tid, s));
    }
  },

  sendMessage: async (
    content,
    mentions = [],
    attachments = [],
    structuredUserInput = null,
  ) => {
    const trimmed = content.trim();
    if (!trimmed && attachments.length === 0 && !structuredUserInput) return false;

    const aiStatus = get().aiStatus?.status;
    if (aiStatus === "missing_key" || aiStatus === "unconfigured") {
      get().openProviderSetup();
      return false;
    }

    let conversationId = get().activeConversationId;
    if (!conversationId) {
      await get().createConversation();
      conversationId = get().activeConversationId;
    }
    if (!conversationId) return false;

    const messageContent =
      trimmed ||
      (structuredUserInput ? "Form submitted" : "Shared attachments");

    const optimistic: ChatMessage = {
      id: `local-${crypto.randomUUID()}`,
      conversationId,
      role: "user",
      content: messageContent,
      createdAt: new Date().toISOString(),
      status: "pending",
      metadata:
        mentions.length > 0 || attachments.length > 0
          ? {
              ...(mentions.length > 0
                ? {
                    mentions: mentions.map((m) => ({
                      mentionId: m.mentionId,
                      toolId: m.toolId,
                      label: m.label,
                      start: m.start,
                      end: m.end,
                    })),
                  }
                : {}),
              ...(attachments.length > 0
                ? {
                    attachments: attachments.map((a) => ({
                      id: a.id,
                      name: a.name,
                      mimeType: a.mimeType,
                      byteSize: a.byteSize,
                    })),
                  }
                : {}),
            }
          : null,
    };

    set((state) => {
      const provisionalId = pendingTurnId(conversationId);
      return {
        messages: [...state.messages, optimistic],
        sending: true,
        sendingConversationId: conversationId,
        sendError: null,
        agentActions: [],
        streamingText: null,
        turnsById: {
          ...state.turnsById,
          [provisionalId]: createTurnLiveState(provisionalId, conversationId),
        },
        activeTurnIdByConversation: {
          ...state.activeTurnIdByConversation,
          [conversationId]: provisionalId,
        },
      };
    });

    const turnConversationId = conversationId;
    const onChannelEvent = (event: AgentTurnEvent) => {
      // Sync/Conflict: Rust prefers subscribeConversationSync; invoke Channel
      // is only used when no sync subscriber delivered. Apply whichever arrives
      // — never both for the same emit (see emit_turn ordering).
      if (event.kind === "sync" || event.kind === "conflict") {
        applySyncOrConflictEvent(get, set, event);
        return;
      }
      // Defense in depth: Channel is invoke-scoped, but still require conversation match.
      if (!("conversationId" in event) || event.conversationId !== turnConversationId) {
        return;
      }
      // Always accumulate in turnsById (even when navigated away). Only mirror
      // derived live UI fields when this conversation is on screen.
      const isActive = get().activeConversationId === turnConversationId;

      if (event.kind === "previewSurface") {
        set((state) => ({
          previewSurfacesByKey: applyPreviewSurfaceOverlay(
            state.previewSurfacesByKey,
            {
              conversationId: turnConversationId,
              turnId: event.turnId,
              toolId: event.toolId,
              surfaceId: event.surfaceId,
              applicationId: event.applicationId,
              definitionJson: event.definitionJson,
              stateJson: event.stateJson,
              revision: event.revision,
              sequence: event.sequence,
            },
          ),
        }));
        return;
      }

      if (event.kind === "operation") {
        const shortId = event.operationId.slice(0, 8) || "—";
        const label =
          event.status === "preview"
            ? `Preview: ${shortId}`
            : event.status === "interrupted"
              ? "Preview interrupted"
              : `Operation ${event.status}: ${shortId}`;
        set((state) => {
          const upserted = upsertTurnFromChannel(state, turnConversationId, null);
          const actions = [...upserted.turn.actions, label].slice(-8);
          const clearPreview =
            event.status === "interrupted" || event.status === "fatal";
          return {
            turnsById: {
              ...upserted.turnsById,
              [upserted.turnId]: { ...upserted.turn, actions },
            },
            activeTurnIdByConversation: upserted.activeTurnIdByConversation,
            ...(clearPreview
              ? {
                  previewSurfacesByKey: clearPreviewOverlaysForConversation(
                    state.previewSurfacesByKey,
                    turnConversationId,
                  ),
                }
              : {}),
            ...(isActive
              ? { agentActions: [...state.agentActions, label].slice(-8) }
              : {}),
          };
        });
        return;
      }
      if (event.kind === "action") {
        set((state) => {
          const upserted = upsertTurnFromChannel(state, turnConversationId, null);
          const actions = [...upserted.turn.actions, event.label].slice(-8);
          return {
            turnsById: {
              ...upserted.turnsById,
              [upserted.turnId]: { ...upserted.turn, actions },
            },
            activeTurnIdByConversation: upserted.activeTurnIdByConversation,
            ...(isActive
              ? { agentActions: [...state.agentActions, event.label].slice(-8) }
              : {}),
          };
        });
        return;
      }
      if (event.kind === "text") {
        set((state) => {
          const upserted = upsertTurnFromChannel(
            state,
            turnConversationId,
            event.turnId,
          );
          const next = applyTextDelta(upserted.turn, event);
          return {
            turnsById: {
              ...upserted.turnsById,
              [upserted.turnId]: next,
            },
            activeTurnIdByConversation: upserted.activeTurnIdByConversation,
            ...(isActive ? { streamingText: next.text || null } : {}),
          };
        });
        return;
      }
      if (event.kind === "error") {
        set((state) => {
          const upserted = upsertTurnFromChannel(state, turnConversationId, null);
          return {
            turnsById: {
              ...upserted.turnsById,
              [upserted.turnId]: {
                ...upserted.turn,
                error: event.message,
                status: "failed" as const,
              },
            },
            activeTurnIdByConversation: upserted.activeTurnIdByConversation,
            previewSurfacesByKey: clearPreviewOverlaysForConversation(
              state.previewSurfacesByKey,
              turnConversationId,
            ),
            ...(isActive ? { sendError: event.message } : {}),
          };
        });
      }
    };

    try {
      const result = await api.sendMessage({
        conversationId,
        content: messageContent,
        activeToolId: get().activeToolId,
        model: get().preferredModel || "auto",
        mentions: mentions.map((m) => ({
          toolId: m.toolId,
          label: m.label,
        })),
        attachments: attachments.map((a) => ({
          id: a.id,
        })),
        structuredUserInput: structuredUserInput ?? null,
        onEvent: onChannelEvent,
      });

      const messages = await api.getMessages(conversationId);
      await get().refreshConversations();

      const pending =
        result.toolChange != null
          ? {
              conversationId,
              messageId: result.messageId,
              toolChange: result.toolChange,
            }
          : null;

      let kernelProposal: PendingKernelProposal | null = null;
      const rv = result.runtimeV2 as
        | {
            proposalId?: string;
            risk?: string;
            impactSummary?: string;
            summary?: string;
            operations?: unknown[];
            status?: string;
            error?: string;
          }
        | null
        | undefined;
      if (
        rv?.proposalId &&
        Array.isArray(rv.operations) &&
        rv.operations.length > 0 &&
        rv.status !== "discarded" &&
        rv.status !== "applied"
      ) {
        kernelProposal = {
          conversationId,
          messageId: result.messageId,
          proposalId: rv.proposalId,
          summary: rv.summary ?? "Proposed application change",
          impactSummary: rv.impactSummary ?? "",
          risk: rv.risk ?? "strong",
          operations: rv.operations,
        };
      }

      // Everything below writes conversation-scoped state (messages, pending
      // proposals, errors). Those fields describe whichever chat is on screen,
      // so a turn that finished after the user navigated away may only clear
      // the turn-scoped indicators.
      const stillActive = get().activeConversationId === conversationId;
      if (!stillActive) {
        set((state) => ({
          sending: false,
          sendingConversationId: null,
          agentActions: [],
          streamingText: null,
          ...finalizeTurnInState(state, conversationId, "completed"),
        }));
        return true;
      }

      if (result.queued) {
        set((state) => ({
          sending: false,
          sendingConversationId: null,
          agentActions: ["Message queued"],
          streamingText: null,
          sendError: null,
          ...finalizeTurnInState(state, conversationId, "completed"),
          messages: state.messages.map((m) =>
            m.id === optimistic.id
              ? {
                  ...m,
                  status: "ok" as const,
                  content: `${m.content}\n\n(Queued — will send when the current reply finishes.)`,
                }
              : m,
          ),
        }));
        return true;
      }

      // settingsChange is a proposal only — do not treat as already applied.
      const pendingSettings: PendingSettingsChange | null = result.settingsChange
        ? {
            conversationId,
            messageId: result.messageId,
            settingsChange: result.settingsChange,
          }
        : null;

      // The user may have navigated away while the turn finished. Conversation-scoped
      // fields must not overwrite whichever chat is now open.
      if (get().activeConversationId !== conversationId) {
        set((state) => ({
          sending: false,
          sendingConversationId: null,
          agentActions: [],
          streamingText: null,
          ...finalizeTurnInState(state, conversationId, "completed"),
          ...(pendingSettings ? { pendingSettingsChange: pendingSettings } : {}),
        }));
        return true;
      }

      set((state) => ({
        messages,
        sending: false,
        sendingConversationId: null,
        pendingToolChange: pending,
        pendingSettingsChange: pendingSettings ?? state.pendingSettingsChange,
        pendingKernelProposal: kernelProposal,
        agentActions: [],
        streamingText: null,
        sendError: rv?.error ? String(rv.error) : null,
        ...finalizeTurnInState(
          state,
          conversationId,
          rv?.error ? "failed" : "completed",
          rv?.error ? String(rv.error) : null,
        ),
      }));
      return true;
    } catch (error) {
      const message =
        error instanceof TauriCommandError || error instanceof Error
          ? error.message
          : "Failed to send message";
      get().setChatDraft(conversationId, trimmed);
      if (get().activeConversationId !== conversationId) {
        set((state) => ({
          sending: false,
          sendingConversationId: null,
          agentActions: [],
          streamingText: null,
          ...finalizeTurnInState(state, conversationId, "failed", message),
        }));
        return false;
      }
      set((state) => ({
        sending: false,
        sendingConversationId: null,
        sendError: message,
        agentActions: [],
        streamingText: null,
        ...finalizeTurnInState(state, conversationId, "failed", message),
        messages: state.messages.map((m) =>
          m.id === optimistic.id
            ? { ...m, status: "error" as const, errorMessage: message }
            : m,
        ),
      }));
      return false;
    }
  },

  cancelRequest: async () => {
    const id = get().sendingConversationId ?? get().activeConversationId;
    await api.cancelRequest(id ?? undefined);
    set((state) => ({
      sending: false,
      sendingConversationId: null,
      agentActions: [],
      streamingText: null,
      ...(id ? finalizeTurnInState(state, id, "cancelled") : {}),
    }));
  },

  retryLastFailed: async () => {
    const failed = [...get().messages]
      .reverse()
      .find((m) => m.role === "user" && m.status === "error");
    if (!failed) return;
    set((state) => ({
      messages: state.messages.filter((m) => m.id !== failed.id),
    }));
    await get().sendMessage(failed.content);
  },

  editAndResendMessage: async (messageId, content) => {
    const trimmed = content.trim();
    if (!trimmed) return;
    const messages = get().messages;
    const index = messages.findIndex((m) => m.id === messageId);
    if (index < 0) return;
    const target = messages[index];
    if (target.role !== "user") return;

    // Drop this message and everything after it (locally + on disk when persisted).
    if (!messageId.startsWith("local-")) {
      try {
        await api.deleteMessagesFrom(messageId);
      } catch (error) {
        const message =
          error instanceof TauriCommandError || error instanceof Error
            ? error.message
            : "Failed to edit message";
        set({ sendError: message });
        return;
      }
    }

    set({
      messages: messages.slice(0, index),
      pendingToolChange: null,
      sendError: null,
    });
    await get().sendMessage(trimmed);
  },

  applyPendingToolChange: async () => {
    const pending = get().pendingToolChange;
    if (!pending) return;
    const validated = validateToolDefinition(pending.toolChange.tool);
    if (!validated.success) {
      set({
        sendError: validated.error + (validated.issues[0] ? `: ${validated.issues[0]}` : ""),
      });
      return;
    }
    const protectedCheck = await api.validateChangeTargets(
      [
        validated.data.id,
        pending.toolChange.targetToolId ?? "",
      ].filter((id) => id.trim().length > 0),
    );
    if (!protectedCheck.ok) {
      set({
        sendError: `Protected core resource cannot be modified: ${protectedCheck.rejected.join(", ")}`,
      });
      return;
    }
    const tool = await api.applyToolChange({
      ...pending,
      toolChange: { ...pending.toolChange, tool: validated.data },
    });
    await get().refreshTools();
    const state = await api.getToolState(tool.id);
    set({
      pendingToolChange: null,
      sendError: null,
      activeToolId: tool.id,
      activeTool: tool,
      toolState: state ?? {},
      messages: get().messages.map((m) =>
        m.id === pending.messageId
          ? {
              ...m,
              metadata: {
                ...(m.metadata ?? {}),
                toolChange: pending.toolChange,
                pending: false,
                toolChangeStatus: "applied",
              },
            }
          : m,
      ),
    });
  },

  discardPendingToolChange: async () => {
    const pending = get().pendingToolChange;
    if (pending) {
      try {
        await api.discardToolChange(pending.messageId);
      } catch {
        // Still clear local pending state if backend discard fails.
      }
    }
    set({
      pendingToolChange: null,
      messages: get().messages.map((m) =>
        pending && m.id === pending.messageId
          ? {
              ...m,
              metadata: {
                ...(m.metadata ?? {}),
                pending: false,
                discarded: true,
                toolChangeStatus: "discarded",
              },
            }
          : m,
      ),
    });
  },

  applyPendingSettingsChange: async () => {
    const pending = get().pendingSettingsChange;
    if (!pending) return;
    const sc = pending.settingsChange;
    try {
      // Expand pair roots → allowlisted *Light/*Dark hex keys (Rust rejects pair roots).
      const colorPairFields = [
        "accentPrimary",
        "accentSecondary",
        "background",
        "surface",
        "surfaceMuted",
        "border",
        "textPrimary",
        "textSecondary",
      ] as const;
      for (const field of colorPairFields) {
        const variants = sc[field];
        if (!variants) continue;
        for (const mode of ["light", "dark"] as const) {
          const raw = variants[mode];
          if (raw == null || raw === "") continue;
          const hex = normalizeCanonicalHex(raw);
          if (!hex) {
            throw new Error(`Invalid ${field}.${mode} color`);
          }
          const key = `${field}${mode === "light" ? "Light" : "Dark"}`;
          await api.setSetting(key, hex);
        }
      }
      if (sc.wallpaper) {
        const wallpaperJson = JSON.stringify(sc.wallpaper);
        await get().applyWorkspaceWallpaper(wallpaperJson);
      }
      if (sc.theme) {
        await get().setTheme(sc.theme);
      }
      const settings = await api.getSettings();
      const theme = settings.theme ?? get().theme;
      const wallpaper = wallpaperFromSettings(settings);
      const globalWallpaperJson = settings.wallpaperJson ?? null;
      syncCommittedWallpaperFromSettings({ wallpaper, globalWallpaperJson });
      set({
        pendingSettingsChange: null,
        sendError: null,
        theme,
        resolvedTheme: resolveTheme(theme),
        appearance: appearanceFromSettings(settings),
        wallpaper,
        globalWallpaperJson,
        messages: get().messages.map((m) =>
          m.id === pending.messageId
            ? {
                ...m,
                metadata: {
                  ...(m.metadata ?? {}),
                  settingsChange: sc,
                  settingsChangeStatus: "applied",
                  pending: false,
                },
              }
            : m,
        ),
      });
    } catch (error) {
      const message =
        error instanceof TauriCommandError || error instanceof Error
          ? error.message
          : "Failed to apply appearance change";
      set({ sendError: message });
    }
  },

  discardPendingSettingsChange: async () => {
    const pending = get().pendingSettingsChange;
    set({
      pendingSettingsChange: null,
      messages: get().messages.map((m) =>
        pending && m.id === pending.messageId
          ? {
              ...m,
              metadata: {
                ...(m.metadata ?? {}),
                pending: false,
                settingsChangeStatus: "discarded",
              },
            }
          : m,
      ),
    });
  },

  applyPendingKernelProposal: async () => {
    const pending = get().pendingKernelProposal;
    if (!pending) return;
    try {
      const result = await api.kernelApplyChange({
        proposalId: pending.proposalId,
        summary: pending.summary,
        operations: pending.operations,
        silent: false,
        sourceType: "user",
        requireApproval: false,
        approvalGranted: true,
        conversationId: pending.conversationId,
        projectId: null,
        turnId: null,
        provider: null,
        model: null,
      });
      const apply = result.apply as { conflicts?: string[] } | undefined;
      if (apply?.conflicts && apply.conflicts.length > 0) {
        if (
          shouldShowAppConflict(
            { conversationId: pending.conversationId },
            get().activeConversationId,
          )
        ) {
          set({
            appConflict: {
              message:
                "This change conflicts with another window or newer revision.",
              conflicts: apply.conflicts,
              conversationId: pending.conversationId,
            },
          });
        }
        return;
      }
      try {
        await api.setKernelProposalStatus(pending.messageId, "applied");
      } catch {
        // best-effort status stamp
      }
      set({
        pendingKernelProposal: null,
        messages: get().messages.map((m) =>
          m.id === pending.messageId
            ? {
                ...m,
                metadata: {
                  ...(m.metadata ?? {}),
                  kernelProposalStatus: "applied",
                  runtimeV2: {
                    ...((m.metadata as { runtimeV2?: Record<string, unknown> } | null)
                      ?.runtimeV2 ?? {}),
                    status: "applied",
                  },
                },
              }
            : m,
        ),
      });
      const messages = await api.getMessages(pending.conversationId);
      set({ messages });
      await get().reloadActiveSurfaces();
    } catch (error) {
      set({
        sendError:
          error instanceof TauriCommandError || error instanceof Error
            ? error.message
            : "Failed to apply proposed change",
      });
    }
  },

  discardPendingKernelProposal: async () => {
    const pending = get().pendingKernelProposal;
    if (!pending) return;
    try {
      await api.setKernelProposalStatus(pending.messageId, "discarded");
    } catch {
      try {
        await api.discardKernelProposal(pending.messageId);
      } catch {
        // best-effort
      }
    }
    set({
      pendingKernelProposal: null,
      messages: get().messages.map((m) =>
        m.id === pending.messageId
          ? {
              ...m,
              metadata: {
                ...(m.metadata ?? {}),
                kernelProposalStatus: "discarded",
                runtimeV2: {
                  ...((m.metadata as { runtimeV2?: Record<string, unknown> } | null)
                    ?.runtimeV2 ?? {}),
                  status: "discarded",
                },
              },
            }
          : m,
      ),
    });
  },

  clearAppConflict: () => set({ appConflict: null }),

  setSurfaceDraftConflict: (conflict) => set({ surfaceDraftConflict: conflict }),
  clearSurfaceDraftConflict: () => set({ surfaceDraftConflict: null }),
  resolveSurfaceDraftConflict: async (action) => {
    const conflict = get().surfaceDraftConflict;
    if (!conflict) return;
    try {
      if (action === "cancel") {
        // Discard the persisted surface draft and dismiss the banner.
        await api.deleteDraft(
          conflict.surfaceId,
          conflict.componentId,
          conflict.windowId,
        );
        set({ surfaceDraftConflict: null });
        return;
      }
      if (action === "keep") {
        await api.saveDraft({
          surfaceId: conflict.surfaceId,
          componentId: conflict.componentId,
          windowId: conflict.windowId,
          baseRevision: conflict.storedRevision,
          draft: conflict.userDraft,
          formId: conflict.formId ?? null,
          force: true,
        });
      } else {
        await api.saveDraft({
          surfaceId: conflict.surfaceId,
          componentId: conflict.componentId,
          windowId: conflict.windowId,
          baseRevision: conflict.requestedRevision,
          draft: conflict.agentDraft ?? {},
          formId: conflict.formId ?? null,
          force: true,
        });
      }
      set({ surfaceDraftConflict: null });
    } catch (error) {
      set({
        sendError:
          error instanceof Error ? error.message : "Failed to resolve draft conflict",
      });
    }
  },

  reloadActiveSurfaces: async () => {
    const toolId = get().activeToolId;
    if (toolId) {
      try {
        await get().selectTool(toolId);
      } catch {
        // ignore
      }
    }
  },

  refreshAiStatus: async () => {
    const aiStatus = await api.getAiStatus();
    set({ aiStatus });
    await get().refreshModelCatalog();
  },

  testConnection: async () => {
    set({ testingConnection: true, connectionTestMessage: null });
    try {
      const result = await api.testAiConnection();
      await get().refreshAiStatus();
      set({
        testingConnection: false,
        connectionTestMessage: result.message ?? "Connection successful",
      });
    } catch (error) {
      set({
        testingConnection: false,
        connectionTestMessage:
          error instanceof Error ? error.message : "Connection failed",
      });
      await get().refreshAiStatus();
    }
  },

  clearConversations: async () => {
    await api.clearConversations();
    set({
      conversations: [],
      activeConversationId: null,
      messages: [],
      pendingToolChange: null,
    });
  },

  clearTools: async () => {
    await api.clearTools();
    set({
      tools: [],
      activeToolId: null,
      activeTool: null,
      toolState: {},
    });
  },

  loadToolWindow: async (toolId) => {
    try {
      const bootstrapStatus = await api.getBootstrapStatus();
      if (bootstrapStatus.status === "recoveryRequired") {
        set({
          bootstrapped: true,
          bootError: null,
          bootstrapStatus,
          appInfo: await api.getAppInfo().catch(() => null),
        });
        return;
      }

      // Settings are best-effort: tool chrome needs allow-get-settings, but a
      // denied/missing read must not block opening the bound tool.
      const [tool, state, settings, appInfo] = await Promise.all([
        api.getTool(toolId),
        api.getToolState(toolId),
        api.getSettings().catch(() => null),
        api.getAppInfo(),
      ]);
      const theme = settings?.theme ?? "system";
      const wallpaper = wallpaperFromSettings(settings ?? {});
      const globalWallpaperJson = settings?.wallpaperJson ?? null;
      syncCommittedWallpaperFromSettings({ wallpaper, globalWallpaperJson });
      set({
        bootstrapped: true,
        bootError: null,
        bootstrapStatus: { status: "ready" },
        appInfo,
        theme,
        resolvedTheme: resolveTheme(theme),
        appearance: appearanceFromSettings(settings ?? {}),
        wallpaper,
        globalWallpaperJson,
        activeToolId: toolId,
        activeTool: tool,
        toolState: state ?? {},
        sidebarCollapsed: true,
      });
      attachAgentTurnSyncListener(get, set);
      // Resolve conversation for Sync Channel (surf-{toolId} when promoted).
      void api
        .getSurface(`surf-${toolId}`)
        .then((surface) => {
          const cid =
            typeof surface?.conversationId === "string"
              ? surface.conversationId.trim()
              : "";
          if (cid) {
            ensureConversationSyncSubscription(cid, get, set);
          }
        })
        .catch(() => {
          // Tool may lack a surface row; residual global Sync remains.
        });
    } catch (error) {
      set({
        bootstrapped: true,
        bootError:
          error instanceof Error ? error.message : "Failed to open tool window",
      });
    }
  },
}));
