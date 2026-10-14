import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  isAllowedMarkdownHref,
  normalizeMarkdownHref,
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

describe("normalizeMarkdownHref", () => {
  it("normalizes HTTPS://x.com to a resolved https href", () => {
    expect(normalizeMarkdownHref("HTTPS://x.com")).toBe("https://x.com/");
    expect(isAllowedMarkdownHref("HTTPS://x.com")).toBe(true);
  });

  it("rejects protocol-relative //x.com", () => {
    expect(normalizeMarkdownHref("//x.com")).toBeNull();
    expect(isAllowedMarkdownHref("//x.com")).toBe(false);
  });

  it("rejects javascript:alert(1)", () => {
    expect(normalizeMarkdownHref("javascript:alert(1)")).toBeNull();
    expect(isAllowedMarkdownHref("javascript:alert(1)")).toBe(false);
  });
});

describe("MessageBubble markdown links", () => {
  beforeEach(() => {
    openExternalUrl.mockClear();
  });

  it("opens HTTPS://x.com via openExternalUrl on click with normalized href", () => {
    render(
      <MessageBubble message={assistantMessage("[x](HTTPS://x.com)")} />,
    );
    const link = screen.getByRole("link", { name: "x" });
    fireEvent.click(link);
    expect(openExternalUrl).toHaveBeenCalledWith("https://x.com/");
  });

  it("opens allowed links on middle-click (auxclick button 1)", () => {
    render(
      <MessageBubble message={assistantMessage("[x](HTTPS://x.com)")} />,
    );
    const link = screen.getByRole("link", { name: "x" });
    fireEvent(
      link,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );
    expect(openExternalUrl).toHaveBeenCalledWith("https://x.com/");
  });

  it("does not open links on right-click auxclick (button 2)", () => {
    render(
      <MessageBubble message={assistantMessage("[x](HTTPS://x.com)")} />,
    );
    const link = screen.getByRole("link", { name: "x" });
    fireEvent(
      link,
      new MouseEvent("auxclick", { bubbles: true, button: 2 }),
    );
    expect(openExternalUrl).not.toHaveBeenCalled();
  });

  it("renders remark-gfm footnote #user-content-fn-1 as <a>", () => {
    render(
      <MessageBubble
        message={assistantMessage(
          "See note[^1]\n\n[^1]: footnote body\n",
        )}
      />,
    );
    const footnoteLink = document.querySelector(
      'a[href="#user-content-fn-1"]',
    ) as HTMLAnchorElement | null;
    expect(footnoteLink).toBeTruthy();
    expect(footnoteLink!.tagName).toBe("A");
    fireEvent.click(footnoteLink!);
    expect(openExternalUrl).not.toHaveBeenCalled();
  });

  it("renders an explicit #user-content-fn-1 link as <a>", () => {
    render(
      <MessageBubble
        message={assistantMessage("[fn](#user-content-fn-1)")}
      />,
    );
    const link = screen.getByRole("link", { name: "fn" });
    expect(link.getAttribute("href")).toBe("#user-content-fn-1");
    fireEvent.click(link);
    expect(openExternalUrl).not.toHaveBeenCalled();
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
