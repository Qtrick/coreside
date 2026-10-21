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

  it("rejects AI-backed proposals", () => {
    expect(
      parseAutomationChange({
        name: "Review",
        requiresAi: true,
        trigger: { type: "daily", time: "09:00" },
        action: {
          type: "set_tool_state_value",
          toolId: "tool-tasks",
          path: "reset",
          value: true,
        },
      }),
    ).toBeNull();
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
});
