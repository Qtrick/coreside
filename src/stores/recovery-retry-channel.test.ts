import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentTurnEvent } from "@/lib/tauri";

const retryRecoverableGeneration = vi.fn();
const getMessages = vi.fn();
const refreshConversations = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    retryRecoverableGeneration: (...args: unknown[]) =>
      retryRecoverableGeneration(...args),
    getMessages: (...args: unknown[]) => getMessages(...args),
    refreshConversations: (...args: unknown[]) => refreshConversations(...args),
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

describe("retryRecoverableGeneration channel + proposals", () => {
  beforeEach(() => {
    retryRecoverableGeneration.mockReset();
    getMessages.mockReset();
    refreshConversations.mockReset();
    getMessages.mockResolvedValue([
      {
        id: "msg-user",
        conversationId: "conv-retry",
        role: "user",
        content: "retry me",
        createdAt: "2026-10-19T00:00:00Z",
      },
      {
        id: "msg-asst",
        conversationId: "conv-retry",
        role: "assistant",
        content: "proposal ready",
        createdAt: "2026-10-19T00:00:01Z",
      },
    ]);
    refreshConversations.mockResolvedValue(undefined);
    vi.resetModules();
  });

  it("applies streaming events and stages runtimeV2 proposals after getMessages", async () => {
    const conversationId = "conv-retry";
    retryRecoverableGeneration.mockImplementation(
      async (args: {
        conversationId: string;
        onEvent?: (event: AgentTurnEvent) => void;
      }) => {
        args.onEvent?.({
          kind: "text",
          conversationId,
          turnId: "turn-retry",
          text: "Building",
          sequence: 1,
        });
        args.onEvent?.({
          kind: "action",
          conversationId,
          label: "Validated plan",
        });
        return {
          admitted: true,
          retiredTurnId: "turn-old",
          conversationId,
          originalContent: "retry me",
          userMessageId: "msg-user",
          reusedExistingMessage: true,
          startsNewAttempt: true,
          messageId: "msg-asst",
          runtimeV2: {
            proposalId: "prop-1",
            summary: "Create Task Tracker",
            impactSummary: "Adds a personal tool",
            risk: "strong",
            status: "pending",
            operations: [{ op: "surface.create" }],
          },
        };
      },
    );

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      activeConversationId: conversationId,
      messages: [],
      previewSurfacesByKey: {},
      pendingKernelProposal: null,
      sending: false,
      sendError: null,
      streamingText: null,
      agentActions: [],
    });

    const result = await useAppStore.getState().retryRecoverableGeneration({
      conversationId,
      turnId: "turn-old",
      expectedAttemptId: "attempt-1",
      expectedSequence: 1,
    });

    expect(result.admitted).toBe(true);
    expect(getMessages).toHaveBeenCalledWith(conversationId);
    expect(useAppStore.getState().messages).toHaveLength(2);
    expect(useAppStore.getState().streamingText).toBeNull();
    expect(useAppStore.getState().pendingKernelProposal).toMatchObject({
      proposalId: "prop-1",
      messageId: "msg-asst",
      conversationId,
    });
    expect(
      useAppStore
        .getState()
        .turnsById[useAppStore.getState().activeTurnIdByConversation[conversationId]!]
        ?.actions.some((a) => a.includes("Validated plan")),
    ).toBe(true);
  });

  it("surfaces busy errors without claiming admission", async () => {
    const conversationId = "conv-busy";
    retryRecoverableGeneration.mockRejectedValue(
      new Error("busy. Your saved preview remains available."),
    );

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      activeConversationId: conversationId,
      messages: [],
      sending: false,
      sendError: null,
    });

    await expect(
      useAppStore.getState().retryRecoverableGeneration({
        conversationId,
        turnId: "turn-busy",
        expectedAttemptId: "attempt-1",
      }),
    ).rejects.toThrow(/busy/i);

    expect(useAppStore.getState().sending).toBe(false);
    expect(useAppStore.getState().sendError).toMatch(/busy/i);
  });
});
