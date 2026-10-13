import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  isAllowedMarkdownHref,
  MessageBubble,
} from "@/components/chat/MessageBubble";
import type { ChatMessage } from "@/types/messages";

const openExternalUrl = vi.fn().mockResolvedValue(undefined);

vi.mock("@/lib/open-url", () => ({
  openExternalUrl: (...args: unknown[]) => openExternalUrl(...args),
}));

vi.mock("@/stores/app-store", () => ({
  useAppStore: (
    selector: (s: Record<string, unknown>) => unknown,
  ) =>
    selector({
      retryLastFailed: vi.fn(),
      editAndResendMessage: vi.fn(),
      branchConversation: vi.fn(),
      sending: false,
      sendingConversationId: null,
      aiStatus: null,
    }),
}));

vi.mock("@/components/action-log/ActionLog", () => ({
  ActionLog: () => null,
}));
vi.mock("@/components/chat/ChangeProposalCard", () => ({
  ChangeProposalCard: () => null,
}));
vi.mock("@/components/chat/InlineSurface", () => ({
  InlineSurfacesForMessage: () => null,
}));
vi.mock("@/components/chat/MessageAttachments", () => ({
  MessageAttachments: () => null,
}));
vi.mock("@/components/search/SearchResults", () => ({
  SearchResults: () => null,
}));

function assistantMessage(content: string): ChatMessage {
  return {
    id: "msg-1",
    conversationId: "conv-1",
    role: "assistant",
    content,
    status: "ok",
    createdAt: new Date().toISOString(),
    metadata: null,
  };
}

describe("isAllowedMarkdownHref", () => {
  it("allows HTTPS://x.com after protocol lowercasing", () => {
    expect(isAllowedMarkdownHref("HTTPS://x.com")).toBe(true);
  });

  it("rejects protocol-relative //x.com", () => {
    expect(isAllowedMarkdownHref("//x.com")).toBe(false);
  });

  it("rejects javascript:alert(1)", () => {
    expect(isAllowedMarkdownHref("javascript:alert(1)")).toBe(false);
  });
});

describe("MessageBubble markdown links", () => {
  beforeEach(() => {
    openExternalUrl.mockClear();
  });

  it("opens HTTPS://x.com via openExternalUrl on click", () => {
    render(
      <MessageBubble message={assistantMessage("[x](HTTPS://x.com)")} />,
    );
    const link = screen.getByRole("link", { name: "x" });
    fireEvent.click(link);
    expect(openExternalUrl).toHaveBeenCalledWith("HTTPS://x.com");
  });

  it("opens allowed links on middle-click (auxclick)", () => {
    render(
      <MessageBubble message={assistantMessage("[x](HTTPS://x.com)")} />,
    );
    const link = screen.getByRole("link", { name: "x" });
    fireEvent(
      link,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );
    expect(openExternalUrl).toHaveBeenCalledWith("HTTPS://x.com");
  });

  it("renders //x.com as inert text", () => {
    render(<MessageBubble message={assistantMessage("[x](//x.com)")} />);
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByText("x")).toBeTruthy();
    expect(openExternalUrl).not.toHaveBeenCalled();
  });

  it("renders javascript:alert(1) as inert text", () => {
    render(
      <MessageBubble
        message={assistantMessage("[x](javascript:alert(1))")}
      />,
    );
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.getByText("x")).toBeTruthy();
    expect(openExternalUrl).not.toHaveBeenCalled();
  });
});
