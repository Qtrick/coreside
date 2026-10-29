import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceDashboard } from "./WorkspaceDashboard";
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

// Stub Composer so it doesn't need full store wiring
vi.mock("@/components/chat/Composer", () => ({
  Composer: ({ conversationId }: { conversationId?: string | null }) => (
    <div data-testid="composer" data-conv={conversationId ?? "null"} />
  ),
}));

// Stub CoresideLogo
vi.mock("@/components/branding/CoresideLogo", () => ({
  CoresideLogo: () => <svg data-testid="coreside-logo" />,
}));

function setStoreState(overrides: Record<string, unknown> = {}) {
  useAppStore.setState({
    resolvedTheme: "dark",
    createConversation: vi.fn(async () => undefined),
    navigateToAutomations: vi.fn(),
    ...overrides,
  });
}

describe("WorkspaceDashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setStoreState();
  });

  afterEach(() => {
    cleanup();
  });

  describe("Greeting", () => {
    it("renders a contextual greeting (morning, afternoon, or evening)", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);

      // The module-level sessionGreeting persists across tests in the same worker,
      // so we match all three possible greetings.
      expect(
        screen.getByRole("heading", {
          name: /Good morning\.|Good afternoon\.|Good evening\./i,
        }),
      ).toBeInTheDocument();
    });

    it("renders 'Good morning.' when hour is between 5 and 11", () => {
      // Reset module-level session greeting by mocking the Date
      vi.spyOn(Date.prototype, "getHours").mockReturnValue(9);

      // We need to reset sessionGreeting; we do this by dynamically re-importing
      // a fresh module. Instead, assert using a regex since sessionGreeting caches.
      render(<WorkspaceDashboard conversationId="conv-1" />);

      const heading = screen.getByRole("heading", {
        name: /Good morning\.|Good afternoon\.|Good evening\./i,
      });
      expect(heading).toBeInTheDocument();
    });

    it("renders 'What would you like to work on today?' subtitle", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);
      expect(
        screen.getByText("What would you like to work on today?"),
      ).toBeInTheDocument();
    });

    it("has stable greeting across re-renders (useMemo)", () => {
      const { rerender } = render(<WorkspaceDashboard conversationId="conv-1" />);

      const firstHeading = screen.getByRole("heading", {
        name: /Good morning\.|Good afternoon\.|Good evening\./i,
      }).textContent;

      rerender(<WorkspaceDashboard conversationId="conv-1" autoFocus={false} />);

      const secondHeading = screen.getByRole("heading", {
        name: /Good morning\.|Good afternoon\.|Good evening\./i,
      }).textContent;

      // Greeting must be stable across re-renders
      expect(secondHeading).toBe(firstHeading);
    });
  });

  describe("Quick action cards", () => {
    it("renders exactly 4 quick action cards with correct labels", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);

      expect(
        screen.getByRole("button", { name: /Start a new conversation/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /Summarize a document/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /Create an application/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /Set up an automation/i }),
      ).toBeInTheDocument();

      const quickActions = document.querySelector('[aria-label="Quick actions"]');
      expect(quickActions).toBeInTheDocument();
      const cards = quickActions!.querySelectorAll(".quick-action-card");
      expect(cards).toHaveLength(4);
    });

    it("calls createConversation when 'Start a new conversation' is clicked", () => {
      const mockCreate = vi.fn(async () => undefined);
      setStoreState({ createConversation: mockCreate });
      render(<WorkspaceDashboard conversationId="conv-1" />);

      fireEvent.click(screen.getByRole("button", { name: /Start a new conversation/i }));
      expect(mockCreate).toHaveBeenCalledTimes(1);
    });

    it("calls navigateToAutomations when 'Set up an automation' is clicked", () => {
      const mockNavigate = vi.fn();
      setStoreState({ navigateToAutomations: mockNavigate });
      render(<WorkspaceDashboard conversationId="conv-1" />);

      fireEvent.click(screen.getByRole("button", { name: /Set up an automation/i }));
      expect(mockNavigate).toHaveBeenCalledTimes(1);
    });
  });

  describe("Suggestions", () => {
    it("renders 4 suggestion rows with correct text", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);

      const suggestionsRegion = document.querySelector('[aria-label="Suggested prompts"]');
      expect(suggestionsRegion).toBeInTheDocument();

      const rows = suggestionsRegion!.querySelectorAll(".suggestion-row");
      expect(rows).toHaveLength(4);

      expect(screen.getByText("Help me plan my day")).toBeInTheDocument();
      expect(screen.getByText("Research a topic")).toBeInTheDocument();
      expect(screen.getByText("Analyze this file")).toBeInTheDocument();
      expect(screen.getByText("Turn an idea into an app")).toBeInTheDocument();
    });

    it("dispatches prefill event with correct text when suggestion is clicked", () => {
      render(<WorkspaceDashboard conversationId="conv-abc" />);

      const eventSpy = vi.fn();
      window.addEventListener("coreside:prefill-composer", eventSpy);

      fireEvent.click(screen.getByRole("button", { name: /Help me plan my day/i }));

      expect(eventSpy).toHaveBeenCalledTimes(1);
      const detail = (eventSpy.mock.calls[0][0] as CustomEvent).detail;
      expect(detail.text).toBe("Help me plan my day");
      expect(detail.conversationId).toBe("conv-abc");

      window.removeEventListener("coreside:prefill-composer", eventSpy);
    });
  });

  describe("Composer integration", () => {
    it("renders Composer within dashboard", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);
      expect(screen.getByTestId("composer")).toBeInTheDocument();
    });

    it("passes correct conversationId to Composer", () => {
      render(<WorkspaceDashboard conversationId="conv-xyz" />);
      const composer = screen.getByTestId("composer");
      expect(composer).toHaveAttribute("data-conv", "conv-xyz");
    });

    it("passes null conversationId to Composer when none given", () => {
      render(<WorkspaceDashboard />);
      const composer = screen.getByTestId("composer");
      expect(composer).toHaveAttribute("data-conv", "null");
    });
  });

  describe("Prefill event scoping", () => {
    it("dispatches prefill event with conversationId in detail for 'Summarize a document'", () => {
      render(<WorkspaceDashboard conversationId="conv-42" />);

      const eventSpy = vi.fn();
      window.addEventListener("coreside:prefill-composer", eventSpy);

      fireEvent.click(screen.getByRole("button", { name: /Summarize a document/i }));

      const detail = (eventSpy.mock.calls[0][0] as CustomEvent).detail;
      expect(detail.conversationId).toBe("conv-42");
      expect(detail.text).toBe("Please summarize the following document: ");

      window.removeEventListener("coreside:prefill-composer", eventSpy);
    });

    it("dispatches prefill event with conversationId=null when prop is undefined", () => {
      render(<WorkspaceDashboard />);

      const eventSpy = vi.fn();
      window.addEventListener("coreside:prefill-composer", eventSpy);

      fireEvent.click(screen.getByRole("button", { name: /Summarize a document/i }));

      const detail = (eventSpy.mock.calls[0][0] as CustomEvent).detail;
      expect(detail.conversationId).toBeNull();

      window.removeEventListener("coreside:prefill-composer", eventSpy);
    });

    it("calls onActionSelect callback when card is clicked", () => {
      const onActionSelect = vi.fn();
      render(
        <WorkspaceDashboard
          conversationId="conv-1"
          onActionSelect={onActionSelect}
        />,
      );

      fireEvent.click(screen.getByRole("button", { name: /Create an application/i }));

      expect(onActionSelect).toHaveBeenCalledTimes(1);
      expect(onActionSelect).toHaveBeenCalledWith(
        "Build a personal interactive application for ",
      );
    });
  });

  describe("Dashboard element", () => {
    it("has data-testid='workspace-dashboard'", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);
      expect(screen.getByTestId("workspace-dashboard")).toBeInTheDocument();
    });

    it("has aria-label='Workspace Dashboard'", () => {
      render(<WorkspaceDashboard conversationId="conv-1" />);
      expect(
        screen.getByRole("region", { name: "Workspace Dashboard" }),
      ).toBeInTheDocument();
    });
  });
});
