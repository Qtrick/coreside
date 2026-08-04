import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ConversationHistory,
  REPLAY_STEP_MS,
  ReplayPlayer,
  redactDiagnosticJson,
  redactSecretsForDisplay,
  transactionsToReplayEvents,
} from "./ConversationHistory";

vi.mock("@/lib/tauri", () => ({
  api: {
    listBranches: vi.fn(),
    listSnapshots: vi.fn(),
    listTransactions: vi.fn(),
    listDiagnostics: vi.fn(),
    branchConversation: vi.fn(),
    createSnapshot: vi.fn(),
    getSnapshot: vi.fn(),
    applyOperations: vi.fn(),
    undoTransaction: vi.fn(),
  },
}));

vi.mock("@/stores/app-store", () => ({
  useAppStore: (
    selector: (s: {
      developerMode: boolean;
      messages: { id: string }[];
      conversations: { id: string; projectId: string | null }[];
      refreshConversations: () => Promise<void>;
      navigateToChat: (id: string) => Promise<void>;
    }) => unknown,
  ) =>
    selector({
      developerMode: true,
      messages: [{ id: "msg-1" }],
      conversations: [{ id: "conv-1", projectId: null }],
      refreshConversations: vi.fn(async () => undefined),
      navigateToChat: vi.fn(async () => undefined),
    }),
}));

import { api } from "@/lib/tauri";

const sampleEvents = transactionsToReplayEvents([
  {
    id: "txn-1",
    summary: "Created clock",
    status: "applied",
    createdAt: "2026-08-03T10:00:00Z",
    opsCount: 2,
  },
  {
    id: "txn-2",
    summary: "Updated color",
    status: "applied",
    createdAt: "2026-08-03T11:00:00Z",
    opsCount: 1,
  },
]);

describe("redactSecretsForDisplay", () => {
  it("redacts sk- and Bearer tokens", () => {
    expect(
      redactSecretsForDisplay("key=sk-abc123456789 and Bearer tokensecret99"),
    ).toBe("key=[REDACTED] and Bearer [REDACTED]");
  });

  it("redacts Gemini-style keys and api_key assignments", () => {
    expect(
      redactSecretsForDisplay(
        "AIzaSyA-test-key-value-1234567890abcd api_key=plainSecretValue99",
      ),
    ).toBe("[REDACTED] api_key=[REDACTED]");
  });

  it("redacts nested diagnostic JSON including sensitive keys", () => {
    const out = redactDiagnosticJson({
      header: "Authorization: Bearer abc.def.ghi",
      nested: { key: "sk-proj-abcdefghijklmnop" },
      apiKey: "not-sk-shaped-but-secret",
      authorization: "raw-header-value",
    });
    expect(out).not.toMatch(/sk-/);
    expect(out).not.toMatch(/Bearer abc/);
    expect(out).not.toMatch(/not-sk-shaped-but-secret/);
    expect(out).not.toMatch(/raw-header-value/);
    expect(out).toContain("[REDACTED]");
  });
});

describe("transactionsToReplayEvents", () => {
  it("maps transactions to chronological Committed transaction events", () => {
    const events = transactionsToReplayEvents([
      {
        id: "b",
        summary: "Later",
        status: "applied",
        createdAt: "2026-08-03T12:00:00Z",
        opsCount: 1,
      },
      {
        id: "a",
        summary: "Earlier",
        status: "applied",
        createdAt: "2026-08-03T10:00:00Z",
        opsCount: 3,
      },
    ]);
    expect(events.map((e) => e.id)).toEqual(["a", "b"]);
    expect(events[0]?.kind).toBe("committed_transaction");
    expect(events[0]?.summary).toBe("Committed transaction: Earlier");
  });
});

describe("ReplayPlayer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(api.applyOperations).mockReset();
    vi.mocked(api.undoTransaction).mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("steps next and previous without calling apply APIs", () => {
    render(<ReplayPlayer events={sampleEvents} />);

    expect(screen.getByText(/Live — select an event/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next replay event" }));
    expect(
      screen.getByLabelText("Current replay event summary"),
    ).toHaveTextContent("Committed transaction: Created clock");
    expect(screen.getByText("1 / 2")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next replay event" }));
    expect(
      screen.getByLabelText("Current replay event summary"),
    ).toHaveTextContent("Committed transaction: Updated color");
    expect(screen.getByText("2 / 2")).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Previous replay event" }),
    );
    expect(
      screen.getByLabelText("Current replay event summary"),
    ).toHaveTextContent("Committed transaction: Created clock");

    expect(api.applyOperations).not.toHaveBeenCalled();
    expect(api.undoTransaction).not.toHaveBeenCalled();
  });

  it("auto-plays paced steps without calling apply APIs", () => {
    render(<ReplayPlayer events={sampleEvents} />);

    fireEvent.click(screen.getByRole("button", { name: "Play replay" }));
    expect(
      screen.getByLabelText("Current replay event summary"),
    ).toHaveTextContent("Committed transaction: Created clock");

    act(() => {
      vi.advanceTimersByTime(REPLAY_STEP_MS);
    });
    expect(
      screen.getByLabelText("Current replay event summary"),
    ).toHaveTextContent("Committed transaction: Updated color");

    act(() => {
      vi.advanceTimersByTime(REPLAY_STEP_MS);
    });
    // Stays on last event and pauses — still read-only.
    expect(
      screen.getByLabelText("Current replay event summary"),
    ).toHaveTextContent("Committed transaction: Updated color");
    expect(screen.getByRole("button", { name: "Play replay" })).toBeInTheDocument();

    expect(api.applyOperations).not.toHaveBeenCalled();
    expect(api.undoTransaction).not.toHaveBeenCalled();
  });

  it("returns to live", () => {
    render(<ReplayPlayer events={sampleEvents} />);
    fireEvent.click(screen.getByRole("button", { name: "Next replay event" }));
    fireEvent.click(screen.getByRole("button", { name: "Return to live" }));
    expect(screen.getByText(/Live — select an event/)).toBeInTheDocument();
    expect(screen.getByText("Live")).toBeInTheDocument();
  });
});

describe("ConversationHistory", () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.mocked(api.listBranches).mockReset().mockResolvedValue([]);
    vi.mocked(api.listSnapshots).mockReset().mockResolvedValue([]);
    vi.mocked(api.listTransactions).mockReset().mockResolvedValue([]);
    vi.mocked(api.listDiagnostics).mockReset().mockResolvedValue([]);
    vi.mocked(api.applyOperations).mockReset();
    vi.mocked(api.undoTransaction).mockReset();
  });

  it("opens History dialog and loads branches", async () => {
    vi.mocked(api.listBranches).mockResolvedValue([
      {
        id: "br-1",
        branchName: "Explore",
        newConversationId: "conv-2",
        createdAt: "2026-08-03T12:00:00Z",
      },
    ]);

    render(<ConversationHistory conversationId="conv-1" />);

    fireEvent.click(
      screen.getByRole("button", { name: "Open conversation history" }),
    );

    expect(
      await screen.findByRole("dialog", { name: "History" }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(api.listBranches).toHaveBeenCalledWith("conv-1");
    });
    expect(screen.getByText("Explore")).toBeInTheDocument();
    expect(
      screen.getByRole("tab", { name: "Inspector" }),
    ).toBeInTheDocument();
  });

  it("loads replay player from transactions without apply APIs", async () => {
    vi.mocked(api.listTransactions).mockResolvedValue([
      {
        id: "txn-1",
        summary: "Created clock",
        status: "applied",
        createdAt: "2026-08-03T10:00:00Z",
        operations: [{}, {}],
      },
    ]);

    render(<ConversationHistory conversationId="conv-1" />);
    fireEvent.click(
      screen.getByRole("button", { name: "Open conversation history" }),
    );
    fireEvent.click(screen.getByRole("tab", { name: "Replay" }));

    expect(
      await screen.findByLabelText("Read-only replay player"),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(api.listTransactions).toHaveBeenCalledWith("conv-1", 50);
    });
    expect(
      screen.getByText("Committed transaction: Created clock"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next replay event" }));
    expect(api.applyOperations).not.toHaveBeenCalled();
    expect(api.undoTransaction).not.toHaveBeenCalled();
  });
});
