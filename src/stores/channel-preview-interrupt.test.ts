import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentTurnEvent } from "@/lib/tauri";

const sendMessage = vi.fn();
const getMessages = vi.fn();
const refreshConversations = vi.fn();
const cancelRequest = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    sendMessage: (...args: unknown[]) => sendMessage(...args),
    getMessages: (...args: unknown[]) => getMessages(...args),
    refreshConversations: (...args: unknown[]) => refreshConversations(...args),
    cancelRequest: (...args: unknown[]) => cancelRequest(...args),
    subscribeConversationSync: vi.fn(async () => () => undefined),
    getSurface: vi.fn(async () => null),
    setWorkspaceAppearance: vi.fn(),
    listConversations: vi.fn(async () => []),
    listTools: vi.fn(async () => []),
  },
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(message: string, code = "error") {
      super(message);
      this.code = code;
    }
  },
  listenAgentTurn: vi.fn(),
  shouldShowAppConflict: () => false,
  shouldApplyAgentTurnSync: () => false,
}));

const previewToolDef = {
  id: "tool-1",
  name: "Counter",
  description: "preview",
  components: [{ id: "c1", type: "progress", props: { maximum: 10 } }],
};

describe("sendMessage channel preview lifecycle", () => {
  beforeEach(() => {
    sendMessage.mockReset();
    getMessages.mockReset();
    refreshConversations.mockReset();
    cancelRequest.mockReset();
    getMessages.mockResolvedValue([]);
    refreshConversations.mockResolvedValue(undefined);
    cancelRequest.mockResolvedValue(undefined);
    vi.resetModules();
  });

  it("discards speculative preview overlays when an operation is interrupted", async () => {
    const conversationId = "conv-interrupt";
    sendMessage.mockImplementation(
      async (args: {
        conversationId: string;
        onEvent?: (event: AgentTurnEvent) => void;
      }) => {
        args.onEvent?.({
          kind: "previewSurface",
          conversationId,
          turnId: "turn-live",
          toolId: "tool-1",
          surfaceId: "surf-tool-1",
          applicationId: "tool-1",
          definitionJson: previewToolDef,
          stateJson: { n: 1 },
          revision: 1,
          sequence: 1,
        });
        args.onEvent?.({
          kind: "operation",
          conversationId,
          operationId: "op-interrupted",
          status: "interrupted",
        });
        return {
          assistantMessage: "Stopped",
          messageId: "msg-1",
          toolChange: null,
        };
      },
    );

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      activeConversationId: conversationId,
      messages: [],
      previewSurfacesByKey: {},
      sending: false,
      sendError: null,
    });

    await useAppStore.getState().sendMessage("hello");

    expect(Object.keys(useAppStore.getState().previewSurfacesByKey)).toEqual([]);
    expect(
      useAppStore
        .getState()
        .turnsById[useAppStore.getState().activeTurnIdByConversation[conversationId]!]
        ?.actions.some((a) => a.label.includes("interrupted")),
    ).toBe(true);
  });

  it("ignores late previewSurface events after cancelRequest", async () => {
    const conversationId = "conv-cancel-race";
    const channel = {
      emit: null as null | ((event: AgentTurnEvent) => void),
      release: null as null | (() => void),
    };
    sendMessage.mockImplementation(
      async (args: {
        conversationId: string;
        onEvent?: (event: AgentTurnEvent) => void;
      }) => {
        channel.emit = args.onEvent ?? null;
        await new Promise<void>((resolve) => {
          channel.release = resolve;
        });
        return {
          assistantMessage: "",
          messageId: "msg-never",
          toolChange: null,
        };
      },
    );

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      activeConversationId: conversationId,
      messages: [],
      previewSurfacesByKey: {},
      sending: false,
      sendError: null,
      turnsById: {},
      activeTurnIdByConversation: {},
    });

    const sendPromise = useAppStore.getState().sendMessage("hello");
    // Let the send path register the Channel before we cancel.
    await Promise.resolve();
    await Promise.resolve();

    await useAppStore.getState().cancelRequest();
    expect(cancelRequest).toHaveBeenCalled();
    expect(useAppStore.getState().sending).toBe(false);

    expect(channel.emit).toBeTypeOf("function");
    channel.emit!({
      kind: "previewSurface",
      conversationId,
      turnId: "turn-late",
      toolId: "tool-1",
      surfaceId: "surf-tool-1",
      applicationId: "tool-1",
      definitionJson: previewToolDef,
      stateJson: { n: 99 },
      revision: 2,
      sequence: 9,
    });

    expect(Object.keys(useAppStore.getState().previewSurfacesByKey)).toEqual([]);
    channel.release?.();
    await sendPromise.catch(() => undefined);
  });
});
