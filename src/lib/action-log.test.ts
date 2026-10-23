import { describe, expect, it } from "vitest";
import {
  actionTimelineSummary,
  formatActionDuration,
  normalizeActionStatus,
  parseActionLogMode,
  resolveActionEventsAfterProposal,
  shouldShowActionLog,
  totalActionDurationMs,
  upsertAgentAction,
} from "@/lib/action-log";

describe("parseActionLogMode", () => {
  it("maps aliases", () => {
    expect(parseActionLogMode("off")).toBe("off");
    expect(parseActionLogMode("always")).toBe("always");
    expect(parseActionLogMode("intelligent")).toBe("intelligent");
    expect(parseActionLogMode("auto")).toBe("intelligent");
    expect(parseActionLogMode(true)).toBe("always");
  });
});

describe("normalizeActionStatus", () => {
  it("accepts lifecycle statuses and aliases", () => {
    expect(normalizeActionStatus("running")).toBe("running");
    expect(normalizeActionStatus("succeeded")).toBe("completed");
    expect(normalizeActionStatus("awaiting_approval")).toBe("awaitingApproval");
    expect(normalizeActionStatus("bogus")).toBe("completed");
  });
});

describe("shouldShowActionLog", () => {
  it("hides when off", () => {
    expect(
      shouldShowActionLog("off", [{ label: "Searching the web", eventType: "tool_started" }]),
    ).toBe(false);
  });

  it("always shows any events", () => {
    expect(
      shouldShowActionLog("always", [{ label: "Preparing your request", eventType: "request_started" }]),
    ).toBe(true);
  });

  it("intelligent shows only substantive work", () => {
    expect(
      shouldShowActionLog("intelligent", [
        { label: "Preparing your request", eventType: "request_started" },
        { label: "Building agent context", eventType: "context_loaded" },
      ]),
    ).toBe(false);
    expect(
      shouldShowActionLog("intelligent", [
        { label: "Preparing your request", eventType: "request_started" },
        { label: "Searching the web", eventType: "tool_started" },
      ]),
    ).toBe(true);
    expect(
      shouldShowActionLog("intelligent", [
        { label: "Preparing tool change preview", eventType: "tool_change_proposed" },
      ]),
    ).toBe(true);
  });
});

describe("upsertAgentAction", () => {
  it("updates the same stepId instead of appending", () => {
    const started = upsertAgentAction([], {
      id: "act-0",
      label: "Searching the web",
      status: "running",
      eventType: "tool_started",
      stepId: "tool-1-0",
    });
    const done = upsertAgentAction(started, {
      id: "act-0",
      label: "Searching the web",
      status: "completed",
      eventType: "tool_completed",
      stepId: "tool-1-0",
      durationMs: 420,
    });
    expect(done).toHaveLength(1);
    expect(done[0]?.status).toBe("completed");
    expect(done[0]?.eventType).toBe("tool_completed");
    expect(done[0]?.durationMs).toBe(420);
  });
});

describe("actionTimelineSummary", () => {
  it("does not claim success while a step is running", () => {
    expect(
      actionTimelineSummary([
        {
          id: "1",
          label: "Searching the web",
          status: "running",
          eventType: "tool_started",
        },
      ]).tone,
    ).toBe("active");
  });

  it("summarizes applied changes as Built it", () => {
    expect(
      actionTimelineSummary([
        {
          id: "1",
          label: "Searching the web",
          status: "completed",
          eventType: "tool_completed",
        },
        {
          id: "2",
          label: "Applied changes",
          status: "completed",
          eventType: "change_applied",
        },
      ]).title,
    ).toBe("Built it");
  });
});

describe("resolveActionEventsAfterProposal", () => {
  it("maps awaitingApproval to completed after apply", () => {
    const resolved = resolveActionEventsAfterProposal(
      [
        {
          id: "1",
          label: "Proposed a change",
          status: "awaitingApproval",
          eventType: "change_proposed",
        },
      ],
      "applied",
    );
    expect(resolved[0]?.status).toBe("completed");
    expect(resolved[0]?.eventType).toBe("change_applied");
  });

  it("resolves automation proposals independently of kernel status", () => {
    const resolved = resolveActionEventsAfterProposal(
      [
        {
          id: "1",
          label: "Proposing an automation",
          status: "awaitingApproval",
          eventType: "automation_proposed",
        },
        {
          id: "2",
          label: "Proposed a change",
          status: "awaitingApproval",
          eventType: "change_proposed",
        },
      ],
      null,
      "discarded",
    );
    expect(resolved[0]?.status).toBe("cancelled");
    expect(resolved[1]?.status).toBe("awaitingApproval");
  });

  it("does not resolve automation rows when only the kernel proposal is applied", () => {
    const resolved = resolveActionEventsAfterProposal(
      [
        {
          id: "1",
          label: "Proposing an automation",
          status: "awaitingApproval",
          eventType: "automation_proposed",
        },
        {
          id: "2",
          label: "Proposed a change",
          status: "awaitingApproval",
          eventType: "change_proposed",
        },
      ],
      "applied",
      null,
    );
    expect(resolved[0]?.status).toBe("awaitingApproval");
    expect(resolved[0]?.eventType).toBe("automation_proposed");
    expect(resolved[1]?.status).toBe("completed");
    expect(resolved[1]?.eventType).toBe("change_applied");
  });

  it("marks applied automation as scheduled without touching kernel awaiting rows", () => {
    const resolved = resolveActionEventsAfterProposal(
      [
        {
          id: "1",
          label: "Proposing an automation",
          status: "awaitingApproval",
          eventType: "automation_proposed",
        },
        {
          id: "2",
          label: "Proposed a change",
          status: "awaitingApproval",
          eventType: "tool_change_proposed",
        },
      ],
      null,
      "applied",
    );
    expect(resolved[0]?.status).toBe("completed");
    expect(resolved[0]?.eventType).toBe("automation_scheduled");
    expect(resolved[1]?.status).toBe("awaitingApproval");
    expect(resolved[1]?.eventType).toBe("tool_change_proposed");
  });

  it("does not claim Built it for an automation proposal alone", () => {
    expect(
      actionTimelineSummary([
        {
          id: "1",
          label: "Proposing an automation",
          status: "completed",
          eventType: "automation_proposed",
        },
      ]).title,
    ).toBe("Automation scheduled");
  });

  it("summarizes awaiting automation proposal without Built it", () => {
    expect(
      actionTimelineSummary([
        {
          id: "1",
          label: "Proposing an automation",
          status: "awaitingApproval",
          eventType: "automation_proposed",
        },
      ]).title,
    ).toBe("Automation proposed");
  });

  it("titles automation_scheduled as Automation scheduled", () => {
    expect(
      actionTimelineSummary([
        {
          id: "1",
          label: "Automation scheduled",
          status: "completed",
          eventType: "automation_scheduled",
        },
      ]).title,
    ).toBe("Automation scheduled");
  });
});

describe("duration helpers", () => {
  it("formats compact durations", () => {
    expect(formatActionDuration(420)).toBe("420ms");
    expect(formatActionDuration(1500)).toBe("1.5s");
    expect(formatActionDuration(65_000)).toBe("1m 5s");
  });

  it("sums step durations", () => {
    expect(
      totalActionDurationMs([
        { durationMs: 100 },
        { startedAt: 0, completedAt: 250 },
      ]),
    ).toBe(350);
  });
});
