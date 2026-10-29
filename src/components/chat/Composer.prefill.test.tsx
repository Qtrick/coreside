import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
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

function setupStore(overrides: Record<string, unknown> = {}) {
  useAppStore.setState({
    activeConversationId: "conv-1",
    chatViewState: {},
    tools: [],
    aiStatus: { provider: "mock", model: "mock-model" } as never,
    sending: false,
    sendingConversationId: null,
    setChatDraft: vi.fn((convId: string, draft: string) => {
      const state = useAppStore.getState();
      useAppStore.setState({
        chatViewState: {
          ...state.chatViewState,
          [convId]: { ...(state.chatViewState[convId] ?? {}), draft },
        },
      });
    }),
    ...overrides,
  });
}

describe("Composer prefill event scoping and draft preservation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupStore();
  });

  afterEach(() => {
    cleanup();
  });

  it("populates textarea when prefill event has matching conversationId", () => {
    render(<Composer conversationId="conv-1" autoFocus={false} />);

    const textarea = screen.getByRole("textbox", { name: /message composer/i }) as HTMLTextAreaElement;
    expect(textarea.value).toBe("");

    act(() => {
      window.dispatchEvent(
        new CustomEvent("coreside:prefill-composer", {
          detail: { text: "Summarize this document", conversationId: "conv-1" },
        }),
      );
    });

    expect(textarea.value).toBe("Summarize this document");
  });

  it("ignores prefill event targeted at a different conversationId", () => {
    render(<Composer conversationId="conv-1" autoFocus={false} />);

    const textarea = screen.getByRole("textbox", { name: /message composer/i }) as HTMLTextAreaElement;
    expect(textarea.value).toBe("");

    act(() => {
      window.dispatchEvent(
        new CustomEvent("coreside:prefill-composer", {
          detail: { text: "Should not appear", conversationId: "conv-other" },
        }),
      );
    });

    expect(textarea.value).toBe("");
  });

  it("appends to an existing non-empty draft instead of overwriting it", () => {
    setupStore({
      chatViewState: {
        "conv-1": { draft: "Initial user notes" },
      },
    });

    render(<Composer conversationId="conv-1" autoFocus={false} />);

    const textarea = screen.getByRole("textbox", { name: /message composer/i }) as HTMLTextAreaElement;
    expect(textarea.value).toBe("Initial user notes");

    act(() => {
      window.dispatchEvent(
        new CustomEvent("coreside:prefill-composer", {
          detail: { text: "Help me plan my day", conversationId: "conv-1" },
        }),
      );
    });

    expect(textarea.value).toBe("Initial user notes\n\nHelp me plan my day");
  });

  it("accepts prefill event when conversationId is null on both event and composer", () => {
    setupStore({ activeConversationId: null });

    render(<Composer conversationId={null} autoFocus={false} />);

    const textarea = screen.getByRole("textbox", { name: /message composer/i }) as HTMLTextAreaElement;
    expect(textarea.value).toBe("");

    act(() => {
      window.dispatchEvent(
        new CustomEvent("coreside:prefill-composer", {
          detail: { text: "New draft text", conversationId: null },
        }),
      );
    });

    expect(textarea.value).toBe("New draft text");
  });
});
