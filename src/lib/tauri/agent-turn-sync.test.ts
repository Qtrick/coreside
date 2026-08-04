import { describe, expect, it } from "vitest";
import {
  shouldApplyAgentTurnSync,
  shouldShowAppConflict,
} from "./events";

describe("shouldApplyAgentTurnSync", () => {
  const chatA = {
    activeConversationId: "conv-a",
    activeToolId: null as string | null,
  };
  const chatAToolX = {
    activeConversationId: "conv-a",
    activeToolId: "tool-x",
  };
  const toolWindowX = {
    activeConversationId: null as string | null,
    activeToolId: "tool-x",
  };

  it("skips Sync when conversationId differs from active", () => {
    expect(
      shouldApplyAgentTurnSync(
        {
          conversationId: "conv-b",
          surfaceIds: ["surf-tool-x"],
          toolIds: ["tool-x"],
        },
        chatAToolX,
      ),
    ).toBe(false);
  });

  it("applies Sync for matching conversation without surface targeting", () => {
    expect(
      shouldApplyAgentTurnSync(
        {
          conversationId: "conv-a",
          surfaceIds: [],
        },
        chatA,
      ),
    ).toBe(true);
  });

  it("applies Sync when surfaceIds overlap the active tool", () => {
    expect(
      shouldApplyAgentTurnSync(
        {
          conversationId: "conv-a",
          surfaceIds: ["surf-tool-x"],
          toolIds: ["tool-x"],
        },
        chatAToolX,
      ),
    ).toBe(true);
  });

  it("skips Sync when surface/tool targeting does not match active tool", () => {
    expect(
      shouldApplyAgentTurnSync(
        {
          conversationId: "conv-a",
          surfaceIds: ["surf-tool-y"],
          toolIds: ["tool-y"],
        },
        chatAToolX,
      ),
    ).toBe(false);
  });

  it("applies Sync in a tool window by toolId even without active conversation", () => {
    expect(
      shouldApplyAgentTurnSync(
        {
          conversationId: "conv-a",
          surfaceIds: ["surf-tool-x"],
          toolIds: ["tool-x"],
        },
        toolWindowX,
      ),
    ).toBe(true);
  });

  it("skips Sync in a tool window for a different tool", () => {
    expect(
      shouldApplyAgentTurnSync(
        {
          conversationId: "conv-a",
          surfaceIds: ["surf-tool-y"],
          toolIds: ["tool-y"],
        },
        toolWindowX,
      ),
    ).toBe(false);
  });

  it("refuses unscoped Sync with no conversation or surface target", () => {
    expect(
      shouldApplyAgentTurnSync({ surfaceIds: [] }, chatA),
    ).toBe(false);
  });
});

describe("shouldShowAppConflict", () => {
  it("shows conflict only for the matching active conversation", () => {
    expect(
      shouldShowAppConflict({ conversationId: "conv-a" }, "conv-a"),
    ).toBe(true);
    expect(
      shouldShowAppConflict({ conversationId: "conv-b" }, "conv-a"),
    ).toBe(false);
  });

  it("hides conflict when conversation ids are empty", () => {
    expect(shouldShowAppConflict({ conversationId: null }, "conv-a")).toBe(
      false,
    );
    expect(shouldShowAppConflict({ conversationId: "conv-a" }, null)).toBe(
      false,
    );
    expect(shouldShowAppConflict({ conversationId: "" }, "")).toBe(false);
  });
});
