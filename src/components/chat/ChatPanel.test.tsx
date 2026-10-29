import { act, cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChatPanel } from "./ChatPanel";
import { useAppStore } from "@/stores/app-store";

vi.mock("@/lib/tauri", () => ({
  api: {
    getMessages: vi.fn(async () => []),
    subscribeConversationSync: vi.fn(async () => () => undefined),
    getSurface: vi.fn(async () => null),
    getAiStatus: vi.fn(async () => ({ provider: "mock", model: "mock", disclosure: {} })),
  },
  TauriCommandError: class TauriCommandError extends Error {},
  listenAgentTurn: vi.fn(async () => () => undefined),
  shouldShowAppConflict: () => false,
  shouldApplyAgentTurnSync: () => false,
}));

// Stub heavy sub-components so tests are fast and focused.
vi.mock("@/components/chat/MessageList", () => ({
  MessageList: () => <div data-testid="message-list" />,
}));
vi.mock("@/components/chat/Composer", () => ({
  Composer: () => <div data-testid="composer" />,
}));
vi.mock("@/components/chat/ConversationQueue", () => ({
  ConversationQueue: () => null,
}));
vi.mock("@/components/chat/ConversationHistory", () => ({
  ConversationHistory: () => null,
}));
vi.mock("@/components/chat/GenerationRecoveryBanner", () => ({
  GenerationRecoveryBanner: () => null,
}));
vi.mock("@/components/chat/ProjectIndicator", () => ({
  ProjectIndicator: () => null,
}));
vi.mock("@/components/workspace/WorkspaceDashboard", () => ({
  WorkspaceDashboard: ({ conversationId }: { conversationId?: string | null }) => (
    <div data-testid="workspace-dashboard" data-conv={conversationId ?? "null"} />
  ),
}));

/** Convenience: set the app store fields that ChatPanel reads. */
function setStoreState(overrides: Record<string, unknown> = {}) {
  useAppStore.setState({
    conversations: [],
    projects: [],
    messages: [],
    messagesLoading: false,
    messagesError: null,
    sending: false,
    sendingConversationId: null,
    activeConversationId: null,
    createConversation: vi.fn(async () => undefined),
    navigateToProject: vi.fn(async () => undefined),
    navigateToSettings: vi.fn(),
    ...overrides,
  });
}

const SAMPLE_CONV = {
  id: "conv-1",
  title: "Test Chat",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  projectId: null,
  pinned: false,
  archived: false,
};

describe("ChatPanel routing", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  describe("WorkspaceDashboard (empty conversation)", () => {
    it("renders WorkspaceDashboard when active conversation is empty (messageCount=0)", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [],
        messagesLoading: false,
        messagesError: null,
        sending: false,
        sendingConversationId: null,
      });

      render(<ChatPanel conversationId="conv-1" />);

      expect(screen.getByTestId("workspace-dashboard")).toBeInTheDocument();
      expect(screen.queryByTestId("message-list")).not.toBeInTheDocument();
    });

    it("passes correct conversationId to WorkspaceDashboard", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [],
        messagesLoading: false,
      });

      render(<ChatPanel conversationId="conv-1" />);

      const dashboard = screen.getByTestId("workspace-dashboard");
      expect(dashboard).toHaveAttribute("data-conv", "conv-1");
    });
  });

  describe("Full chat UI (non-empty conversation)", () => {
    it("renders full chat UI when messageCount > 0", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [{ id: "m1" } as never],
        messagesLoading: false,
      });

      render(<ChatPanel conversationId="conv-1" />);

      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
      expect(screen.getByTestId("message-list")).toBeInTheDocument();
    });

    it("renders full chat UI while messagesLoading=true (no premature dashboard)", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [],
        messagesLoading: true,
      });

      render(<ChatPanel conversationId="conv-1" />);

      // messagesLoading prevents dashboard; full chat panel with conversation title rendered
      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
      expect(screen.getByRole("heading", { name: /Test Chat/i })).toBeInTheDocument();
    });

    it("renders full chat UI while sending=true (no dashboard during send)", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [],
        messagesLoading: false,
        sending: true,
        sendingConversationId: "conv-1",
      });

      render(<ChatPanel conversationId="conv-1" />);

      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
      expect(screen.getByRole("heading", { name: /Test Chat/i })).toBeInTheDocument();
    });

    it("renders full chat UI when messagesError is set", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [],
        messagesLoading: false,
        messagesError: "Network error",
      });

      render(<ChatPanel conversationId="conv-1" />);

      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
    });
  });

  describe("Background pane (propConversationId != globalConversationId)", () => {
    it("always renders full chat for background pane even when empty", () => {
      // Global active = conv-2, but this pane is scoped to conv-1
      setStoreState({
        activeConversationId: "conv-2",
        conversations: [
          SAMPLE_CONV,
          { ...SAMPLE_CONV, id: "conv-2", title: "Other Chat" },
        ],
        messages: [],
        messagesLoading: false,
        sending: false,
      });

      render(<ChatPanel conversationId="conv-1" />);

      // Background pane must never render the dashboard
      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
    });
  });

  describe("Null conversationId (no conversation selected)", () => {
    it("shows no-conversation empty state (not dashboard) when conversationId is null", () => {
      setStoreState({
        activeConversationId: null,
        conversations: [],
        messages: [],
        messagesLoading: false,
      });

      render(<ChatPanel conversationId={null} />);

      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
      expect(
        screen.getByText(/No conversation selected/i),
      ).toBeInTheDocument();
    });

    it("renders New chat button in no-conversation empty state", () => {
      setStoreState({
        activeConversationId: null,
        conversations: [],
        messages: [],
      });

      render(<ChatPanel conversationId={null} />);

      const emptyState = document.querySelector(".empty-state")!;
      expect(
        within(emptyState as HTMLElement).getByRole("button", { name: /new chat/i }),
      ).toBeInTheDocument();
    });
  });

  describe("First-message transition: dashboard → message list", () => {
    it("replaces dashboard with message list when messages arrive", () => {
      setStoreState({
        activeConversationId: "conv-1",
        conversations: [SAMPLE_CONV],
        messages: [],
        messagesLoading: false,
      });

      const { rerender } = render(<ChatPanel conversationId="conv-1" />);
      expect(screen.getByTestId("workspace-dashboard")).toBeInTheDocument();

      // Simulate messages arriving
      act(() => {
        useAppStore.setState({ messages: [{ id: "m1" } as never] });
      });
      rerender(<ChatPanel conversationId="conv-1" />);

      expect(screen.queryByTestId("workspace-dashboard")).not.toBeInTheDocument();
      expect(screen.getByTestId("message-list")).toBeInTheDocument();
    });
  });
});
