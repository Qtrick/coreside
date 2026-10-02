import { describe, expect, it } from "vitest";
import { toToolDefinition, type ToolRecord } from "./tools";

describe("toToolDefinition", () => {
  it("prefers tools row id over copied definition id (branch forks)", () => {
    const record: ToolRecord = {
      id: "app-branch-abc",
      name: "Task Tracker (branch)",
      description: "forked",
      currentVersion: 1,
      definition: {
        id: "tool-task-tracker",
        name: "Task Tracker",
        description: "source def",
        layout: { type: "single-column" },
        components: [],
      },
    };
    const tool = toToolDefinition(record);
    expect(tool.id).toBe("app-branch-abc");
    expect(tool.name).toBe("Task Tracker (branch)");
    expect(tool.version).toBe(1);
  });
});
