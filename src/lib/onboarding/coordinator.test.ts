import { describe, expect, it } from "vitest";
import {
  backStep,
  completeTour,
  emptyCoordinatorState,
  nextStep,
  pauseTour,
  showWelcome,
  skipTour,
  startTour,
} from "./coordinator";
import { ESSENTIALS_TUTORIAL_ID } from "./tutorials";

describe("onboarding coordinator", () => {
  it("starts, advances, goes back, and completes", () => {
    let state = emptyCoordinatorState();
    state = showWelcome(state);
    expect(state.phase).toBe("welcome");

    let result = startTour(state, ESSENTIALS_TUTORIAL_ID, false);
    state = result.state;
    expect(state.phase).toBe("tour");
    expect(state.stepIndex).toBeGreaterThanOrEqual(1);
    expect(result.persist?.status).toBe("in_progress");

    const at = state.stepIndex;
    result = nextStep(state);
    state = result.state;
    expect(state.stepIndex).toBe(at + 1);

    result = backStep(state);
    state = result.state;
    expect(state.stepIndex).toBe(at);

    while (state.phase === "tour") {
      result = nextStep(state);
      state = result.state;
    }
    expect(state.phase).toBe("completed");
    expect(result.persist?.status).toBe("completed");
  });

  it("skips from welcome or tour", () => {
    const state = showWelcome(emptyCoordinatorState());
    let result = skipTour({ ...state, activeTutorialId: ESSENTIALS_TUTORIAL_ID });
    expect(result.state.phase).toBe("skipped");
    expect(result.persist?.status).toBe("skipped");

    result = startTour(emptyCoordinatorState(), ESSENTIALS_TUTORIAL_ID, false);
    result = skipTour(result.state);
    expect(result.state.phase).toBe("skipped");
  });

  it("pause preserves in_progress instead of skipping", () => {
    const started = startTour(emptyCoordinatorState(), ESSENTIALS_TUTORIAL_ID, false);
    const paused = pauseTour(started.state);
    expect(paused.state.phase).toBe("idle");
    expect(paused.state.activeTutorialId).toBeNull();
    expect(paused.persist?.status).toBe("in_progress");
    expect(paused.persist?.currentStepId).toBeTruthy();

    const resumed = startTour(
      {
        ...paused.state,
        progressById: {
          [ESSENTIALS_TUTORIAL_ID]: {
            tutorialId: ESSENTIALS_TUTORIAL_ID,
            tutorialVersion: 1,
            status: "in_progress",
            currentStepId: paused.persist?.currentStepId ?? null,
            completedStepIds: paused.persist?.completedStepIds ?? [],
            startedAt: "2026-01-01T00:00:00Z",
            updatedAt: "2026-01-01T00:00:00Z",
            completedAt: null,
            skippedAt: null,
            lastOpenedAt: null,
          },
        },
      },
      ESSENTIALS_TUTORIAL_ID,
      true,
    );
    expect(resumed.state.phase).toBe("tour");
  });

  it("resumes in_progress at current step", () => {
    const seeded = emptyCoordinatorState([
      {
        tutorialId: ESSENTIALS_TUTORIAL_ID,
        tutorialVersion: 1,
        status: "in_progress",
        currentStepId: "app-panel",
        completedStepIds: ["welcome", "chat-composer"],
        startedAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
        completedAt: null,
        skippedAt: null,
        lastOpenedAt: null,
      },
    ]);
    const result = startTour(seeded, ESSENTIALS_TUTORIAL_ID, true);
    expect(result.state.stepIndex).toBe(2);
  });

  it("skip from welcome marks essentials skipped without requiring active tour", () => {
    const state = showWelcome(emptyCoordinatorState());
    const result = skipTour({
      ...state,
      activeTutorialId: ESSENTIALS_TUTORIAL_ID,
    });
    expect(result.state.phase).toBe("skipped");
    expect(result.persist?.status).toBe("skipped");
  });

  it("completeTour marks all step ids", () => {
    const started = startTour(emptyCoordinatorState(), ESSENTIALS_TUTORIAL_ID, false);
    const done = completeTour(started.state);
    expect(done.persist?.completedStepIds.length).toBeGreaterThan(3);
    expect(done.state.phase).toBe("completed");
  });
});
