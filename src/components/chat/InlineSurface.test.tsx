import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { InlineSurfaceCard } from "./InlineSurface";
import type { SurfaceRecord } from "@/types/runtime-v2";

const mockSendMessage = vi.fn();
const mockSetSurfaceDraftConflict = vi.fn();

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
  },
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.code = code;
    }
  },
}));

// Mock ToolRenderer to simulate user actions
vi.mock("@/components/tool-renderer/ToolRenderer", () => ({
  ToolRenderer: ({ onSubmitToAgent }: { onSubmitToAgent: (payload: { componentId: string; eventName: string; values: Record<string, unknown> }) => Promise<void> }) => {
    return (
      <div data-testid="mock-tool-renderer">
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

describe("InlineSurfaceCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    expect(mockSendMessage).toHaveBeenCalledTimes(1);
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
    expect(mockSendMessage).toHaveBeenCalledTimes(2);
  });
});
