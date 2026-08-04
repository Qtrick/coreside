import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  ConversationHistory,
  redactDiagnosticJson,
  redactSecretsForDisplay,
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

describe("ConversationHistory", () => {
  beforeEach(() => {
    vi.mocked(api.listBranches).mockReset().mockResolvedValue([]);
    vi.mocked(api.listSnapshots).mockReset().mockResolvedValue([]);
    vi.mocked(api.listTransactions).mockReset().mockResolvedValue([]);
    vi.mocked(api.listDiagnostics).mockReset().mockResolvedValue([]);
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
});
