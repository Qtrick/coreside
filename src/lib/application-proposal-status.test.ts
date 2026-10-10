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
    expect(proposalStatusPresentation("failed").label).toBe("Needs attention");
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

  it("maps verification activity to Testing (index 4)", () => {
    expect(
      computeLiveApplicationGenerationIndex(["Running declarative tests"], true),
    ).toBe(4);
    expect(computeLiveApplicationGenerationIndex(["Verifying surface"], false)).toBe(
      4,
    );
  });

  it("maps finished generation to Ready (index 5)", () => {
    expect(
      computeLiveApplicationGenerationIndex(["Generation complete"], false),
    ).toBe(5);
    expect(computeLiveApplicationGenerationIndex(["Test pass"], false)).toBe(5);
  });

  it("does not treat Generating as Applying while streaming", () => {
    expect(
      computeLiveApplicationGenerationIndex(["Generating application"], true),
    ).toBe(1);
  });

  it("advances to Review when a pending proposal exists", () => {
    expect(
      computeLiveApplicationGenerationIndex(["Writing reply"], false, {
        hasPendingProposal: true,
      }),
    ).toBe(2);
  });

  it("prefers applying/testing/ready hints over action heuristics", () => {
    expect(
      computeLiveApplicationGenerationIndex(["Writing reply"], false, {
        hasPendingProposal: true,
        applying: true,
      }),
    ).toBe(3);
    expect(
      computeLiveApplicationGenerationIndex([], false, { testing: true }),
    ).toBe(4);
    expect(
      computeLiveApplicationGenerationIndex([], false, { ready: true }),
    ).toBe(5);
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

  it("recognizes granular component ops and default preservation copy", () => {
    const evolution = deriveEvolutionCopy(
      [
        { opType: "component.insert" },
        { type: "component.update_actions" },
        { type: "state.patch" },
      ],
      "",
      "Extend tasks panel",
    );
    expect(evolution.isEvolution).toBe(true);
    expect(evolution.headline).toBe("Proposed changes");
    expect(evolution.whatWillBePreserved).toContain("Existing records");
  });

  it("uses explicit preservation summary when provided", () => {
    const evolution = deriveEvolutionCopy(
      [{ type: "component.update_props" }],
      "Adds priority column.",
      "Evolve planner",
      "Dashboard cards and saved study tasks stay intact.",
    );
    expect(evolution.whatWillBePreserved).toBe(
      "Dashboard cards and saved study tasks stay intact.",
    );
  });

  it("treats full replace as create preview headline", () => {
    const created = deriveEvolutionCopy(
      [{ type: "tool.full_replace" }],
      "Rebuild layout.",
      "Redesign",
    );
    expect(created.isEvolution).toBe(false);
    expect(created.headline).toBe("New application preview");
  });
});
