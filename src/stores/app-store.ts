import { create } from "zustand";
import type {
  AiStatus,
  AppInfo,
  DockIconPreference,
  ModelCatalog,
  ThemePreference,
  ToolChange,
  WallpaperConfig,
} from "@/types/agent";
import { DEFAULT_WALLPAPER, WallpaperKindSchema } from "@/types/agent";
import { parseWallpaperJson } from "@/types/wallpaper";
import type { ChatMessage, Conversation } from "@/types/messages";
import type {
  ToolDefinition,
  ToolState,
  ToolSummary,
} from "@/types/tool";
import { api, TauriCommandError, listenAgentTurn } from "@/lib/tauri";
import type { ToolMention } from "@/lib/mentions";
import { ensureReadableForeground } from "@/lib/readability/contrast";
import type { ActionLogMode } from "@/lib/action-log";
import { validateToolDefinition } from "@/lib/tool-schema";
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

export type PendingToolChange = {
  conversationId: string;
  messageId: string;
  toolChange: ToolChange;
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
  appInfo: AppInfo | null;
  aiStatus: AiStatus | null;
  theme: ThemePreference;
  resolvedTheme: "light" | "dark";
  appearance: AppearancePalette;
  wallpaper: WallpaperConfig;
  globalWallpaperJson: string | null;
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
  dockIcon: DockIconPreference;
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

  pendingToolChange: PendingToolChange | null;
  pendingKernelProposal: PendingKernelProposal | null;
  appConflict: AppConflict | null;
  surfaceDraftConflict: SurfaceDraftConflict | null;
  sending: boolean;
  sendError: string | null;
  agentActions: string[];
  streamingText: string | null;
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
  applyProjectWallpaper: (projectId: string, wallpaperJson: string) => Promise<void>;
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
  setDockIcon: (preference: DockIconPreference) => Promise<void>;
  setDeveloperMode: (enabled: boolean) => Promise<void>;
  refreshModelCatalog: () => Promise<void>;

  refreshConversations: () => Promise<void>;
  createConversation: () => Promise<void>;
  /** @deprecated Prefer navigateToChat */
  selectConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;

  refreshTools: () => Promise<void>;
  selectTool: (id: string | null) => Promise<void>;
  closeToolCanvas: () => void;
  openToolWindow: () => Promise<void>;
  undoTool: () => Promise<void>;
  updateToolState: (state: ToolState, persist?: boolean) => Promise<void>;

  sendMessage: (
    content: string,
    mentions?: ToolMention[],
    attachments?: import("@/types/attachments").StagedAttachment[],
  ) => Promise<void>;
  cancelRequest: () => Promise<void>;
  retryLastFailed: () => Promise<void>;
  editAndResendMessage: (messageId: string, content: string) => Promise<void>;
  applyPendingToolChange: () => Promise<void>;
  discardPendingToolChange: () => Promise<void>;
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

function attachAgentTurnSyncListener(
  get: () => {
    reloadActiveSurfaces: () => Promise<void>;
  },
  set: (partial: {
    appConflict: AppConflict | null;
  }) => void,
) {
  if (agentTurnSyncAttached) return;
  agentTurnSyncAttached = true;
  void listenAgentTurn((event) => {
    if (event.kind === "conflict") {
      set({
        appConflict: {
          message: event.message,
          conflicts: event.conflicts,
          conversationId: event.conversationId ?? null,
        },
      });
      return;
    }
    if (event.kind === "sync") {
      void get().reloadActiveSurfaces();
    }
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

export const useAppStore = create<AppStore>((set, get) => ({
  bootstrapped: false,
  bootError: null,
  appInfo: null,
  aiStatus: null,
  theme: "system",
  resolvedTheme: "light",
  appearance: { ...DEFAULT_APPEARANCE },
  wallpaper: { ...DEFAULT_WALLPAPER },
  globalWallpaperJson: null,
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
  dockIcon: "auto",
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

  pendingToolChange: null,
  pendingKernelProposal: null,
  appConflict: null,
  surfaceDraftConflict: null,
  sending: false,
  sendError: null,
  agentActions: [],
  streamingText: null,
  testingConnection: false,
  connectionTestMessage: null,

  bootstrap: async () => {
    try {
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
      set({
        bootstrapped: true,
        bootError: null,
        appInfo,
        aiStatus,
        theme,
        resolvedTheme: resolved,
        appearance,
        wallpaper,
        globalWallpaperJson: settings.wallpaperJson ?? null,
        sidebarCollapsed: settings.sidebarCollapsed ?? false,
        preferredModel: settings.preferredModel ?? modelCatalog?.selected ?? "auto",
        dockIcon: settings.dockIcon ?? "auto",
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
      });
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

    set({
      view: { kind: "chat", conversationId: trimmed },
      activeConversationId: trimmed,
      activeProjectId:
        state.conversations.find((c) => c.id === trimmed)?.projectId ?? null,
      ...(needLoad
        ? {
            messagesLoading: true,
            messagesError: null,
            pendingToolChange: null,
          }
        : {}),
    });

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
        set({
          messages,
          messagesLoading: false,
          pendingToolChange: pending,
          pendingKernelProposal: kernelProposal,
        });
      } catch (error) {
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
    const trimmed = wallpaperJson.trim();
    let settings;
    if (!trimmed) {
      settings = await api.setSetting("wallpaperJson", "");
      const legacy = await api.setSetting("wallpaper", DEFAULT_WALLPAPER);
      settings = legacy;
    } else {
      const parsed = parseWallpaperJson(trimmed);
      if (parsed.format === "legacy") {
        settings = await api.setSetting("wallpaper", parsed.config);
        settings = await api.setSetting("wallpaperJson", "");
      } else {
        settings = await api.setSetting("wallpaperJson", trimmed);
        settings = await api.setSetting("wallpaper", DEFAULT_WALLPAPER);
      }
    }
    set({
      wallpaper: wallpaperFromSettings(settings),
      globalWallpaperJson: settings.wallpaperJson ?? null,
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
    const next = preference === "dark" || preference === "light" ? preference : "auto";
    set({ dockIcon: next });
    const settings = await api.setSetting("dockIcon", next);
    const resolved = (settings.dockIcon ?? next) as DockIconPreference;
    set({ dockIcon: resolved });
    const osIsDark =
      typeof window !== "undefined" && window.matchMedia
        ? window.matchMedia("(prefers-color-scheme: dark)").matches
        : false;
    await api.setDockIcon(resolved, osIsDark).catch(() => {
      // Non-macOS / web preview: command may no-op.
    });
  },

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

  refreshTools: async () => {
    const tools = await api.listTools();
    set({ tools });
  },

  selectTool: async (id) => {
    if (!id) {
      set({ activeToolId: null, activeTool: null, toolState: {} });
      return;
    }
    const conversationId = get().activeConversationId;
    if (conversationId) {
      get().setChatActiveToolId(conversationId, id);
    }
    set({ view: { kind: "chat", conversationId: conversationId ?? null } });
    const [tool, state] = await Promise.all([
      api.getTool(id),
      api.getToolState(id),
    ]);
    set({
      activeToolId: id,
      activeTool: tool,
      toolState: state ?? {},
    });
  },

  closeToolCanvas: () => {
    const conversationId = get().activeConversationId;
    if (conversationId) {
      get().setChatActiveToolId(conversationId, null);
    }
    set({ activeToolId: null, activeTool: null, toolState: {} });
  },

  openToolWindow: async () => {
    const id = get().activeToolId;
    if (!id) return;
    await api.openToolWindow(id);
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
    const previous = get().toolState;
    set({ toolState: state });
    if (persist && id) {
      try {
        await api.saveToolState(id, state);
      } catch {
        set({ toolState: previous });
      }
    }
  },

  sendMessage: async (content, mentions = [], attachments = []) => {
    const trimmed = content.trim();
    if (!trimmed && attachments.length === 0) return;

    const aiStatus = get().aiStatus?.status;
    if (aiStatus === "missing_key" || aiStatus === "unconfigured") {
      get().openProviderSetup();
      return;
    }

    let conversationId = get().activeConversationId;
    if (!conversationId) {
      await get().createConversation();
      conversationId = get().activeConversationId;
    }
    if (!conversationId) return;

    const messageContent = trimmed || "Shared attachments";

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
                      localFilename: a.localFilename,
                    })),
                  }
                : {}),
            }
          : null,
    };

    set((state) => ({
      messages: [...state.messages, optimistic],
      sending: true,
      sendError: null,
      agentActions: [],
      streamingText: null,
    }));

    const activeConversationId = conversationId;
    const stopListen = await listenAgentTurn((event) => {
      if (event.kind === "conflict") {
        set({
          appConflict: {
            message: event.message,
            conflicts: event.conflicts,
            conversationId: event.conversationId,
          },
        });
        return;
      }
      if (event.kind === "sync") {
        void get().reloadActiveSurfaces();
        return;
      }
      if (event.kind === "operation") {
        set((state) => ({
          agentActions: [
            ...state.agentActions,
            `Operation ${event.status}: ${event.operationId.slice(0, 8)}`,
          ].slice(-8),
        }));
        return;
      }
      if (event.conversationId !== activeConversationId) return;
      if (event.kind === "action") {
        set((state) => ({
          agentActions: [...state.agentActions, event.label].slice(-8),
        }));
      } else if (event.kind === "text") {
        set({ streamingText: event.text });
      } else if (event.kind === "error") {
        set({ sendError: event.message });
      }
    });

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
          name: a.name,
          mimeType: a.mimeType,
          byteSize: a.byteSize,
          localFilename: a.localFilename,
        })),
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

      if (result.queued) {
        set((state) => ({
          sending: false,
          agentActions: ["Message queued"],
          streamingText: null,
          sendError: null,
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
        return;
      }

      let themePatch: Partial<{
        theme: ThemePreference;
        resolvedTheme: "light" | "dark";
        appearance: AppearancePalette;
        wallpaper: WallpaperConfig;
        globalWallpaperJson: string | null;
      }> = {};
      if (result.settingsChange) {
        const settings = await api.getSettings();
        const theme = settings.theme ?? get().theme;
        themePatch = {
          theme,
          resolvedTheme: resolveTheme(theme),
          appearance: appearanceFromSettings(settings),
          wallpaper: wallpaperFromSettings(settings),
          globalWallpaperJson: settings.wallpaperJson ?? null,
        };
      }

      set({
        messages,
        sending: false,
        pendingToolChange: pending,
        pendingKernelProposal: kernelProposal,
        agentActions: [],
        streamingText: null,
        sendError: rv?.error ? String(rv.error) : null,
        ...themePatch,
      });
    } catch (error) {
      const message =
        error instanceof TauriCommandError || error instanceof Error
          ? error.message
          : "Failed to send message";
      set((state) => ({
        sending: false,
        sendError: message,
        agentActions: [],
        streamingText: null,
        messages: state.messages.map((m) =>
          m.id === optimistic.id
            ? { ...m, status: "error" as const, errorMessage: message }
            : m,
        ),
      }));
    } finally {
      stopListen();
    }
  },

  cancelRequest: async () => {
    const id = get().activeConversationId;
    await api.cancelRequest(id ?? undefined);
    set({
      sending: false,
      agentActions: [],
      streamingText: null,
    });
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

  applyPendingKernelProposal: async () => {
    const pending = get().pendingKernelProposal;
    if (!pending || pending.operations.length === 0) return;
    try {
      const result = await api.kernelApplyChange({
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
        set({
          appConflict: {
            message:
              "This change conflicts with another window or newer revision.",
            conflicts: apply.conflicts,
            conversationId: pending.conversationId,
          },
        });
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
    if (action === "cancel") {
      set({ surfaceDraftConflict: null });
      return;
    }
    try {
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
    const [tool, state, settings, appInfo] = await Promise.all([
      api.getTool(toolId),
      api.getToolState(toolId),
      api.getSettings(),
      api.getAppInfo(),
    ]);
    const theme = settings.theme ?? "system";
    set({
      bootstrapped: true,
      appInfo,
      theme,
      resolvedTheme: resolveTheme(theme),
      appearance: appearanceFromSettings(settings),
      wallpaper: wallpaperFromSettings(settings),
      globalWallpaperJson: settings.wallpaperJson ?? null,
      activeToolId: toolId,
      activeTool: tool,
      toolState: state ?? {},
      sidebarCollapsed: true,
    });
    attachAgentTurnSyncListener(get, set);
  },
}));
