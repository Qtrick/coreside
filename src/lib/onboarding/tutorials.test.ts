import { describe, expect, it } from "vitest";
import { TUTORIALS, TOUR_TARGETS, CONTEXTUAL_TIPS, WHATS_NEW_ID } from "./tutorials";

describe("tutorial registry", () => {
  it("keeps unique tutorial and step ids with versions", () => {
    const tutorialIds = new Set<string>();
    for (const tutorial of TUTORIALS) {
      expect(tutorial.version).toBeGreaterThanOrEqual(1);
      expect(tutorial.steps.length).toBeGreaterThan(0);
      expect(tutorialIds.has(tutorial.id)).toBe(false);
      tutorialIds.add(tutorial.id);

      const stepIds = new Set<string>();
      for (const step of tutorial.steps) {
        expect(stepIds.has(step.id)).toBe(false);
        stepIds.add(step.id);
        if (step.target) {
          expect(Object.values(TOUR_TARGETS)).toContain(step.target);
        }
      }
    }
  });

  it("marks essentials as first-run with declared targets", () => {
    const essentials = TUTORIALS.find((t) => t.id === "coreside-essentials");
    expect(essentials?.firstRun).toBe(true);
    expect(essentials?.version).toBe(1);
    const targets = essentials?.steps.map((s) => s.target).filter(Boolean);
    expect(targets?.length).toBeGreaterThanOrEqual(3);
  });

  it("keeps unique contextual tip ids", () => {
    const ids = new Set<string>();
    for (const tip of CONTEXTUAL_TIPS) {
      expect(tip.version).toBeGreaterThanOrEqual(1);
      expect(ids.has(tip.id)).toBe(false);
      ids.add(tip.id);
      expect(tip.id.startsWith("contextual-")).toBe(true);
    }
    expect(WHATS_NEW_ID.startsWith("whats-new:")).toBe(true);
  });

  it("keeps at least one developerOnly advanced module", () => {
    expect(TUTORIALS.some((t) => t.developerOnly)).toBe(true);
    expect(TUTORIALS.filter((t) => !t.firstRun).length).toBeGreaterThanOrEqual(3);
  });
});
