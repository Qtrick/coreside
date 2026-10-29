import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppShell } from "./AppShell";
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
  isTauriRuntime: () => false,
  isWebPreview: () => true,
}));

// Stub heavy components
vi.mock("@/components/sidebar/Sidebar", () => ({
  Sidebar: () => <aside data-testid="sidebar" />,
}));
vi.mock("@/components/workspace/WorkspaceView", () => ({
  WorkspaceView: () => <div data-testid="workspace-view" />,
}));
vi.mock("@/components/wallpaper/LiveWallpaper", () => ({
  LiveWallpaper: () => null,
}));
vi.mock("@/components/CommandPalette", () => ({
  CommandPalette: ({ open }: { open: boolean }) => (
    <div data-testid="command-palette" data-open={open ? "true" : "false"} />
  ),
}));
vi.mock("@/components/onboarding/WelcomeDialog", () => ({
  WelcomeDialog: () => null,
}));
vi.mock("@/components/onboarding/TutorialOverlay", () => ({
  TutorialOverlay: () => null,
}));
vi.mock("@/components/onboarding/ContextualEducationHost", () => ({
  ContextualEducationHost: () => null,
}));
vi.mock("@/components/providers/ProviderSetupDialog", () => ({
  ProviderSetupDialog: () => null,
}));
vi.mock("@/components/applications/PendingApprovalsHost", () => ({
  PendingApprovalsHost: () => null,
}));

function setupStore(overrides: Record<string, unknown> = {}) {
  useAppStore.setState({
    sidebarCollapsed: false,
    view: { kind: "chat", conversationId: null },
    activeToolId: null,
    activeTool: null,
    exportDialog: null,
    wallpaper: { kind: "none" } as never,
    globalWallpaperJson: null,
    interfaceTransparency: 0,
    chatToolSplitRatio: 0.5,
    layoutMode: "standard",
    projects: [],
    activeProject: null,
    projectConversations: [],
    conversations: [],
    activeConversationId: null,
    createProjectDialogOpen: false,
    editProjectDialogOpen: false,
    addChatsDialogOpen: false,
    manageContextDialogOpen: false,
    deleteProjectDialogOpen: false,
    renameConversationDialogOpen: false,
    deleteConversationDialogOpen: false,
    ...overrides,
  });
}

describe("AppShell keyboard shortcut ownership and layout chrome", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupStore();
  });

  afterEach(() => {
    cleanup();
  });

  it("opens CommandPalette on Cmd+Shift+K", () => {
    render(<AppShell />);

    const palette = screen.getByTestId("command-palette");
    expect(palette).toHaveAttribute("data-open", "false");

    fireEvent.keyDown(window, { key: "k", metaKey: true, shiftKey: true });
    expect(palette).toHaveAttribute("data-open", "true");

    fireEvent.keyDown(window, { key: "k", metaKey: true, shiftKey: true });
    expect(palette).toHaveAttribute("data-open", "false");
  });

  it("opens CommandPalette on Ctrl+Shift+K (Windows/Linux shortcut)", () => {
    render(<AppShell />);

    const palette = screen.getByTestId("command-palette");
    expect(palette).toHaveAttribute("data-open", "false");

    fireEvent.keyDown(window, { key: "k", ctrlKey: true, shiftKey: true });
    expect(palette).toHaveAttribute("data-open", "true");
  });

  it("does NOT open CommandPalette on Cmd+K alone (sidebar search shortcut)", () => {
    render(<AppShell />);

    const palette = screen.getByTestId("command-palette");
    expect(palette).toHaveAttribute("data-open", "false");

    // Cmd+K without Shift should be ignored by AppShell so ContextualSidebar can handle it
    fireEvent.keyDown(window, { key: "k", metaKey: true, shiftKey: false });
    expect(palette).toHaveAttribute("data-open", "false");
  });

  it("renders app-shell with sidebar and workspace-view", () => {
    render(<AppShell />);

    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
    expect(screen.getByTestId("workspace-view")).toBeInTheDocument();
  });
});
