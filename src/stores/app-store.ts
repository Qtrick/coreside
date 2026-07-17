import { create } from "zustand";
import type {
  AiStatus,
  AppInfo,
  ThemePreference,
  ToolChange,
} from "@/types/agent";
import type { ChatMessage, Conversation } from "@/types/messages";
import type {
  ToolDefinition,
  ToolState,
  ToolSummary,
} from "@/types/tool";
import { api, TauriCommandError } from "@/lib/tauri";
import { validateToolDefinition } from "@/lib/tool-schema";

export type PendingToolChange = {
  conversationId: string;
  messageId: string;
  toolChange: ToolChange;
};

type AppStore = {
  bootstrapped: boolean;
  bootError: string | null;
  appInfo: AppInfo | null;
  aiStatus: AiStatus | null;
  theme: ThemePreference;
  resolvedTheme: "light" | "dark";
  sidebarCollapsed: boolean;
  settingsOpen: boolean;

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
  sending: boolean;
  sendError: string | null;
  testingConnection: boolean;
  connectionTestMessage: string | null;

  bootstrap: () => Promise<void>;
  setTheme: (theme: ThemePreference) => Promise<void>;
  applyResolvedTheme: (resolved: "light" | "dark") => void;
  toggleSidebar: () => Promise<void>;
  setSettingsOpen: (open: boolean) => void;

  refreshConversations: () => Promise<void>;
  createConversation: () => Promise<void>;
  selectConversation: (id: string) => Promise<void>;
  deleteConversation: (id: string) => Promise<void>;

  refreshTools: () => Promise<void>;
  selectTool: (id: string | null) => Promise<void>;
  closeToolCanvas: () => void;
  openToolWindow: () => Promise<void>;
  undoTool: () => Promise<void>;
  updateToolState: (state: ToolState, persist?: boolean) => Promise<void>;

  sendMessage: (content: string) => Promise<void>;
  cancelRequest: () => Promise<void>;
  retryLastFailed: () => Promise<void>;
  applyPendingToolChange: () => Promise<void>;
  discardPendingToolChange: () => Promise<void>;

  refreshAiStatus: () => Promise<void>;
  testConnection: () => Promise<void>;
  clearConversations: () => Promise<void>;
  clearTools: () => Promise<void>;

  loadToolWindow: (toolId: string) => Promise<void>;
};

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

export const useAppStore = create<AppStore>((set, get) => ({
  bootstrapped: false,
  bootError: null,
  appInfo: null,
  aiStatus: null,
  theme: "system",
  resolvedTheme: "light",
  sidebarCollapsed: false,
  settingsOpen: false,

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
  sending: false,
  sendError: null,
  testingConnection: false,
  connectionTestMessage: null,

  bootstrap: async () => {
    try {
      const [appInfo, aiStatus, settings, conversations, tools] =
        await Promise.all([
          api.getAppInfo(),
          api.getAiStatus(),
          api.getSettings(),
          api.listConversations(),
          api.listTools(),
        ]);

      const theme = settings.theme ?? "system";
      const resolved = resolveTheme(theme);
      set({
        bootstrapped: true,
        bootError: null,
        appInfo,
        aiStatus,
        theme,
        resolvedTheme: resolved,
        sidebarCollapsed: settings.sidebarCollapsed ?? false,
        conversations,
        tools,
      });

      if (conversations.length > 0) {
        await get().selectConversation(conversations[0].id);
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

  setSettingsOpen: (open) => set({ settingsOpen: open }),

  refreshConversations: async () => {
    const conversations = await api.listConversations();
    set({ conversations });
  },

  createConversation: async () => {
    const conversation = await api.createConversation();
    set((state) => ({
      conversations: [conversation, ...state.conversations],
      activeConversationId: conversation.id,
      messages: [],
      messagesError: null,
      pendingToolChange: null,
      sendError: null,
      settingsOpen: false,
    }));
  },

  selectConversation: async (id) => {
    set({
      activeConversationId: id,
      messagesLoading: true,
      messagesError: null,
      pendingToolChange: null,
      settingsOpen: false,
    });
    try {
      const messages = await api.getMessages(id);
      let pending: PendingToolChange | null = null;
      for (let i = messages.length - 1; i >= 0; i -= 1) {
        const meta = messages[i].metadata as
          | {
              toolChange?: ToolChange;
              pending?: boolean;
              toolChangeStatus?: string;
            }
          | null
          | undefined;
        const isPending =
          !!meta?.toolChange &&
          (meta.toolChangeStatus === "pending" || meta.pending === true);
        if (isPending && meta?.toolChange) {
          pending = {
            conversationId: id,
            messageId: messages[i].id,
            toolChange: meta.toolChange,
          };
          break;
        }
      }
      set({ messages, messagesLoading: false, pendingToolChange: pending });
    } catch (error) {
      set({
        messages: [],
        messagesLoading: false,
        messagesError:
          error instanceof Error ? error.message : "Failed to load conversation",
      });
    }
  },

  deleteConversation: async (id) => {
    await api.deleteConversation(id);
    const conversations = get().conversations.filter((c) => c.id !== id);
    set({ conversations });
    if (get().activeConversationId === id) {
      if (conversations[0]) {
        await get().selectConversation(conversations[0].id);
      } else {
        set({
          activeConversationId: null,
          messages: [],
          pendingToolChange: null,
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
    set({ settingsOpen: false });
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
    set({ toolState: state });
    if (persist && id) {
      await api.saveToolState(id, state);
    }
  },

  sendMessage: async (content) => {
    const trimmed = content.trim();
    if (!trimmed) return;

    let conversationId = get().activeConversationId;
    if (!conversationId) {
      await get().createConversation();
      conversationId = get().activeConversationId;
    }
    if (!conversationId) return;

    const optimistic: ChatMessage = {
      id: `local-${crypto.randomUUID()}`,
      conversationId,
      role: "user",
      content: trimmed,
      createdAt: new Date().toISOString(),
      status: "pending",
    };

    set((state) => ({
      messages: [...state.messages, optimistic],
      sending: true,
      sendError: null,
    }));

    try {
      const result = await api.sendMessage({
        conversationId,
        content: trimmed,
        activeToolId: get().activeToolId,
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

      set({
        messages,
        sending: false,
        pendingToolChange: pending,
      });
    } catch (error) {
      const message =
        error instanceof TauriCommandError || error instanceof Error
          ? error.message
          : "Failed to send message";
      set((state) => ({
        sending: false,
        sendError: message,
        messages: state.messages.map((m) =>
          m.id === optimistic.id
            ? { ...m, status: "error" as const, errorMessage: message }
            : m,
        ),
      }));
    }
  },

  cancelRequest: async () => {
    const id = get().activeConversationId;
    await api.cancelRequest(id ?? undefined);
    set({ sending: false });
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

  refreshAiStatus: async () => {
    const aiStatus = await api.getAiStatus();
    set({ aiStatus });
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
      activeToolId: toolId,
      activeTool: tool,
      toolState: state ?? {},
      sidebarCollapsed: true,
    });
  },
}));
