import { describe, expect, it } from "vitest";
import { deriveAgentPresence } from "@/lib/agent-activity";
import type { AgentActionEntry } from "@/lib/action-log";

function action(
  partial: Partial<AgentActionEntry> & Pick<AgentActionEntry, "label">,
): AgentActionEntry {
  return {
    id: partial.id ?? "a1",
    label: partial.label,
    status: partial.status ?? "running",
    eventType: partial.eventType ?? "tool_started",
    stepId: partial.stepId,
  };
}

describe("deriveAgentPresence", () => {
  it("stays idle when nothing is happening", () => {
    expect(
      deriveAgentPresence({
        actions: [],
        streaming: false,
        sending: false,
      }),
    ).toEqual({
      phase: "idle",
      tone: "neutral",
      label: "",
      busy: false,
    });
  });

  it("shows busy while sending even without actions", () => {
    const p = deriveAgentPresence({
      actions: [],
      streaming: false,
      sending: true,
    });
    expect(p.phase).toBe("busy");
    expect(p.busy).toBe(true);
    expect(p.label).toBeTruthy();
  });

  it("prefer awaiting approval over busy", () => {
    const p = deriveAgentPresence({
      actions: [
        action({ label: "Review proposed changes", status: "awaitingApproval" }),
      ],
      streaming: false,
      sending: true,
    });
    expect(p.phase).toBe("awaiting");
    expect(p.busy).toBe(false);
  });

  it("surfaces send errors", () => {
    const p = deriveAgentPresence({
      actions: [],
      streaming: false,
      sending: false,
      error: "Provider timeout",
    });
    expect(p.phase).toBe("failed");
    expect(p.tone).toBe("error");
  });

  it("marks done when all actions completed and turn finished", () => {
    const p = deriveAgentPresence({
      actions: [
        action({
          label: "Searching the web",
          status: "completed",
          eventType: "tool_completed",
        }),
      ],
      streaming: false,
      sending: false,
    });
    expect(p.phase).toBe("done");
    expect(p.tone).toBe("success");
  });
});
