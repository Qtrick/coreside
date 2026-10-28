import { describe, expect, it } from "vitest";
import {
  getActiveOnboardingStages,
  isStageActive,
  ONBOARDING_STAGES,
} from "./stages";

describe("Onboarding stage registry and dormant gates", () => {
  it("keeps dormant future stages disabled and active stages enabled", () => {
    expect(isStageActive("welcome")).toBe(true);
    expect(isStageActive("appearance")).toBe(true);
    expect(isStageActive("account_sign_in")).toBe(false);
    expect(isStageActive("offline_choice")).toBe(false);
    expect(isStageActive("account_sync")).toBe(false);
  });

  it("getActiveOnboardingStages returns only active stages in correct order", () => {
    const active = getActiveOnboardingStages();
    expect(active.map((s) => s.id)).toEqual(["welcome", "appearance"]);
    expect(active.every((s) => s.enabled)).toBe(true);
  });

  it("ensures no future dormant stage requires network in consumer UI", () => {
    const dormant = ONBOARDING_STAGES.filter((s) => !s.enabled);
    expect(dormant.length).toBeGreaterThanOrEqual(3);
    // Unimplemented stages must never be marked required
    expect(dormant.every((s) => !s.required)).toBe(true);
  });
});

