import { describe, expect, it } from "vitest";
import {
  canOfferContextualTip,
  canShowWhatsNewBanner,
  isCoreTourBlocking,
} from "./contextual";
import type { TutorialProgress } from "./types";
import {
  CONTEXTUAL_TIP_IDS,
  ESSENTIALS_TUTORIAL_ID,
  WHATS_NEW_ID,
} from "./tutorials";

function row(
  tutorialId: string,
  status: TutorialProgress["status"],
): TutorialProgress {
  return {
    tutorialId,
    tutorialVersion: 1,
    status,
    currentStepId: null,
    completedStepIds: [],
    startedAt: null,
    updatedAt: "2026-01-01T00:00:00Z",
    completedAt: null,
    skippedAt: null,
    lastOpenedAt: null,
  };
}

describe("contextual tip eligibility", () => {
  const tipId = CONTEXTUAL_TIP_IDS.firstApp;

  it("suppresses when core tour is active", () => {
    expect(isCoreTourBlocking("tour")).toBe(true);
    expect(isCoreTourBlocking("welcome")).toBe(true);
    expect(
      canOfferContextualTip({
        tipId,
        phase: "tour",
        onboardingDisabled: false,
        activeContextualId: null,
        progressById: {},
      }),
    ).toBe(false);
  });

  it("suppresses when already completed", () => {
    expect(
      canOfferContextualTip({
        tipId,
        phase: "idle",
        onboardingDisabled: false,
        activeContextualId: null,
        progressById: {
          [tipId]: row(tipId, "completed"),
        },
      }),
    ).toBe(false);
  });

  it("suppresses when onboarding disabled or another tip active", () => {
    expect(
      canOfferContextualTip({
        tipId,
        phase: "idle",
        onboardingDisabled: true,
        activeContextualId: null,
        progressById: {},
      }),
    ).toBe(false);
    expect(
      canOfferContextualTip({
        tipId,
        phase: "idle",
        onboardingDisabled: false,
        activeContextualId: CONTEXTUAL_TIP_IDS.firstQueue,
        progressById: {},
      }),
    ).toBe(false);
  });

  it("allows when idle and not completed", () => {
    expect(
      canOfferContextualTip({
        tipId,
        phase: "idle",
        onboardingDisabled: false,
        activeContextualId: null,
        progressById: {},
      }),
    ).toBe(true);
  });
});

describe("whats-new banner eligibility", () => {
  it("requires activity, finished essentials, and undismissed whats-new", () => {
    expect(
      canShowWhatsNewBanner({
        phase: "idle",
        onboardingDisabled: false,
        hasMeaningfulActivity: true,
        progressById: {
          [ESSENTIALS_TUTORIAL_ID]: row(ESSENTIALS_TUTORIAL_ID, "completed"),
        },
        essentialsId: ESSENTIALS_TUTORIAL_ID,
        whatsNewId: WHATS_NEW_ID,
      }),
    ).toBe(true);

    expect(
      canShowWhatsNewBanner({
        phase: "tour",
        onboardingDisabled: false,
        hasMeaningfulActivity: true,
        progressById: {
          [ESSENTIALS_TUTORIAL_ID]: row(ESSENTIALS_TUTORIAL_ID, "skipped"),
        },
        essentialsId: ESSENTIALS_TUTORIAL_ID,
        whatsNewId: WHATS_NEW_ID,
      }),
    ).toBe(false);

    expect(
      canShowWhatsNewBanner({
        phase: "idle",
        onboardingDisabled: false,
        hasMeaningfulActivity: true,
        progressById: {
          [ESSENTIALS_TUTORIAL_ID]: row(ESSENTIALS_TUTORIAL_ID, "completed"),
          [WHATS_NEW_ID]: row(WHATS_NEW_ID, "skipped"),
        },
        essentialsId: ESSENTIALS_TUTORIAL_ID,
        whatsNewId: WHATS_NEW_ID,
      }),
    ).toBe(false);
  });
});
