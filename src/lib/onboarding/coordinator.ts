import type {
  CoordinatorPhase,
  CoordinatorState,
  TutorialDefinition,
  TutorialProgress,
  TutorialStatus,
} from "./types";
import { ESSENTIALS_TUTORIAL_ID, getTutorial } from "./tutorials";

export function emptyCoordinatorState(
  progress: TutorialProgress[] = [],
): CoordinatorState {
  const progressById: Record<string, TutorialProgress> = {};
  for (const row of progress) progressById[row.tutorialId] = row;
  return {
    phase: "idle",
    activeTutorialId: null,
    stepIndex: 0,
    progressById,
  };
}

export function progressFor(
  state: CoordinatorState,
  tutorialId: string,
): TutorialProgress | undefined {
  return state.progressById[tutorialId];
}

function clampIndex(tutorial: TutorialDefinition, index: number): number {
  if (tutorial.steps.length === 0) return 0;
  return Math.max(0, Math.min(index, tutorial.steps.length - 1));
}

function completedThrough(
  tutorial: TutorialDefinition,
  stepIndex: number,
): string[] {
  return tutorial.steps.slice(0, stepIndex + 1).map((s) => s.id);
}

export type PersistPatch = {
  tutorialId: string;
  tutorialVersion: number;
  status: TutorialStatus;
  currentStepId: string | null;
  completedStepIds: string[];
};

export type CoordinatorResult = {
  state: CoordinatorState;
  persist?: PersistPatch;
};

export function showWelcome(state: CoordinatorState): CoordinatorState {
  return { ...state, phase: "welcome", activeTutorialId: null, stepIndex: 0 };
}

export function startTour(
  state: CoordinatorState,
  tutorialId: string = ESSENTIALS_TUTORIAL_ID,
  resume = true,
): CoordinatorResult {
  const tutorial = getTutorial(tutorialId);
  if (!tutorial) return { state };

  const existing = progressFor(state, tutorialId);
  let stepIndex = 0;
  if (resume && existing?.status === "in_progress" && existing.currentStepId) {
    const idx = tutorial.steps.findIndex((s) => s.id === existing.currentStepId);
    if (idx >= 0) stepIndex = idx;
  }
  // Skip pure welcome step when starting the overlay tour.
  if (tutorial.steps[stepIndex]?.kind === "welcome" && tutorial.steps.length > 1) {
    stepIndex = 1;
  }
  stepIndex = clampIndex(tutorial, stepIndex);
  const step = tutorial.steps[stepIndex];
  const next: CoordinatorState = {
    ...state,
    phase: "tour",
    activeTutorialId: tutorialId,
    stepIndex,
  };
  return {
    state: next,
    persist: {
      tutorialId,
      tutorialVersion: tutorial.version,
      status: "in_progress",
      currentStepId: step?.id ?? null,
      completedStepIds: completedThrough(tutorial, Math.max(0, stepIndex - 1)),
    },
  };
}

export function nextStep(state: CoordinatorState): CoordinatorResult {
  if (!state.activeTutorialId || state.phase !== "tour") return { state };
  const tutorial = getTutorial(state.activeTutorialId);
  if (!tutorial) return { state };

  const last = tutorial.steps.length - 1;
  if (state.stepIndex >= last) {
    return completeTour(state);
  }
  const stepIndex = state.stepIndex + 1;
  const step = tutorial.steps[stepIndex];
  return {
    state: { ...state, stepIndex },
    persist: {
      tutorialId: tutorial.id,
      tutorialVersion: tutorial.version,
      status: "in_progress",
      currentStepId: step.id,
      completedStepIds: completedThrough(tutorial, stepIndex - 1),
    },
  };
}

export function backStep(state: CoordinatorState): CoordinatorResult {
  if (!state.activeTutorialId || state.phase !== "tour") return { state };
  const tutorial = getTutorial(state.activeTutorialId);
  if (!tutorial || state.stepIndex <= 0) return { state };
  // Do not go back into the welcome modal step while overlay is open.
  const min = tutorial.steps[0]?.kind === "welcome" ? 1 : 0;
  if (state.stepIndex <= min) return { state };
  const stepIndex = state.stepIndex - 1;
  const step = tutorial.steps[stepIndex];
  return {
    state: { ...state, stepIndex },
    persist: {
      tutorialId: tutorial.id,
      tutorialVersion: tutorial.version,
      status: "in_progress",
      currentStepId: step.id,
      completedStepIds: completedThrough(tutorial, stepIndex - 1),
    },
  };
}

export function pauseTour(state: CoordinatorState): CoordinatorResult {
  if (!state.activeTutorialId || state.phase !== "tour") {
    return { state: { ...state, phase: "idle", activeTutorialId: null } };
  }
  const tutorial = getTutorial(state.activeTutorialId);
  const step = tutorial?.steps[state.stepIndex];
  const existing = progressFor(state, state.activeTutorialId);
  return {
    state: {
      ...state,
      phase: "idle",
      // Keep progress in_progress; clear only the live overlay session.
      activeTutorialId: null,
      stepIndex: 0,
    },
    persist: {
      tutorialId: state.activeTutorialId,
      tutorialVersion: tutorial?.version ?? existing?.tutorialVersion ?? 1,
      status: "in_progress",
      currentStepId: step?.id ?? existing?.currentStepId ?? null,
      completedStepIds: existing?.completedStepIds ?? [],
    },
  };
}

export function skipTour(state: CoordinatorState): CoordinatorResult {
  const tutorialId = state.activeTutorialId ?? ESSENTIALS_TUTORIAL_ID;
  const tutorial = getTutorial(tutorialId);
  const existing = progressFor(state, tutorialId);
  const next: CoordinatorState = {
    ...state,
    phase: "skipped",
    activeTutorialId: null,
    stepIndex: 0,
  };
  return {
    state: next,
    persist: {
      tutorialId,
      tutorialVersion: tutorial?.version ?? existing?.tutorialVersion ?? 1,
      status: "skipped",
      currentStepId: null,
      completedStepIds: existing?.completedStepIds ?? [],
    },
  };
}

export function completeTour(state: CoordinatorState): CoordinatorResult {
  const tutorialId = state.activeTutorialId ?? ESSENTIALS_TUTORIAL_ID;
  const tutorial = getTutorial(tutorialId);
  if (!tutorial) {
    return { state: { ...state, phase: "completed", activeTutorialId: null } };
  }
  return {
    state: {
      ...state,
      phase: "completed" satisfies CoordinatorPhase,
      activeTutorialId: null,
      stepIndex: 0,
    },
    persist: {
      tutorialId: tutorial.id,
      tutorialVersion: tutorial.version,
      status: "completed",
      currentStepId: null,
      completedStepIds: tutorial.steps.map((s) => s.id),
    },
  };
}

export function applyPersistedProgress(
  state: CoordinatorState,
  row: TutorialProgress,
): CoordinatorState {
  return {
    ...state,
    progressById: { ...state.progressById, [row.tutorialId]: row },
  };
}

export function activeStep(state: CoordinatorState) {
  if (!state.activeTutorialId) return null;
  const tutorial = getTutorial(state.activeTutorialId);
  if (!tutorial) return null;
  return tutorial.steps[state.stepIndex] ?? null;
}

export function activeTutorial(state: CoordinatorState) {
  if (!state.activeTutorialId) return null;
  return getTutorial(state.activeTutorialId) ?? null;
}
