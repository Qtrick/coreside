import { describe, expect, it } from "vitest";
import {
  isCurrentChat,
  messagesBelongToChat,
  shouldLoadChatMessages,
  shouldNavigateToChatNoOp,
} from "./chat";
import type { AppView } from "./types";

describe("isCurrentChat", () => {
  it("returns true when view is chat and active id matches", () => {
    const view: AppView = { kind: "chat", conversationId: "c-1" };
    expect(isCurrentChat(view, "c-1", "c-1")).toBe(true);
  });

  it("returns false when view is chat but id differs", () => {
    const view: AppView = { kind: "chat", conversationId: "c-1" };
    expect(isCurrentChat(view, "c-1", "c-2")).toBe(false);
  });

  it("returns false when active id matches but view conversationId is null", () => {
    const view: AppView = { kind: "chat", conversationId: null };
    expect(isCurrentChat(view, "c-1", "c-1")).toBe(false);
  });

  it("returns false when view conversationId matches but active id differs", () => {
    const view: AppView = { kind: "chat", conversationId: "c-1" };
    expect(isCurrentChat(view, "c-2", "c-1")).toBe(false);
  });

  it("returns false when active id matches but view is not chat", () => {
    const cases: AppView[] = [
      { kind: "settings" },
      { kind: "automations" },
      { kind: "projects" },
      { kind: "media" },
      { kind: "project", projectId: "p-1" },
    ];
    for (const view of cases) {
      expect(isCurrentChat(view, "c-1", "c-1")).toBe(false);
    }
  });

  it("no-op predicate mirrors isCurrentChat", () => {
    const view: AppView = { kind: "chat", conversationId: "c-1" };
    expect(shouldNavigateToChatNoOp(view, "c-1", "c-1")).toBe(
      isCurrentChat(view, "c-1", "c-1"),
    );
  });
});

describe("shouldLoadChatMessages", () => {
  it("loads when switching to a different chat", () => {
    expect(
      shouldLoadChatMessages("c-1", "c-2", [
        { conversationId: "c-1" },
      ]),
    ).toBe(true);
  });

  it("skips load when same chat and messages belong to it", () => {
    expect(
      shouldLoadChatMessages("c-1", "c-1", [
        { conversationId: "c-1" },
        { conversationId: "c-1" },
      ]),
    ).toBe(false);
  });

  it("loads when same id but messages are from another chat", () => {
    expect(
      shouldLoadChatMessages("c-1", "c-1", [
        { conversationId: "c-2" },
      ]),
    ).toBe(true);
  });
});

describe("messagesBelongToChat", () => {
  it("treats empty messages as belonging to the chat", () => {
    expect(messagesBelongToChat([], "c-1")).toBe(true);
  });
});
