import { describe, expect, it } from "vitest";
import {
  isQueueEventForConversation,
  type QueueChangedEvent,
} from "./events";

describe("isQueueEventForConversation", () => {
  const base: QueueChangedEvent = {
    kind: "itemAdded",
    conversationId: "conv-a",
    itemId: "q-1",
  };

  it("accepts events for the active conversation", () => {
    expect(isQueueEventForConversation(base, "conv-a")).toBe(true);
  });

  it("filters events for a different conversation", () => {
    expect(isQueueEventForConversation(base, "conv-b")).toBe(false);
  });

  it("filters when conversationId is empty on the event", () => {
    expect(
      isQueueEventForConversation(
        { ...base, conversationId: "" },
        "conv-a",
      ),
    ).toBe(false);
  });

  it("filters when the active conversationId is empty", () => {
    expect(isQueueEventForConversation(base, "")).toBe(false);
  });
});
