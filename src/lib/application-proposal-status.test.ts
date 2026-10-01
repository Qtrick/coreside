import { describe, expect, it } from "vitest";
import {
  computeLiveApplicationGenerationIndex,
  deriveEvolutionCopy,
  proposalStatusPresentation,
  sanitizeConsumerError,
} from "@/lib/application-proposal-status";

describe("sanitizeConsumerError", () => {
  it("replaces sql and stack-like messages", () => {
    expect(sanitizeConsumerError("SELECT * FROM surfaces")).toMatch(/could not be completed/i);
    expect(sanitizeConsumerError("error at src/foo.rs:12")).toMatch(/could not be completed/i);
  });

  it("passes through short consumer messages", () => {
    expect(sanitizeConsumerError("Proposal has expired")).toBe("Proposal has expired");
  });
});

describe("proposalStatusPresentation", () => {
  it("never maps failure to ready", () => {
    expect(proposalStatusPresentation("failed").label).toBe("Failed");
    expect(proposalStatusPresentation("applied").label).toBe("Ready");
  });

  it("maps pending to review", () => {
    expect(proposalStatusPresentation("pending").label).toBe("Review changes");
  });
});

describe("computeLiveApplicationGenerationIndex", () => {
  it("advances toward preview and building", () => {
    expect(computeLiveApplicationGenerationIndex([], false)).toBe(0);
    expect(computeLiveApplicationGenerationIndex(["Loading context"], true)).toBe(1);
    expect(
      computeLiveApplicationGenerationIndex(["Updating surface layout"], true),
    ).toBe(2);
    expect(
      computeLiveApplicationGenerationIndex(["Applying patch"], true),
    ).toBe(3);
  });
});

describe("deriveEvolutionCopy", () => {
  it("detects evolution vs create", () => {
    const evolution = deriveEvolutionCopy(
      [{ type: "component.update_props" }],
      "Adds due dates to tasks.",
      "Evolve tracker",
    );
    expect(evolution.isEvolution).toBe(true);
    expect(evolution.headline).toBe("Proposed changes");
    expect(evolution.whatWillChange).toContain("due dates");
  });
});
