import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceView } from "./WorkspaceView";
import { useWorkspaceStore } from "@/stores/workspace-store";
import { useAppStore } from "@/stores/app-store";
import { createDefaultWorkspaceState } from "@/types/workspace";

vi.mock("@/lib/tauri", () => ({
  api: {
    getMessages: vi.fn(async () => []),
    listConversations: vi.fn(async () => []),
    listTools: vi.fn(async () => []),
    listProjects: vi.fn(async () => []),
    listBranches: vi.fn(async () => []),
    listSnapshots: vi.fn(async () => []),
    listTransactions: vi.fn(async () => []),
    listTurnTimeline: vi.fn(async () => []),
    listDiagnostics: vi.fn(async () => []),
    subscribeConversationSync: vi.fn(async () => () => undefined),
    subscribeConversationQueue: vi.fn(async () => () => undefined),
    getSurface: vi.fn(async () => null),
    windowOrchestratorRestore: vi.fn(async () => undefined),
    getAiStatus: vi.fn(async () => ({ provider: "mock", model: "mock", disclosure: {} })),
  },
  TauriCommandError: class TauriCommandError extends Error {},
  listenAgentTurn: vi.fn(async () => () => undefined),
  listenApprovalsChanged: vi.fn(async () => () => undefined),
  listenApplicationDataChanged: vi.fn(async () => () => undefined),
  listenAppearanceChanged: vi.fn(async () => () => undefined),
  listenThemePresetsChanged: vi.fn(async () => () => undefined),
  shouldShowAppConflict: () => false,
  shouldApplyAgentTurnSync: () => false,
  isTauriRuntime: () => false,
  isWebPreview: () => true,
}));

let resizeCallbacks: Array<(entries: Array<{ contentRect: { width: number } }>) => void> = [];

class MockResizeObserver {
  cb: (entries: Array<{ contentRect: { width: number } }>) => void;
  constructor(cb: (entries: Array<{ contentRect: { width: number } }>) => void) {
    this.cb = cb;
    resizeCallbacks.push(cb);
  }
  observe() {}
  unobserve() {}
  disconnect() {
    resizeCallbacks = resizeCallbacks.filter((c) => c !== this.cb);
  }
}

describe("WorkspaceView", () => {
  const mockCreateConversation = vi.fn(async () => undefined);
  const mockNavigateToAutomations = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    resizeCallbacks = [];
    localStorage.clear();

    // Stub Element pointer capture for jsdom
    if (!Element.prototype.setPointerCapture) {
      Element.prototype.setPointerCapture = vi.fn();
    }
    if (!Element.prototype.releasePointerCapture) {
      Element.prototype.releasePointerCapture = vi.fn();
    }

    // Polyfill ResizeObserver with our test controller
    globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

    // Reset workspace store to clean baseline
    useWorkspaceStore.setState(createDefaultWorkspaceState());

    // Reset app store state
    useAppStore.setState({
      createConversation: mockCreateConversation,
      navigateToAutomations: mockNavigateToAutomations,
      resolvedTheme: "dark",
      conversations: [],
      tools: [],
      projects: [],
      activeConversationId: null,
      activeToolId: null,
      activeProjectId: null,
    });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe("Multi-Pane Layout Rendering (1, 2, 3, 4 Panes)", () => {
    it("renders single pane layout by default", () => {
      render(<WorkspaceView />);

      expect(screen.getByTestId("workspace-view")).toBeInTheDocument();
      expect(screen.getByTestId("workspace-grid-single")).toBeInTheDocument();

      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes).toHaveLength(1);
      expect(panes[0]).toHaveAttribute("data-pane-id", "pane-1");
      expect(panes[0]).toHaveClass("focused");
    });

    it("renders 2-pane vertical split layout with draggable splitter divider", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      render(<WorkspaceView />);

      expect(screen.getByTestId("workspace-grid-split-vertical")).toBeInTheDocument();
      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes).toHaveLength(2);
      expect(panes[0]).toHaveAttribute("data-pane-id", "pane-1");
      expect(panes[1]).toHaveAttribute("data-pane-id", "pane-2");

      const splitter = screen.getByRole("separator", { name: "Pane divider" });
      expect(splitter).toBeInTheDocument();
      expect(splitter).toHaveClass("vertical");
    });

    it("renders 2-pane horizontal split layout with horizontal splitter divider", () => {
      useWorkspaceStore.getState().setLayout("split-horizontal");

      render(<WorkspaceView />);

      expect(screen.getByTestId("workspace-grid-split-horizontal")).toBeInTheDocument();
      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes).toHaveLength(2);
      expect(panes[0]).toHaveAttribute("data-pane-id", "pane-1");
      expect(panes[1]).toHaveAttribute("data-pane-id", "pane-2");

      const splitter = screen.getByRole("separator", { name: "Horizontal pane divider" });
      expect(splitter).toBeInTheDocument();
      expect(splitter).toHaveClass("horizontal");
    });

    it("renders 3-pane layout with left main pane and right stacked panes", () => {
      useWorkspaceStore.getState().setLayout("split-three");

      render(<WorkspaceView />);

      expect(screen.getByTestId("workspace-grid-split-three")).toBeInTheDocument();
      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes).toHaveLength(3);
      expect(panes[0]).toHaveAttribute("data-pane-id", "pane-1");
      expect(panes[1]).toHaveAttribute("data-pane-id", "pane-2");
      expect(panes[2]).toHaveAttribute("data-pane-id", "pane-3");
    });

    it("renders 4-pane 2x2 grid layout", () => {
      useWorkspaceStore.getState().setLayout("split-grid");

      render(<WorkspaceView />);

      expect(screen.getByTestId("workspace-grid-split-grid")).toBeInTheDocument();
      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes).toHaveLength(4);
      expect(panes[0]).toHaveAttribute("data-pane-id", "pane-1");
      expect(panes[1]).toHaveAttribute("data-pane-id", "pane-2");
      expect(panes[2]).toHaveAttribute("data-pane-id", "pane-3");
      expect(panes[3]).toHaveAttribute("data-pane-id", "pane-4");
    });

    it("falls back to compact single pane view with pane switcher pills when container is narrow (<700px)", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      render(<WorkspaceView />);

      // Initially wide: split-vertical
      expect(screen.getByTestId("workspace-grid-split-vertical")).toBeInTheDocument();

      // Trigger ResizeObserver resize callback to simulate compact container (<700px)
      act(() => {
        resizeCallbacks.forEach((cb) => cb([{ contentRect: { width: 600 } }]));
      });

      // Compact view fallback should now be active
      expect(screen.getByTestId("workspace-grid-single")).toBeInTheDocument();
      const pillSelector = screen.getByRole("tablist", { name: "Panes" });
      expect(pillSelector).toBeInTheDocument();

      const pane1Pill = screen.getByRole("button", { name: "Pane 1" });
      const pane2Pill = screen.getByRole("button", { name: "Pane 2" });
      expect(pane1Pill).toHaveClass("active");

      // Switching pane via compact selector pill
      fireEvent.click(pane2Pill);
      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-2");
    });
  });

  describe("Pane Focus Transitions", () => {
    it("transitions focused pane when clicking another pane", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      render(<WorkspaceView />);

      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes[0]).toHaveClass("focused");
      expect(panes[1]).not.toHaveClass("focused");

      // Click pane-2
      fireEvent.click(panes[1]);

      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-2");
      expect(panes[1]).toHaveClass("focused");
      expect(panes[0]).not.toHaveClass("focused");
    });
  });

  describe("Tab Strip Interaction & Management", () => {
    it("renders tabs belonging to the focused pane with active tab state", () => {
      useAppStore.setState({
        conversations: [
          { id: "c-a", title: "Chat Alpha", pinned: false, archived: false, createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z" },
          { id: "c-b", title: "Chat Beta", pinned: false, archived: false, createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z" },
        ],
      });

      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({ kind: "chat", title: "Chat Alpha", conversationId: "c-a" });
      store.openTab({ kind: "chat", title: "Chat Beta", conversationId: "c-b" });

      render(<WorkspaceView />);

      const tabStrip = screen.getByTestId("workspace-tab-strip");
      expect(tabStrip).toBeInTheDocument();

      const tabAlpha = screen.getByRole("tab", { name: /Chat Alpha/i });
      const tabBeta = screen.getByRole("tab", { name: /Chat Beta/i });

      expect(tabAlpha).toHaveAttribute("aria-selected", "false");
      expect(tabBeta).toHaveAttribute("aria-selected", "true");

      // Clicking tabAlpha activates it
      fireEvent.click(tabAlpha);
      expect(tabAlpha).toHaveAttribute("aria-selected", "true");
      expect(useWorkspaceStore.getState().panes[0].activeTabId).toBe(tab1);
    });

    it("adds a new tab when clicking the + button in the tab strip", () => {
      render(<WorkspaceView />);

      const initialCount = useWorkspaceStore.getState().panes[0].tabIds.length;
      const addBtn = screen.getByRole("button", { name: "New tab" });
      fireEvent.click(addBtn);

      const nextCount = useWorkspaceStore.getState().panes[0].tabIds.length;
      expect(nextCount).toBe(initialCount + 1);
    });

    it("closes a tab when clicking the tab close button (X)", () => {
      useAppStore.setState({
        conversations: [
          { id: "c-1", title: "Closeable One", pinned: false, archived: false, createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z" },
          { id: "c-2", title: "Closeable Two", pinned: false, archived: false, createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z" },
        ],
      });

      const store = useWorkspaceStore.getState();
      store.openTab({ kind: "chat", title: "Closeable One", conversationId: "c-1" });
      const tab2 = store.openTab({ kind: "chat", title: "Closeable Two", conversationId: "c-2" });

      render(<WorkspaceView />);

      expect(screen.getByRole("tab", { name: /Closeable One/i })).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /Closeable Two/i })).toBeInTheDocument();

      const closeBtn = screen.getByRole("button", { name: "Close Closeable Two tab" });
      fireEvent.click(closeBtn);

      expect(screen.queryByRole("tab", { name: /Closeable Two/i })).not.toBeInTheDocument();
      expect(useWorkspaceStore.getState().tabs[tab2]).toBeUndefined();
    });
  });

  describe("Split Control Buttons", () => {
    it("switches layout using the split control buttons in the tab strip", () => {
      render(<WorkspaceView />);

      const splitVerticalBtn = screen.getByRole("button", {
        name: "Split view vertical (2 panes)",
      });
      const splitThreeBtn = screen.getByRole("button", {
        name: "Split view (3 panes)",
      });
      const splitGridBtn = screen.getByRole("button", {
        name: "Split view grid (4 panes)",
      });
      const singleBtn = screen.getByRole("button", {
        name: "Single pane view",
      });

      expect(singleBtn).toHaveAttribute("aria-pressed", "true");

      // Click vertical split
      fireEvent.click(splitVerticalBtn);
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-vertical");
      expect(splitVerticalBtn).toHaveAttribute("aria-pressed", "true");

      // Click horizontal split
      const splitHorizontalBtn = screen.getByRole("button", {
        name: "Split view horizontal (2 panes)",
      });
      fireEvent.click(splitHorizontalBtn);
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-horizontal");
      expect(splitHorizontalBtn).toHaveAttribute("aria-pressed", "true");

      // Click 3-pane split
      fireEvent.click(splitThreeBtn);
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-three");

      // Click 4-pane grid
      fireEvent.click(splitGridBtn);
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-grid");

      // Click single view
      fireEvent.click(singleBtn);
      expect(useWorkspaceStore.getState().layoutKind).toBe("single");
    });
  });

  describe("Dashboard Quick Actions & Prefill Custom Events", () => {
    it("renders the dashboard greeting and 4 quick action cards for a fresh tab", () => {
      render(<WorkspaceView />);

      const dashboard = screen.getByTestId("workspace-dashboard");
      expect(dashboard).toBeInTheDocument();

      // Heading greeting
      expect(
        screen.getByRole("heading", { name: /Good morning|Good afternoon|Good evening/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByText("What would you like to work on today?"),
      ).toBeInTheDocument();

      // 4 Quick action cards
      expect(screen.getByText("Start a new conversation")).toBeInTheDocument();
      expect(screen.getByText("Summarize a document")).toBeInTheDocument();
      expect(screen.getByText("Create an application")).toBeInTheDocument();
      expect(screen.getByText("Set up an automation")).toBeInTheDocument();
    });

    it("triggers createConversation when clicking 'Start a new conversation'", () => {
      render(<WorkspaceView />);

      const startCard = screen.getByRole("button", {
        name: /Start a new conversation/i,
      });
      fireEvent.click(startCard);

      expect(mockCreateConversation).toHaveBeenCalledTimes(1);
    });

    it("triggers navigateToAutomations when clicking 'Set up an automation'", () => {
      render(<WorkspaceView />);

      const autoCard = screen.getByRole("button", {
        name: /Set up an automation/i,
      });
      fireEvent.click(autoCard);

      expect(mockNavigateToAutomations).toHaveBeenCalledTimes(1);
    });

    it("dispatches 'coreside:prefill-composer' event with text on clicking 'Summarize a document'", () => {
      render(<WorkspaceView />);

      const eventSpy = vi.fn();
      window.addEventListener("coreside:prefill-composer", eventSpy);

      const summarizeCard = screen.getByRole("button", {
        name: /Summarize a document/i,
      });
      fireEvent.click(summarizeCard);

      expect(eventSpy).toHaveBeenCalledTimes(1);
      const detail = eventSpy.mock.calls[0][0].detail;
      expect(detail.text).toBe("Please summarize the following document: ");

      window.removeEventListener("coreside:prefill-composer", eventSpy);
    });

    it("dispatches 'coreside:prefill-composer' event on clicking 'Create an application'", () => {
      render(<WorkspaceView />);

      const eventSpy = vi.fn();
      window.addEventListener("coreside:prefill-composer", eventSpy);

      const appCard = screen.getByRole("button", {
        name: /Create an application/i,
      });
      fireEvent.click(appCard);

      expect(eventSpy).toHaveBeenCalledTimes(1);
      const detail = eventSpy.mock.calls[0][0].detail;
      expect(detail.text).toBe("Build a personal interactive application for ");

      window.removeEventListener("coreside:prefill-composer", eventSpy);
    });

    it("dispatches 'coreside:prefill-composer' event when clicking a suggested prompt row", () => {
      render(<WorkspaceView />);

      const eventSpy = vi.fn();
      window.addEventListener("coreside:prefill-composer", eventSpy);

      const suggestionBtn = screen.getByRole("button", {
        name: /Help me plan my day/i,
      });
      fireEvent.click(suggestionBtn);

      expect(eventSpy).toHaveBeenCalledTimes(1);
      const detail = eventSpy.mock.calls[0][0].detail;
      expect(detail.text).toBe("Help me plan my day");

      window.removeEventListener("coreside:prefill-composer", eventSpy);
    });
  });

  describe("Keyboard Shortcuts in WorkspaceView", () => {
    it("opens a new tab on Cmd+T", () => {
      render(<WorkspaceView />);

      const before = useWorkspaceStore.getState().panes[0].tabIds.length;

      fireEvent.keyDown(window, { key: "t", metaKey: true });

      const after = useWorkspaceStore.getState().panes[0].tabIds.length;
      expect(after).toBe(before + 1);
    });

    it("closes current active tab on Cmd+W", () => {
      const store = useWorkspaceStore.getState();
      store.openTab({ kind: "chat", title: "T1", conversationId: "c1" });
      const tab2 = store.openTab({ kind: "chat", title: "T2", conversationId: "c2" });

      render(<WorkspaceView />);

      expect(useWorkspaceStore.getState().panes[0].activeTabId).toBe(tab2);

      fireEvent.keyDown(window, { key: "w", metaKey: true });

      expect(useWorkspaceStore.getState().tabs[tab2]).toBeUndefined();
    });

    it("switches layouts on Cmd+Alt+1..4 shortcuts", () => {
      render(<WorkspaceView />);

      // Cmd+Alt+2 -> split-vertical
      fireEvent.keyDown(window, { key: "2", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-vertical");

      // Cmd+Alt+3 -> split-three
      fireEvent.keyDown(window, { key: "3", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-three");

      // Cmd+Alt+4 -> split-grid
      fireEvent.keyDown(window, { key: "4", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-grid");

      // Cmd+Alt+H -> split-horizontal
      fireEvent.keyDown(window, { key: "h", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().layoutKind).toBe("split-horizontal");

      // Cmd+Alt+1 -> single
      fireEvent.keyDown(window, { key: "1", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().layoutKind).toBe("single");
    });

    it("switches pane focus on Cmd+Alt+ArrowRight / ArrowLeft", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      render(<WorkspaceView />);

      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-1");

      // ArrowRight -> next pane (pane-2)
      fireEvent.keyDown(window, { key: "ArrowRight", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-2");

      // ArrowLeft -> previous pane (pane-1)
      fireEvent.keyDown(window, { key: "ArrowLeft", metaKey: true, altKey: true });
      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-1");
    });
  });

  describe("Multi-Pane Isolation", () => {
    it("maintains independent active tabs and content per pane", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      const t1 = useWorkspaceStore.getState().openTab(
        { kind: "chat", title: "Pane 1 Conversation", conversationId: "conv-p1" },
        "pane-1",
      );

      const t2 = useWorkspaceStore.getState().openTab(
        { kind: "chat", title: "Pane 2 Conversation", conversationId: "conv-p2" },
        "pane-2",
      );

      render(<WorkspaceView />);

      const state = useWorkspaceStore.getState();
      expect(state.panes[0].activeTabId).toBe(t1);
      expect(state.panes[1].activeTabId).toBe(t2);

      // Pane 1 and Pane 2 render simultaneously
      const panes = document.querySelectorAll(".workspace-pane");
      expect(panes).toHaveLength(2);
      expect(panes[0]).toHaveAttribute("data-pane-id", "pane-1");
      expect(panes[1]).toHaveAttribute("data-pane-id", "pane-2");
    });
  });

  describe("Tab Drag/Keyboard Reordering & Gated Reconciliation", () => {
    it("reorders tabs when using Alt+Arrow keys and drag/drop", () => {
      const store = useWorkspaceStore.getState();
      const t1 = store.openTab({ kind: "chat", title: "Tab Alpha", conversationId: "c1" });
      const t2 = store.openTab({ kind: "chat", title: "Tab Bravo", conversationId: "c2" });

      render(<WorkspaceView />);

      const tabAlpha = screen.getByRole("tab", { name: /Tab Alpha/i });
      const tabBravo = screen.getByRole("tab", { name: /Tab Bravo/i });

      // Alt+ArrowRight on Tab Alpha moves it after Tab Bravo
      fireEvent.keyDown(tabAlpha, { key: "ArrowRight", altKey: true });
      expect(useWorkspaceStore.getState().panes[0].tabIds).toEqual([t2, t1]);

      // Drag and drop Tab Alpha back onto Tab Bravo
      fireEvent.drop(tabBravo, {
        dataTransfer: {
          getData: () => t1,
        },
      });
      expect(useWorkspaceStore.getState().panes[0].tabIds).toEqual([t1, t2]);
    });

    it("does not wipe workspace tabs when app is not bootstrapped yet", () => {
      useAppStore.setState({
        bootstrapped: false,
        conversations: [], // empty before hydration
      });

      const store = useWorkspaceStore.getState();
      const t1 = store.openTab({ kind: "chat", title: "Saved Chat", conversationId: "conv-saved" });

      render(<WorkspaceView />);

      // Should still preserve conversationId because bootstrapped is false
      expect(useWorkspaceStore.getState().tabs[t1].conversationId).toBe("conv-saved");
    });
  });
});
