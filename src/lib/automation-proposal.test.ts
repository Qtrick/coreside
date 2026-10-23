import { describe, expect, it } from "vitest";
import {
  automationChangeStatusFromMetadata,
  parseAutomationChange,
  toUpsertAutomationInput,
} from "@/lib/automation-proposal";

describe("parseAutomationChange", () => {
  it("accepts a daily wallpaper set proposal", () => {
    const proposal = parseAutomationChange({
      name: "Morning wallpaper",
      trigger: { type: "daily", time: "08:00" },
      action: {
        type: "set_workspace_background",
        workspaceId: "workspace-default",
        presetId: "preset-1",
      },
      changeSummary: "Change wallpaper each morning",
    });
    expect(proposal?.name).toBe("Morning wallpaper");
    expect(proposal?.trigger).toEqual({
      type: "daily",
      time: "08:00",
      timezone: null,
    });
  });

  it("accepts AI summary proposals", () => {
    const proposal = parseAutomationChange({
      name: "Morning review",
      requiresAi: true,
      trigger: { type: "daily", time: "08:00", timezone: "America/New_York" },
      action: {
        type: "ai_prompt",
        prompt: "Summarize unfinished study tasks",
      },
      applicationId: "app-study",
    });
    expect(proposal?.requiresAi).toBe(true);
    expect(proposal?.action.type).toBe("ai_prompt");
    expect(proposal?.applicationId).toBe("app-study");
    expect(proposal?.scheduled).toBe(true);
    if (proposal?.action.type === "ai_prompt") {
      expect(proposal.action.prompt).toBe("Summarize unfinished study tasks");
      expect(proposal.action.conversationId).toBeNull();
    }
  });

  it("infers requiresAi from ai_prompt when the flag is omitted", () => {
    const proposal = parseAutomationChange({
      name: "Evening digest",
      trigger: { type: "daily", time: "20:00", timezone: "Asia/Tokyo" },
      action: {
        type: "ai_prompt",
        prompt: "Summarize today's unfinished work",
        conversation_id: "conv-study",
      },
    });
    expect(proposal?.requiresAi).toBe(true);
    expect(proposal?.action).toEqual({
      type: "ai_prompt",
      prompt: "Summarize today's unfinished work",
      conversationId: "conv-study",
    });
  });

  it("rejects empty or oversized ai_prompt text", () => {
    expect(
      parseAutomationChange({
        name: "Empty",
        requiresAi: true,
        trigger: { type: "daily", time: "08:00" },
        action: { type: "ai_prompt", prompt: "   " },
      }),
    ).toBeNull();
    expect(
      parseAutomationChange({
        name: "Too long",
        requiresAi: true,
        trigger: { type: "daily", time: "08:00" },
        action: { type: "ai_prompt", prompt: "x".repeat(4001) },
      }),
    ).toBeNull();
  });

  it("still accepts deterministic tool-state with requiresAi omitted", () => {
    expect(
      parseAutomationChange({
        name: "Reset counter",
        trigger: { type: "daily", time: "09:00" },
        action: {
          type: "set_tool_state_value",
          toolId: "tool-tasks",
          path: "reset",
          value: true,
        },
      }),
    ).not.toBeNull();
  });

  it("rejects unknown action types", () => {
    expect(
      parseAutomationChange({
        name: "Shell job",
        trigger: { type: "interval", intervalMinutes: 30 },
        action: { type: "run_shell", command: "rm -rf /" },
      }),
    ).toBeNull();
  });

  it("rejects intervals under 5 minutes", () => {
    expect(
      parseAutomationChange({
        name: "Too fast",
        trigger: { type: "interval", intervalMinutes: 2 },
        action: {
          type: "set_workspace_background",
          workspaceId: "workspace-default",
          presetId: "preset-1",
        },
      }),
    ).toBeNull();
  });

  it("rejects weekly weekday outside 0-6", () => {
    expect(
      parseAutomationChange({
        name: "Bad day",
        trigger: { type: "weekly", weekday: 9, time: "09:00" },
        action: {
          type: "set_workspace_background",
          workspaceId: "workspace-default",
          presetId: "preset-1",
        },
      }),
    ).toBeNull();
  });

  it("rejects single-preset wallpaper rotation", () => {
    expect(
      parseAutomationChange({
        name: "One wallpaper",
        trigger: { type: "interval", intervalMinutes: 60 },
        action: {
          type: "cycle_workspace_backgrounds",
          workspaceId: "workspace-default",
          presetIds: ["only-one"],
        },
      }),
    ).toBeNull();
  });

  it("rejects tool state path traversal", () => {
    expect(
      parseAutomationChange({
        name: "Traversal",
        trigger: { type: "daily", time: "09:00" },
        action: {
          type: "set_tool_state_value",
          toolId: "tool-tasks",
          path: "../core.secret",
          value: true,
        },
      }),
    ).toBeNull();
  });
});

describe("automationChangeStatusFromMetadata", () => {
  it("defaults pending when proposal present", () => {
    expect(
      automationChangeStatusFromMetadata({
        automationChange: { name: "x" },
      }),
    ).toBe("pending");
  });

  it("reads explicit status", () => {
    expect(
      automationChangeStatusFromMetadata({
        automationChangeStatus: "applied",
      }),
    ).toBe("applied");
  });
});

describe("toUpsertAutomationInput", () => {
  it("maps camelCase proposal to upsert shape", () => {
    const input = toUpsertAutomationInput({
      name: "Rotate",
      trigger: { type: "interval", intervalMinutes: 60, timezone: null },
      action: {
        type: "cycle_workspace_backgrounds",
        workspaceId: "workspace-default",
        presetIds: ["a", "b"],
      },
      requiresAi: false,
    });
    expect(input).toMatchObject({
      name: "Rotate",
      enabled: true,
      requiresAi: false,
      trigger: { type: "interval", intervalMinutes: 60 },
      action: {
        type: "cycle_workspace_backgrounds",
        workspaceId: "workspace-default",
        presetIds: ["a", "b"],
      },
    });
  });

  it("uses a stable id from the source message", () => {
    const input = toUpsertAutomationInput(
      {
        name: "Rotate",
        trigger: { type: "interval", intervalMinutes: 60, timezone: null },
        action: {
          type: "cycle_workspace_backgrounds",
          workspaceId: "workspace-default",
          presetIds: ["a", "b"],
        },
        requiresAi: false,
      },
      { sourceMessageId: "msg-123" },
    );
    expect(input.id).toBe("auto-chat-msg-123");
  });

  it("maps ai_prompt with the same stable auto-chat id on re-confirm", () => {
    const proposal = parseAutomationChange({
      name: "Morning review",
      requiresAi: true,
      trigger: { type: "daily", time: "08:00", timezone: "America/New_York" },
      action: { type: "ai_prompt", prompt: "Summarize unfinished study tasks" },
      applicationId: "app-study",
    });
    expect(proposal).not.toBeNull();
    const first = toUpsertAutomationInput(proposal!, { sourceMessageId: "msg-ai-1" });
    const second = toUpsertAutomationInput(proposal!, { sourceMessageId: "msg-ai-1" });
    expect(first.id).toBe("auto-chat-msg-ai-1");
    expect(second.id).toBe(first.id);
    expect(first.requiresAi).toBe(true);
    expect(first.action).toMatchObject({
      type: "ai_prompt",
      prompt: "Summarize unfinished study tasks",
    });
  });
});
