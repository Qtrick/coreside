import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  compactStateForModel,
  hasInteractiveDefinition,
  useInteractiveSurface,
} from "./interactive-surface";
import type { InteractiveView } from "@/types/runtime-v2";

const mockInteractiveView = vi.fn();
const mockInteractiveDispatch = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    interactiveView: (...args: unknown[]) => mockInteractiveView(...args),
    interactiveDispatch: (...args: unknown[]) => mockInteractiveDispatch(...args),
    interactiveUndo: vi.fn(),
    interactiveHistory: vi.fn().mockResolvedValue([]),
    interactiveReplay: vi.fn(),
  },
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.code = code;
    }
  },
}));

function publicView(overrides: Partial<InteractiveView> = {}): InteractiveView {
  return {
    surfaceId: "surf-quiz",
    applicationId: "app-quiz",
    stateRevision: 4,
    state: { score: 1, status: "in_progress", currentQuestion: 1 },
    actor: "user",
    waitingFor: null,
    status: "in_progress",
    winner: null,
    message: null,
    legalActions: [],
    seq: 2,
    duplicate: false,
    reinitialized: false,
    ...overrides,
  };
}

describe("compactStateForModel", () => {
  it("returns the same public state when under budget", () => {
    const state = { score: 2, status: "in_progress", theme: "dark" };
    expect(compactStateForModel(state)).toBe(state);
    expect(compactStateForModel(state)).toEqual(state);
  });

  it("throws instead of truncating oversized state", () => {
    const huge = { payload: "x".repeat(50_000) };
    expect(() => compactStateForModel(huge, 1_000)).toThrow(
      /exceeds model budget/,
    );
    expect(() => compactStateForModel(huge, 1_000)).toThrow(/500\d\d > 1000/);
  });
});

describe("hasInteractiveDefinition", () => {
  it("detects interactive blobs and rejects empty definitions", () => {
    expect(hasInteractiveDefinition({ interactive: { id: "app" } })).toBe(true);
    expect(hasInteractiveDefinition({ interactive: null })).toBe(false);
    expect(hasInteractiveDefinition({})).toBe(false);
    expect(hasInteractiveDefinition(null)).toBe(false);
  });
});

describe("useInteractiveSurface adopt", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("replaces local state so leftover hidden keys cannot survive adopt", async () => {
    const revisionRef = { current: 1 };
    let localState: Record<string, unknown> = {
      answers: [1, 0, 0],
      score: 0,
      staleSecret: "keep-me-if-merge",
    };
    const setState = vi.fn(
      (update: (current: Record<string, unknown>) => Record<string, unknown>) => {
        localState = update(localState);
      },
    );

    mockInteractiveView.mockResolvedValue(
      publicView({
        state: { score: 1, status: "in_progress", currentQuestion: 1 },
        stateRevision: 5,
      }),
    );

    const { result } = renderHook(() =>
      useInteractiveSurface({
        surfaceId: "surf-quiz",
        stateRevisionRef: revisionRef,
        setState: setState as never,
      }),
    );

    await act(async () => {
      await result.current.hydrate();
    });

    expect(setState).toHaveBeenCalled();
    expect(localState).toEqual({
      score: 1,
      status: "in_progress",
      currentQuestion: 1,
    });
    expect(localState).not.toHaveProperty("answers");
    expect(localState).not.toHaveProperty("staleSecret");
    expect(revisionRef.current).toBe(5);
  });

  it("dispatch adopt also replaces rather than merges", async () => {
    const revisionRef = { current: 5 };
    let localState: Record<string, unknown> = {
      answers: [9, 9, 9],
      score: 1,
    };
    const setState = vi.fn(
      (update: (current: Record<string, unknown>) => Record<string, unknown>) => {
        localState = update(localState);
      },
    );

    mockInteractiveDispatch.mockResolvedValue(
      publicView({
        state: { score: 2, status: "completed", lastResult: "correct" },
        stateRevision: 6,
        seq: 3,
        status: "completed",
      }),
    );

    const { result } = renderHook(() =>
      useInteractiveSurface({
        surfaceId: "surf-quiz",
        stateRevisionRef: revisionRef,
        setState: setState as never,
      }),
    );

    await act(async () => {
      const outcome = await result.current.dispatch({
        actionId: "answer",
        params: { choice: 0 },
      });
      expect(outcome.ok).toBe(true);
    });

    await waitFor(() => {
      expect(localState).toEqual({
        score: 2,
        status: "completed",
        lastResult: "correct",
      });
    });
    expect(localState).not.toHaveProperty("answers");
  });
});
