import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PendingToolChange } from "@/stores/app-store";
import type { ToolChange } from "@/types/agent";

const kernelDecideProposal = vi.fn();
const setKernelProposalStatus = vi.fn();
const getMessages = vi.fn();
const listTools = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    kernelDecideProposal: (...args: unknown[]) => kernelDecideProposal(...args),
    setKernelProposalStatus: (...args: unknown[]) =>
      setKernelProposalStatus(...args),
    getMessages: (...args: unknown[]) => getMessages(...args),
    listTools: (...args: unknown[]) => listTools(...args),
    subscribeConversationSync: vi.fn(async () => () => undefined),
    getSurface: vi.fn(async () => null),
    setWorkspaceAppearance: vi.fn(),
  },
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(message: string, code = "error") {
      super(message);
      this.code = code;
    }
  },
  listenAgentTurn: vi.fn(),
  shouldShowAppConflict: (
    conflict: { conversationId?: string | null },
    activeConversationId: string | null,
  ) =>
    Boolean(conflict.conversationId) &&
    conflict.conversationId === activeConversationId,
  shouldApplyAgentTurnSync: () => false,
}));

const pendingProposal = {
  conversationId: "conv-1",
  messageId: "msg-1",
  proposalId: "prop-1",
  summary: "Create Task Tracker",
  impactSummary: "Adds a personal tool",
  risk: "write",
  operations: [{ op: "surface.create" }],
};

const stubToolChange = (toolId: string, name: string): ToolChange => ({
  action: "create",
  targetToolId: toolId,
  tool: {
    id: toolId,
    name,
    description: "",
    layout: { type: "stack" },
    components: [],
  },
  changeSummary: "",
});

const pendingToolSameMessage: PendingToolChange = {
  conversationId: "conv-1",
  messageId: "msg-1",
  toolChange: stubToolChange("tool-1", "Task Tracker"),
};

describe("applyPendingKernelProposal", () => {
  beforeEach(() => {
    kernelDecideProposal.mockReset();
    setKernelProposalStatus.mockReset();
    getMessages.mockReset();
    listTools.mockReset();
    kernelDecideProposal.mockResolvedValue({ apply: {} });
    setKernelProposalStatus.mockResolvedValue(undefined);
    getMessages.mockResolvedValue([]);
    listTools.mockResolvedValue([
      { id: "tool-1", name: "Task Tracker", createdAt: "", updatedAt: "" },
    ]);
  });

  it("clears matching pendingToolChange and refreshes tools after apply", async () => {
    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      pendingKernelProposal: pendingProposal,
      pendingToolChange: pendingToolSameMessage,
      messages: [],
      tools: [],
      activeToolId: null,
      activeConversationId: "conv-1",
      sendError: null,
    });

    await useAppStore.getState().applyPendingKernelProposal();

    expect(kernelDecideProposal).toHaveBeenCalledWith("prop-1", true);
    expect(listTools).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().pendingKernelProposal).toBeNull();
    expect(useAppStore.getState().pendingToolChange).toBeNull();
    expect(useAppStore.getState().tools).toEqual([
      { id: "tool-1", name: "Task Tracker", createdAt: "", updatedAt: "" },
    ]);
  });

  it("leaves unrelated pendingToolChange when message ids differ", async () => {
    const { useAppStore } = await import("@/stores/app-store");
    const otherPreview: PendingToolChange = {
      conversationId: "conv-1",
      messageId: "msg-other",
      toolChange: stubToolChange("tool-2", "Other"),
    };
    useAppStore.setState({
      pendingKernelProposal: pendingProposal,
      pendingToolChange: otherPreview,
      messages: [],
      tools: [],
      activeToolId: null,
      activeConversationId: "conv-1",
      sendError: null,
    });

    await useAppStore.getState().applyPendingKernelProposal();

    expect(listTools).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().pendingKernelProposal).toBeNull();
    expect(useAppStore.getState().pendingToolChange).toEqual(otherPreview);
  });

  it("does not clear pending or refresh tools when apply reports conflicts", async () => {
    const { useAppStore } = await import("@/stores/app-store");
    kernelDecideProposal.mockResolvedValue({
      apply: { conflicts: ["revision mismatch"] },
    });
    useAppStore.setState({
      pendingKernelProposal: pendingProposal,
      pendingToolChange: pendingToolSameMessage,
      messages: [],
      tools: [],
      activeToolId: null,
      activeConversationId: "conv-1",
      appConflict: null,
      sendError: null,
    });

    await useAppStore.getState().applyPendingKernelProposal();

    expect(listTools).not.toHaveBeenCalled();
    expect(useAppStore.getState().pendingKernelProposal).toEqual(pendingProposal);
    expect(useAppStore.getState().pendingToolChange).toEqual(
      pendingToolSameMessage,
    );
    expect(useAppStore.getState().appConflict?.conflicts).toEqual([
      "revision mismatch",
    ]);
  });

  it("clears matching pendingToolChange when discarding kernel proposal", async () => {
    const { useAppStore } = await import("@/stores/app-store");
    kernelDecideProposal.mockResolvedValue({});
    useAppStore.setState({
      pendingKernelProposal: pendingProposal,
      pendingToolChange: pendingToolSameMessage,
      messages: [],
      sendError: null,
    });

    await useAppStore.getState().discardPendingKernelProposal();

    expect(kernelDecideProposal).toHaveBeenCalledWith("prop-1", false);
    expect(useAppStore.getState().pendingKernelProposal).toBeNull();
    expect(useAppStore.getState().pendingToolChange).toBeNull();
  });
});
