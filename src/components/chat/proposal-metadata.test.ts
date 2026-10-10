import { describe, expect, it } from "vitest";
import { kernelProposalFromMetadata } from "./proposal-metadata";

describe("kernelProposalFromMetadata", () => {
  it("returns null without proposalId or operations", () => {
    expect(kernelProposalFromMetadata(null)).toBeNull();
    expect(kernelProposalFromMetadata({})).toBeNull();
    expect(
      kernelProposalFromMetadata({
        runtimeV2: { proposalId: "p1", operations: [] },
      }),
    ).toBeNull();
  });

  it("extracts pending proposal fields for the apply card", () => {
    const result = kernelProposalFromMetadata({
      runtimeV2: {
        proposalId: "prop-1",
        summary: "Create Task Tracker",
        impactSummary: "Adds a local tasks model",
        risk: "strong",
        operations: [{ type: "surface.create" }],
        preservationSummary: "Existing chats stay",
      },
    });
    expect(result).toEqual({
      proposalId: "prop-1",
      summary: "Create Task Tracker",
      impactSummary: "Adds a local tasks model",
      risk: "strong",
      operations: [{ type: "surface.create" }],
      status: "pending",
      preservationSummary: "Existing chats stay",
    });
  });

  it("falls back to kernelProposalStatus and preservedSummary aliases", () => {
    const result = kernelProposalFromMetadata({
      kernelProposalStatus: "applied",
      runtimeV2: {
        proposalId: "prop-2",
        operations: [{ type: "state.patch" }],
        preservedSummary: "State kept",
      },
    });
    expect(result?.status).toBe("applied");
    expect(result?.preservationSummary).toBe("State kept");
    expect(result?.summary).toBe("Proposed application change");
  });
});
