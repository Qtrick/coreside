import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { InlineSurfaceCard } from "./InlineSurface";
import { api } from "@/lib/tauri";
import type { SurfaceRecord } from "@/types/runtime-v2";

const mockSendMessage = vi.fn();
const mockSetSurfaceDraftConflict = vi.fn();
const rendererStates: Array<Record<string, unknown>> = [];

vi.mock("@/stores/app-store", () => ({
  useAppStore: (selector: (s: unknown) => unknown) => {
    const state = {
      sendMessage: mockSendMessage,
      activeProjectId: "proj-1",
      setSurfaceDraftConflict: mockSetSurfaceDraftConflict,
    };
    return selector(state);
  },
}));

vi.mock("@/lib/tauri", () => ({
  api: {
    getSurfaceStateWithRevision: vi.fn().mockResolvedValue({ state: { count: 0 }, stateRevision: 2 }),
    getSurfaceState: vi.fn().mockResolvedValue({ count: 0 }),
    getContinuity: vi.fn().mockResolvedValue(null),
    kernelGetManifest: vi.fn().mockResolvedValue({ applicationId: "app-test" }),
    saveDraft: vi.fn().mockResolvedValue(undefined),
    saveSurfaceState: vi.fn().mockResolvedValue(3),
    suspendSurface: vi.fn().mockResolvedValue(undefined),
    saveContinuity: vi.fn().mockResolvedValue(undefined),
    interactiveView: vi.fn(),
    interactiveDispatch: vi.fn(),
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

vi.mock("@/lib/preservation", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/preservation")>();
  return {
    ...actual,
    restoreScrollSnapshot: vi.fn(actual.restoreScrollSnapshot),
  };
});

// Mock ToolRenderer to simulate user actions and capture state props
vi.mock("@/components/tool-renderer/ToolRenderer", () => ({
  ToolRenderer: ({
    state,
    onSubmitToAgent,
  }: {
    state: Record<string, unknown>;
    onSubmitToAgent: (payload: {
      componentId: string;
      eventName: string;
      values: Record<string, unknown>;
    }) => Promise<void>;
  }) => {
    rendererStates.push({ ...state });
    return (
      <div data-testid="mock-tool-renderer" data-state-keys={Object.keys(state).join(",")}>
        <button
          type="button"
          data-testid="interactive-btn"
          onClick={() => {
            void onSubmitToAgent({
              componentId: "btn-counter",
              eventName: "game.move",
              values: { x: 1, y: 2 },
            });
          }}
        >
          Click Me
        </button>
      </div>
    );
  },
}));

const sampleSurface: SurfaceRecord = {
  id: "surf-100",
  instanceId: "inst-100",
  surfaceType: "inline",
  placement: "chat_inline",
  ownerType: "conversation",
  lifecycleState: "active",
  conversationId: "conv-100",
  projectId: "proj-1",
  toolId: null,
  messageId: null,
  name: "Counter App",
  archived: false,
  currentRevision: 5,
  capabilityPacks: ["coreside.core"],
  definition: {
    id: "tool-100",
    name: "Counter App",
    layout: { type: "single-column" },
    components: [],
  },
  createdAt: "2026-09-27T00:00:00Z",
  updatedAt: "2026-09-27T00:00:00Z",
};

const quizSurface: SurfaceRecord = {
  ...sampleSurface,
  id: "surf-quiz",
  name: "Planets Quiz",
  definition: {
    id: "tool-quiz",
    name: "Planets Quiz",
    layout: { type: "single-column" },
    components: [],
    interactive: {
      id: "app-quiz",
      kind: "quiz",
      stateSchema: [{ key: "answers", readPolicy: "restricted" }],
    },
  },
};

describe("InlineSurfaceCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    rendererStates.length = 0;
    vi.mocked(api.getSurfaceStateWithRevision).mockResolvedValue({
      state: { count: 0 },
      stateRevision: 2,
    });
  });

  it("renders surface title and revision", async () => {
    render(<InlineSurfaceCard surface={sampleSurface} conversationId="conv-100" />);
    expect(screen.getByText("Counter App")).toBeDefined();
    expect(screen.getByText("r5")).toBeDefined();
  });

  it("serializes interactive submissions and passes authoritative revisions and idempotency key", async () => {
    let resolveSend: (() => void) | null = null;
    mockSendMessage.mockImplementation(() => new Promise<void>((resolve) => {
      resolveSend = resolve;
    }));

    render(<InlineSurfaceCard surface={sampleSurface} conversationId="conv-100" />);

    await waitFor(() => {
      expect(screen.getByTestId("interactive-btn")).toBeDefined();
    });

    const btn = screen.getByTestId("interactive-btn");

    // Click once: starts first interaction
    fireEvent.click(btn);

    await waitFor(() => {
      expect(mockSendMessage).toHaveBeenCalledTimes(1);
    });
    expect(mockSendMessage).toHaveBeenCalledWith(
      "App interaction (game.move)",
      [],
      [],
      expect.objectContaining({
        formId: "btn-counter",
        eventName: "game.move",
        applicationId: "surf-100",
        surfaceId: "surf-100",
        surfaceRevision: 5,
        stateRevision: 2,
        componentId: "btn-counter",
        fields: expect.objectContaining({
          silent: true,
          x: 1,
          y: 2,
        }),
      })
    );

    // Verify visual busy indicator is displayed
    expect(screen.getByText("processing...")).toBeDefined();

    // Click again while first is in-flight: MUST BE DROPPED by interactionLockRef
    fireEvent.click(btn);
    expect(mockSendMessage).toHaveBeenCalledTimes(1);

    // Finish first interaction
    resolveSend!();
    await waitFor(() => {
      expect(screen.queryByText("processing...")).toBeNull();
    });

    // Now a subsequent click should be accepted
    fireEvent.click(btn);
    await waitFor(() => {
      expect(mockSendMessage).toHaveBeenCalledTimes(2);
    });
  });

  it("interactive path does not seed React state from raw restricted answers", async () => {
    vi.mocked(api.getSurfaceStateWithRevision).mockResolvedValue({
      state: { answers: [1, 0, 0], score: 0, status: "in_progress" },
      stateRevision: 7,
    });
    vi.mocked(api.interactiveView).mockResolvedValue({
      surfaceId: "surf-quiz",
      applicationId: "app-quiz",
      stateRevision: 7,
      state: { score: 0, status: "in_progress", currentQuestion: 0 },
      actor: "user",
      waitingFor: null,
      status: "in_progress",
      winner: null,
      message: null,
      legalActions: [],
      seq: 1,
      duplicate: false,
      reinitialized: false,
    });

    render(<InlineSurfaceCard surface={quizSurface} conversationId="conv-100" />);

    await waitFor(() => {
      expect(api.interactiveView).toHaveBeenCalledWith("surf-quiz");
    });
    await waitFor(() => {
      expect(rendererStates.some((s) => s.score === 0 && s.status === "in_progress")).toBe(true);
    });

    for (const state of rendererStates) {
      expect(state).not.toHaveProperty("answers");
    }
    // Raw getSurfaceStateWithRevision must not be used as the interactive seed payload.
    expect(api.getSurfaceState).not.toHaveBeenCalled();
  });

  it("keeps latestSurfaceRef current so hydration seals the newest definition", async () => {
    let resolveContinuity: ((value: never) => void) | null = null;
    vi.mocked(api.getContinuity).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveContinuity = resolve as (value: never) => void;
        }),
    );
    vi.mocked(api.getSurfaceStateWithRevision).mockResolvedValue({
      state: { count: 1 },
      stateRevision: 3,
    });

    const { rerender } = render(
      <InlineSurfaceCard surface={sampleSurface} conversationId="conv-100" />,
    );

    // Mid-hydration: definition advances while continuity is still pending.
    const updated: SurfaceRecord = {
      ...sampleSurface,
      currentRevision: 6,
      definition: {
        ...(sampleSurface.definition as object),
        name: "Counter App v2",
        components: [{ id: "c1", type: "text", props: { text: "updated" } }],
      },
    };
    rerender(<InlineSurfaceCard surface={updated} conversationId="conv-100" />);

    resolveContinuity!(null as never);

    await waitFor(() => {
      expect(screen.getByText("r6")).toBeDefined();
    });
    await waitFor(() => {
      expect(api.getSurfaceStateWithRevision).toHaveBeenCalled();
    });
  });

  it("fails closed when hydration throws instead of marking empty state as hydrated", async () => {
    const preservation = await import("@/lib/preservation");
    vi.mocked(preservation.restoreScrollSnapshot).mockImplementation(() => {
      throw new Error("continuity restore exploded");
    });
    vi.mocked(api.getContinuity).mockResolvedValue({
      id: "cont-1",
      surfaceId: sampleSurface.id,
      windowId: "main",
      scroll: { root: { scrollTop: 10 } },
      focus: {},
      media: {},
      suspensionState: "active",
      updatedAt: "2026-09-29T00:00:00Z",
    });
    vi.mocked(api.getSurfaceStateWithRevision).mockResolvedValue({
      state: { count: 0 },
      stateRevision: 2,
    });

    render(<InlineSurfaceCard surface={sampleSurface} conversationId="conv-100" />);

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeDefined();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/continuity restore exploded/);
    expect(api.interactiveView).not.toHaveBeenCalled();
  });

  it("fails closed when surface state APIs both fail", async () => {
    vi.mocked(api.getSurfaceStateWithRevision).mockRejectedValue(new Error("rev unavailable"));
    vi.mocked(api.getSurfaceState).mockRejectedValue(new Error("state unavailable"));
    vi.mocked(api.getContinuity).mockRejectedValue(new Error("no continuity"));

    render(<InlineSurfaceCard surface={sampleSurface} conversationId="conv-100" />);

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeDefined();
    });
    expect(screen.getByRole("alert").textContent).toMatch(/state unavailable|rev unavailable/);
    expect(api.interactiveView).not.toHaveBeenCalled();
  });
});
