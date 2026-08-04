import { create } from "zustand";
import { api } from "@/lib/tauri";
import { canOfferContextualTip } from "@/lib/onboarding/contextual";
import {
  activeStep,
  activeTutorial,
  applyPersistedProgress,
  backStep,
  completeTour,
  emptyCoordinatorState,
  nextStep,
  showWelcome,
  skipTour,
  startTour,
  type CoordinatorResult,
} from "@/lib/onboarding/coordinator";
import type { CoordinatorState, OnboardingState } from "@/lib/onboarding/types";
import {
  ESSENTIALS_TUTORIAL_ID,
  getContextualTip,
  WHATS_NEW_ID,
} from "@/lib/onboarding/tutorials";

type OnboardingStore = CoordinatorState & {
  bootstrapped: boolean;
  welcomeEligible: boolean;
  onboardingDisabled: boolean;
  hasMeaningfulActivity: boolean;
  activeContextualId: string | null;
  hydrate: () => Promise<void>;
  openWelcomeIfEligible: () => Promise<void>;
  startEssentials: (resume?: boolean) => Promise<void>;
  startModule: (tutorialId: string, resume?: boolean) => Promise<void>;
  next: () => Promise<void>;
  back: () => Promise<void>;
  skip: () => Promise<void>;
  finish: () => Promise<void>;
  dismissWelcome: () => Promise<void>;
  resetEssentials: () => Promise<void>;
  /** Offer a contextual tip when eligible; no-ops when blocked. */
  offerContextual: (tipId: string) => boolean;
  dismissContextual: () => Promise<void>;
  dismissWhatsNew: () => Promise<void>;
};

async function persist(result: CoordinatorResult): Promise<CoordinatorState> {
  let state = result.state;
  if (result.persist) {
    const row = await api.upsertTutorialProgress({
      tutorialId: result.persist.tutorialId,
      tutorialVersion: result.persist.tutorialVersion,
      status: result.persist.status,
      currentStepId: result.persist.currentStepId,
      completedStepIds: result.persist.completedStepIds,
    });
    state = applyPersistedProgress(state, row);
  }
  return state;
}

async function markModuleDone(
  tutorialId: string,
  version: number,
  status: "completed" | "skipped",
): Promise<CoordinatorState> {
  const row = await api.upsertTutorialProgress({
    tutorialId,
    tutorialVersion: version,
    status,
    currentStepId: null,
    completedStepIds: [],
  });
  return applyPersistedProgress(useOnboardingStore.getState(), row);
}

export const useOnboardingStore = create<OnboardingStore>((set, get) => ({
  ...emptyCoordinatorState(),
  bootstrapped: false,
  welcomeEligible: false,
  onboardingDisabled: false,
  hasMeaningfulActivity: false,
  activeContextualId: null,

  hydrate: async () => {
    try {
      const remote: OnboardingState = await api.getOnboardingState();
      set({
        ...emptyCoordinatorState(remote.progress),
        bootstrapped: true,
        welcomeEligible: remote.welcomeEligible,
        onboardingDisabled: remote.onboardingDisabled,
        hasMeaningfulActivity: remote.hasMeaningfulActivity,
        activeContextualId: null,
      });
    } catch {
      set({
        bootstrapped: true,
        welcomeEligible: false,
        onboardingDisabled: false,
        hasMeaningfulActivity: false,
        activeContextualId: null,
      });
    }
  },

  openWelcomeIfEligible: async () => {
    if (!get().bootstrapped) await get().hydrate();
    if (get().welcomeEligible && get().phase === "idle") {
      set(showWelcome(get()));
    }
  },

  startEssentials: async (resume = true) => {
    const state = await persist(startTour(get(), ESSENTIALS_TUTORIAL_ID, resume));
    set({ ...state, welcomeEligible: false, activeContextualId: null });
  },

  startModule: async (tutorialId, resume = true) => {
    const state = await persist(startTour(get(), tutorialId, resume));
    set({ ...state, welcomeEligible: false, activeContextualId: null });
  },

  next: async () => {
    set(await persist(nextStep(get())));
  },

  back: async () => {
    set(await persist(backStep(get())));
  },

  skip: async () => {
    set({
      ...(await persist(skipTour(get()))),
      welcomeEligible: false,
      activeContextualId: null,
    });
  },

  finish: async () => {
    set({
      ...(await persist(completeTour(get()))),
      welcomeEligible: false,
      activeContextualId: null,
    });
  },

  dismissWelcome: async () => {
    set({
      ...(await persist(
        skipTour({ ...get(), activeTutorialId: ESSENTIALS_TUTORIAL_ID }),
      )),
      welcomeEligible: false,
      activeContextualId: null,
    });
  },

  resetEssentials: async () => {
    await api.resetTutorialProgress(ESSENTIALS_TUTORIAL_ID);
    await get().hydrate();
  },

  offerContextual: (tipId) => {
    const s = get();
    if (
      !canOfferContextualTip({
        tipId,
        phase: s.phase,
        onboardingDisabled: s.onboardingDisabled,
        activeContextualId: s.activeContextualId,
        progressById: s.progressById,
      })
    ) {
      return false;
    }
    set({ activeContextualId: tipId });
    return true;
  },

  dismissContextual: async () => {
    const tipId = get().activeContextualId;
    if (!tipId) return;
    const tip = getContextualTip(tipId);
    set({ activeContextualId: null });
    if (!tip) return;
    const next = await markModuleDone(tip.id, tip.version, "completed");
    set({
      progressById: next.progressById,
      activeContextualId: null,
    });
  },

  dismissWhatsNew: async () => {
    const next = await markModuleDone(WHATS_NEW_ID, 1, "skipped");
    set({ progressById: next.progressById });
  },
}));

export function useActiveTutorialStep() {
  return useOnboardingStore((s) => ({
    phase: s.phase,
    tutorial: activeTutorial(s),
    step: activeStep(s),
    stepIndex: s.stepIndex,
  }));
}
