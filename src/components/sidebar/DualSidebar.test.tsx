import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";
import { useAppStore } from "@/stores/app-store";
import type { Conversation } from "@/types/messages";
import type { Project } from "@/types/project";
import type { ToolSummary } from "@/types/tool";

vi.mock("@/lib/tauri", () => ({
  api: {
    getMessages: vi.fn(async () => []),
    subscribeConversationSync: vi.fn(async () => () => undefined),
    getSurface: vi.fn(async () => null),
  },
  TauriCommandError: class TauriCommandError extends Error {},
  listenAgentTurn: vi.fn(async () => () => undefined),
  shouldShowAppConflict: () => false,
  shouldApplyAgentTurnSync: () => false,
}));

describe("DualSidebar Architecture", () => {
  const mockToggleSidebar = vi.fn();
  const mockNavigateToChat = vi.fn(async () => undefined);
  const mockNavigateToProject = vi.fn(async () => undefined);
  const mockNavigateToProjects = vi.fn();
  const mockNavigateToSettings = vi.fn();
  const mockNavigateToMedia = vi.fn();
  const mockNavigateToAutomations = vi.fn();
  const mockCreateConversation = vi.fn(async () => undefined);
  const mockSelectTool = vi.fn(async () => undefined);
  const mockSetTheme = vi.fn();
  const mockSetProjectsExpanded = vi.fn();
  const mockSetCreateProjectDialogOpen = vi.fn();
  const mockSetRenameConversationDialogOpen = vi.fn();
  const mockSetDeleteConversationDialogOpen = vi.fn();
  const mockAssignChats = vi.fn(async () => undefined);
  const mockRemoveChatFromProject = vi.fn(async () => undefined);
  const mockDuplicateConversation = vi.fn(async () => undefined);

  const sampleConversations: Conversation[] = [
    {
      id: "conv-1",
      title: "Design System Discussion",
      createdAt: "2026-10-28T09:00:00Z",
      updatedAt: "2026-10-28T10:00:00Z",
      projectId: "proj-1",
      pinned: false,
      archived: false,
    },
    {
      id: "conv-2",
      title: "Rust Architecture Review",
      createdAt: "2026-10-27T09:00:00Z",
      updatedAt: "2026-10-27T10:00:00Z",
      projectId: null,
      pinned: false,
      archived: false,
    },
  ];

  const sampleProjects: Project[] = [
    {
      id: "proj-1",
      name: "Core Infrastructure",
      description: "Low level system architecture and engine",
      archived: false,
      pinned: true,
      createdAt: "2026-10-01T00:00:00Z",
      updatedAt: "2026-10-28T10:00:00Z",
    },
  ];

  const sampleTools: ToolSummary[] = [
    {
      id: "tool-1",
      name: "Design Token Explorer",
      description: "Inspect colors, spacing, and typography",
      version: 2,
    },
    {
      id: "tool-2",
      name: "Log Inspector",
      description: "Live system output viewer",
      version: 1,
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();

    Object.defineProperty(window, "matchMedia", {
      writable: true,
      configurable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });

    useAppStore.setState({
      sidebarCollapsed: false,
      toggleSidebar: mockToggleSidebar,
      view: { kind: "chat", conversationId: "conv-1" },
      resolvedTheme: "dark",
      theme: "dark",
      setTheme: mockSetTheme,
      navigateToChat: mockNavigateToChat,
      navigateToProject: mockNavigateToProject,
      navigateToProjects: mockNavigateToProjects,
      navigateToSettings: mockNavigateToSettings,
      navigateToMedia: mockNavigateToMedia,
      navigateToAutomations: mockNavigateToAutomations,
      createConversation: mockCreateConversation,
      selectTool: mockSelectTool,
      activeConversationId: "conv-1",
      activeToolId: null,
      activeProjectId: null,
      conversations: sampleConversations,
      projects: sampleProjects,
      projectsExpanded: true,
      setProjectsExpanded: mockSetProjectsExpanded,
      tools: sampleTools,
      setCreateProjectDialogOpen: mockSetCreateProjectDialogOpen,
      setRenameConversationDialogOpen: mockSetRenameConversationDialogOpen,
      setDeleteConversationDialogOpen: mockSetDeleteConversationDialogOpen,
      assignChats: mockAssignChats,
      removeChatFromProject: mockRemoveChatFromProject,
      duplicateConversation: mockDuplicateConversation,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Dual-Sidebar Composition & IconRail Navigation", () => {
    it("renders the dual-sidebar container with both IconRail and ContextualSidebar", () => {
      render(<Sidebar />);

      const sidebar = screen.getByTestId("dual-sidebar");
      expect(sidebar).toBeInTheDocument();
      expect(sidebar).not.toHaveClass("collapsed");

      expect(screen.getByTestId("icon-rail")).toBeInTheDocument();
      expect(screen.getByTestId("contextual-sidebar")).toBeInTheDocument();
    });

    it("navigates to media, automations, and settings from IconRail buttons", () => {
      render(<Sidebar />);

      // Media button
      const mediaBtn = screen.getByRole("button", { name: "Media library" });
      fireEvent.click(mediaBtn);
      expect(mockNavigateToMedia).toHaveBeenCalledTimes(1);

      // Automations button
      const autoBtn = screen.getByRole("button", { name: "Automations" });
      fireEvent.click(autoBtn);
      expect(mockNavigateToAutomations).toHaveBeenCalledTimes(1);

      // Settings button
      const settingsBtn = screen.getByRole("button", { name: "Settings" });
      fireEvent.click(settingsBtn);
      expect(mockNavigateToSettings).toHaveBeenCalledTimes(1);
    });

    it("switches contextual mode to Apps when clicking the Apps IconRail button", () => {
      render(<Sidebar />);

      const appsRailBtn = screen.getByRole("button", { name: "Personal Apps" });
      fireEvent.click(appsRailBtn);

      // Should now render the Apps view mode with personal apps section
      expect(screen.getByText("Personal Apps")).toBeInTheDocument();
      expect(screen.getByTestId("contextual-sidebar")).toBeInTheDocument();
      expect(screen.getByText("Design Token Explorer")).toBeInTheDocument();
    });

    it("switches back to Workspace mode when clicking the Workspace IconRail button", () => {
      render(<Sidebar />);

      // Switch to apps first
      fireEvent.click(screen.getByRole("button", { name: "Personal Apps" }));
      expect(screen.getByText("Personal Apps")).toBeInTheDocument();

      // Click workspace rail button
      fireEvent.click(screen.getByRole("button", { name: "Chats and Projects" }));
      expect(screen.getByText("PROJECTS")).toBeInTheDocument();
      expect(screen.getByText("Chats")).toBeInTheDocument();
    });
  });

  describe("Sole Profile Popover at Bottom of IconRail", () => {
    it("toggles the profile popover on clicking the avatar button", () => {
      render(<Sidebar />);

      const avatarBtn = screen.getByRole("button", {
        name: "Personal profile and quick settings",
      });
      expect(avatarBtn).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByRole("dialog", { name: "Profile menu" })).not.toBeInTheDocument();

      // Open popover
      fireEvent.click(avatarBtn);
      expect(avatarBtn).toHaveAttribute("aria-expanded", "true");
      const dialog = screen.getByRole("dialog", { name: "Profile menu" });
      expect(dialog).toBeInTheDocument();
      expect(screen.getByText("Local Profile")).toBeInTheDocument();
      expect(screen.getByText("Offline Local-First")).toBeInTheDocument();

      // Toggle theme button
      const themeBtn = screen.getByRole("button", { name: /Light theme|Dark theme/i });
      expect(themeBtn).toBeInTheDocument();
      fireEvent.click(themeBtn);
      expect(mockSetTheme).toHaveBeenCalledWith("light");
      // Popover closes after action
      expect(screen.queryByRole("dialog", { name: "Profile menu" })).not.toBeInTheDocument();
    });

    it("navigates to settings and closes popover when clicking 'All Settings'", () => {
      render(<Sidebar />);

      const avatarBtn = screen.getByRole("button", {
        name: "Personal profile and quick settings",
      });
      fireEvent.click(avatarBtn);

      const allSettingsBtn = screen.getByRole("button", { name: "All Settings" });
      fireEvent.click(allSettingsBtn);

      expect(mockNavigateToSettings).toHaveBeenCalledTimes(1);
      expect(screen.queryByRole("dialog", { name: "Profile menu" })).not.toBeInTheDocument();
    });

    it("closes the profile popover when clicking outside", () => {
      render(<Sidebar />);

      const avatarBtn = screen.getByRole("button", {
        name: "Personal profile and quick settings",
      });
      fireEvent.click(avatarBtn);
      expect(screen.getByRole("dialog", { name: "Profile menu" })).toBeInTheDocument();

      // Click outside on document body
      fireEvent.mouseDown(document.body);
      expect(screen.queryByRole("dialog", { name: "Profile menu" })).not.toBeInTheDocument();
    });

    it("closes the profile popover on Escape key", () => {
      render(<Sidebar />);

      const avatarBtn = screen.getByRole("button", {
        name: "Personal profile and quick settings",
      });
      fireEvent.click(avatarBtn);
      expect(screen.getByRole("dialog", { name: "Profile menu" })).toBeInTheDocument();

      // Press Escape
      fireEvent.keyDown(document, { key: "Escape" });
      expect(screen.queryByRole("dialog", { name: "Profile menu" })).not.toBeInTheDocument();
    });
  });

  describe("ContextualSidebar Switcher ('Chats & Projects | Apps')", () => {
    it("renders the segmented switcher with both options", () => {
      render(<Sidebar />);

      const switcher = screen.getByRole("tablist", { name: "Sidebar view modes" });
      expect(switcher).toBeInTheDocument();

      const chatsTab = screen.getByRole("tab", { name: /Chats & Projects/i });
      const appsTab = screen.getByRole("tab", { name: /Apps/i });

      expect(chatsTab).toHaveAttribute("aria-selected", "true");
      expect(appsTab).toHaveAttribute("aria-selected", "false");
    });

    it("switches to Apps mode when clicking the Apps segmented tab", () => {
      render(<Sidebar />);

      const appsTab = screen.getByRole("tab", { name: /Apps/i });
      fireEvent.click(appsTab);

      expect(appsTab).toHaveAttribute("aria-selected", "true");
      expect(screen.getByText("Personal Apps")).toBeInTheDocument();
      expect(screen.getByText("Design Token Explorer")).toBeInTheDocument();
    });
  });

  describe("Unified Search & Keyboard Navigation (Cmd+K)", () => {
    it("focuses and selects the search input on Cmd+K shortcut", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });
      expect(document.activeElement).not.toBe(searchInput);

      // Trigger Cmd+K
      fireEvent.keyDown(window, { key: "k", metaKey: true });
      expect(document.activeElement).toBe(searchInput);
    });

    it("displays live search results dropdown matching chats, projects, and apps", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "Design" } });

      const listbox = screen.getByRole("listbox", { name: "Search results" });
      expect(listbox).toBeInTheDocument();

      // Chat match
      expect(screen.getByRole("option", { name: /Design System Discussion/i })).toBeInTheDocument();
      // App match
      expect(screen.getByRole("option", { name: /Design Token Explorer/i })).toBeInTheDocument();
    });

    it("navigates to the selected chat upon clicking a search result row and clears search", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "Design System" } });

      const option = screen.getByRole("option", { name: /Design System Discussion/i });
      fireEvent.click(option);

      expect(mockNavigateToChat).toHaveBeenCalledWith("conv-1");
      expect((searchInput as HTMLInputElement).value).toBe("");
    });

    it("shows 'No matching chats, projects, or apps' when query yields 0 matches", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "nonexistent-item-query" } });

      expect(
        screen.getByText("No matching chats, projects, or apps"),
      ).toBeInTheDocument();
    });

    it("clears search input when clicking the clear (X) button", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "Rust" } });
      expect((searchInput as HTMLInputElement).value).toBe("Rust");

      const clearBtn = screen.getByRole("button", { name: "Clear search" });
      fireEvent.click(clearBtn);

      expect((searchInput as HTMLInputElement).value).toBe("");
    });

    it("navigates search results with ArrowDown/ArrowUp and selects with Enter", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "Architecture" } });

      // There should be 2 matches: "Rust Architecture Review" (chat) and "Core Infrastructure" (project with Architecture in desc)
      fireEvent.keyDown(searchInput, { key: "ArrowDown" });
      fireEvent.keyDown(searchInput, { key: "Enter" });

      expect(
        mockNavigateToChat.mock.calls.length + mockNavigateToProject.mock.calls.length,
      ).toBeGreaterThan(0);
    });

    it("escapes and closes search dropdown on Escape key", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "Design" } });
      expect(screen.getByRole("listbox")).toBeInTheDocument();

      fireEvent.keyDown(searchInput, { key: "Escape" });
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect((searchInput as HTMLInputElement).value).toBe("");
    });

    it("closes search dropdown when clicking outside", () => {
      render(<Sidebar />);

      const searchInput = screen.getByRole("searchbox", {
        name: "Search chats, projects, and apps",
      });

      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: "Design" } });
      expect(screen.getByRole("listbox")).toBeInTheDocument();

      // Click outside
      fireEvent.mouseDown(document.body);
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });
  });

  describe("Collapse Behavior", () => {
    it("calls toggleSidebar when clicking the contextual collapse button", () => {
      render(<Sidebar />);

      const collapseBtn = screen.getByRole("button", {
        name: "Collapse contextual sidebar",
      });
      fireEvent.click(collapseBtn);

      expect(mockToggleSidebar).toHaveBeenCalledTimes(1);
    });

    it("renders only IconRail when sidebar is collapsed", () => {
      useAppStore.setState({ sidebarCollapsed: true });

      render(<Sidebar />);

      const sidebar = screen.getByTestId("dual-sidebar");
      expect(sidebar).toHaveClass("collapsed");
      expect(screen.getByTestId("icon-rail")).toBeInTheDocument();
      expect(screen.queryByTestId("contextual-sidebar")).not.toBeInTheDocument();
    });
  });

  describe("Tour Targets and Action Triggers", () => {
    it("includes required tour target attributes", () => {
      render(<Sidebar />);

      // sidebar-new-chat on New chat button
      const newChatBtn = screen.getByRole("button", { name: "New chat" });
      expect(newChatBtn).toHaveAttribute("data-coreside-tour", "sidebar-new-chat");

      // sidebar-projects on Projects section
      const projectsSection = screen.getByRole("region", { name: "Projects" });
      expect(projectsSection).toHaveAttribute("data-coreside-tour", "sidebar-projects");

      // Switch to Apps mode to inspect sidebar-apps tour target
      fireEvent.click(screen.getByRole("tab", { name: /Apps/i }));
      const appsSection = screen.getByRole("region", { name: "Personal apps" });
      expect(appsSection).toHaveAttribute("data-coreside-tour", "sidebar-apps");
    });

    it("triggers createConversation when clicking + New chat", () => {
      render(<Sidebar />);

      const newChatBtn = screen.getByRole("button", { name: "New chat" });
      fireEvent.click(newChatBtn);

      expect(mockCreateConversation).toHaveBeenCalledTimes(1);
    });

    it("renders chat items with relative timestamps and handles click navigation", () => {
      render(<Sidebar />);

      const chatsSection = screen.getByRole("region", { name: "Recent chats" });
      const chatBtn = within(chatsSection).getByRole("button", {
        name: /^Design System Discussion/i,
      });
      expect(chatBtn).toBeInTheDocument();
      expect(chatBtn).toHaveClass("active"); // activeConversationId is conv-1

      fireEvent.click(chatBtn);
      expect(mockNavigateToChat).toHaveBeenCalledWith("conv-1");
    });
  });
});
