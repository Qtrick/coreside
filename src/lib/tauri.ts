import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  AiStatus,
  AppInfo,
  AppSettings,
  SendMessageResult,
  ThemePreference,
  ToolChange,
} from "@/types/agent";
import type { ChatMessage, Conversation } from "@/types/messages";
import type {
  ToolDefinition,
  ToolState,
  ToolSummary,
  ToolVersion,
} from "@/types/tool";

export class TauriCommandError extends Error {
  readonly code?: string;

  constructor(message: string, code?: string) {
    super(message);
    this.name = "TauriCommandError";
    this.code = code;
  }
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
    const message =
      typeof error === "string"
        ? error
        : error instanceof Error
          ? error.message
          : "Command failed";
    throw new TauriCommandError(message);
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

/* ─── In-memory mocks for vitest / web preview ─── */

const now = () => new Date().toISOString();

const mockDb = {
  conversations: [] as Conversation[],
  messages: new Map<string, ChatMessage[]>(),
  tools: [] as ToolDefinition[],
  toolState: new Map<string, ToolState>(),
  toolVersions: new Map<string, ToolVersion[]>(),
  settings: {
    theme: "system" as ThemePreference,
    sidebarCollapsed: false as boolean,
  } satisfies AppSettings,
  aiConfigured: false,
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

    case "get_ai_status":
      return {
        provider: "gemini",
        model: "gemini-3.5-flash",
        keyDetected: mockDb.aiConfigured,
        status: mockDb.aiConfigured ? "ready" : "missing_key",
        message: mockDb.aiConfigured
          ? "Ready"
          : "Add AI_API_KEY (or GEMINI_API_KEY) to the project .env file, then restart npm run dev.",
      } satisfies AiStatus as T;

    case "test_ai_connection":
      if (!mockDb.aiConfigured) {
        throw new TauriCommandError(
          "AI is not configured. Add AI_API_KEY or GEMINI_API_KEY to .env.",
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
        createdAt: now(),
        updatedAt: now(),
      };
      mockDb.conversations.unshift(conversation);
      mockDb.messages.set(conversation.id, []);
      return conversation as T;
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

    case "send_message": {
      const conversationId = String(args?.conversationId ?? "");
      const content = String(args?.content ?? "");
      const messages = mockDb.messages.get(conversationId) ?? [];
      const userMessage: ChatMessage = {
        id: crypto.randomUUID(),
        conversationId,
        role: "user",
        content,
        createdAt: now(),
        status: "ok",
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
      return { ...mockDb.settings } as T;
    }

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
  mockDb.messages.clear();
  mockDb.tools = [];
  mockDb.toolState.clear();
  mockDb.toolVersions.clear();
  mockDb.settings = { theme: "system", sidebarCollapsed: false };
  mockDb.aiConfigured = false;
}

export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  getAiStatus: () => invoke<AiStatus>("get_ai_status"),
  testAiConnection: () =>
    invoke<{ ok: boolean; message?: string }>("test_ai_connection"),
  listConversations: () => invoke<Conversation[]>("list_conversations"),
  createConversation: () => invoke<Conversation>("create_conversation"),
  deleteConversation: (conversationId: string) =>
    invoke<void>("delete_conversation", { conversationId }),
  getMessages: (conversationId: string) =>
    invoke<ChatMessage[]>("get_messages", { conversationId }),
  sendMessage: (args: {
    conversationId: string;
    content: string;
    activeToolId?: string | null;
  }) => invoke<SendMessageResult>("send_message", args),
  cancelRequest: (conversationId?: string) =>
    invoke<void>("cancel_request", { conversationId }),
  discardToolChange: (messageId: string) =>
    invoke<ChatMessage>("discard_tool_change", { messageId }),
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
  clearConversations: () => invoke<void>("clear_conversations"),
  clearTools: () => invoke<void>("clear_tools"),
  openToolWindow: (toolId: string) =>
    invoke<void>("open_tool_window", { toolId }),
};
