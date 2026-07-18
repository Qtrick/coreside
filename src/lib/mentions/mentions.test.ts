import { describe, expect, it } from "vitest";
import {
  filterToolsByMentionQuery,
  getActiveMentionQuery,
  isMentionTrigger,
  mergeResolvedMentions,
  resolveMentionsInText,
} from "@/lib/mentions";

describe("mentions", () => {
  it("does not treat email addresses as mention triggers", () => {
    const text = "email me at name@example.com please";
    const at = text.indexOf("@");
    expect(isMentionTrigger(text, at)).toBe(false);
    expect(getActiveMentionQuery(text, at + 4)).toBeNull();
  });

  it("does not trigger inside plus-tag emails", () => {
    const text = "reach user+tag@domain.co";
    const at = text.indexOf("@");
    expect(isMentionTrigger(text, at)).toBe(false);
  });

  it("triggers at start of message and after whitespace", () => {
    expect(isMentionTrigger("@Water", 0)).toBe(true);
    expect(isMentionTrigger("see @Water", 4)).toBe(true);
  });

  it("filters tools by name and description", () => {
    const tools = [
      { id: "1", name: "Water Tracker", description: "hydration" },
      { id: "2", name: "Budget Planner", description: "money" },
    ];
    expect(filterToolsByMentionQuery(tools, "wat").map((t) => t.id)).toEqual([
      "1",
    ]);
    expect(filterToolsByMentionQuery(tools, "money").map((t) => t.id)).toEqual([
      "2",
    ]);
  });

  it("resolves multiple mentions by stable tool id", () => {
    const tools = [
      { id: "a", name: "Alpha" },
      { id: "b", name: "Beta Tool" },
    ];
    const mentions = resolveMentionsInText(
      "Compare @Alpha with @Beta Tool today",
      tools,
    );
    expect(mentions.map((m) => m.toolId)).toEqual(["a", "b"]);
  });

  it("merges pending tool ids onto resolved spans", () => {
    const resolved = resolveMentionsInText("Use @Alpha", [
      { id: "wrong", name: "Alpha" },
    ]);
    const pending = [
      {
        mentionId: "m-real-0",
        toolId: "real",
        label: "Alpha",
        start: 4,
        end: 10,
      },
    ];
    const merged = mergeResolvedMentions(resolved, pending);
    expect(merged.map((m) => m.toolId)).toEqual(["real"]);
    expect(merged[0]?.mentionId).toBe("m-real-4");
  });

  it("keeps distinct tool ids when the same label appears twice", () => {
    const tools = [
      { id: "a1", name: "Alpha" },
      { id: "a2", name: "Alpha" },
    ];
    // Text resolution picks the first matching name; pending overlays by position.
    const resolved = resolveMentionsInText("See @Alpha and @Alpha", tools);
    const pending = [
      {
        mentionId: "m-a1-4",
        toolId: "a1",
        label: "Alpha",
        start: 4,
        end: 10,
      },
      {
        mentionId: "m-a2-15",
        toolId: "a2",
        label: "Alpha",
        start: 15,
        end: 21,
      },
    ];
    expect(mergeResolvedMentions(resolved, pending).map((m) => m.toolId)).toEqual([
      "a1",
      "a2",
    ]);
  });
});
