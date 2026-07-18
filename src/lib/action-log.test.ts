import { describe, expect, it } from "vitest";
import {
  parseActionLogMode,
  shouldShowActionLog,
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
