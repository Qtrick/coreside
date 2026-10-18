import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GenerationRecoveryBanner } from "./GenerationRecoveryBanner";
import { api } from "@/lib/tauri";
import { useAppStore } from "@/stores/app-store";

vi.mock("@/lib/tauri", () => ({
  api: {
    listRecoverableGenerations: vi.fn(),
    getRecoverableGenerationPreview: vi.fn(),
    retryRecoverableGeneration: vi.fn(),
    discardInterruptedTurn: vi.fn(),
  },
  listenAgentTurn: vi.fn(async () => () => undefined),
}));

vi.mock("@/stores/app-store", () => ({
  useAppStore: Object.assign(
    vi.fn(),
    {
      getState: vi.fn(() => ({
        previewSurfacesByKey: {},
        sendMessage: vi.fn().mockResolvedValue(undefined),
      })),
      setState: vi.fn(),
    },
  ),
}));

type Gen = {
  turnId: string;
  conversationId: string;
  attemptId: string;
  sequence: number;
  complete: boolean;
  status:
    | "previewAvailable"
    | "previewUnavailable"
    | "checkpointCorrupted"
    | "notRecoverable";
  opCount: number;
  updatedAt: string;
  canRetry: boolean;
  canDiscard: boolean;
  canReview: boolean;
};

function recoverableGen(overrides: Partial<Gen> = {}): Gen {
  return {
    turnId: "turn-1",
    conversationId: "conv-1",
    attemptId: "attempt-1",
    sequence: 1,
    complete: false,
    status: "previewAvailable",
    opCount: 1,
    updatedAt: "2026-10-17T00:00:00Z",
    canRetry: true,
    canDiscard: true,
    canReview: true,
    ...overrides,
  };
}

describe("GenerationRecoveryBanner", () => {
  beforeEach(() => {
    vi.mocked(api.listRecoverableGenerations).mockReset();
    vi.mocked(api.getRecoverableGenerationPreview).mockReset();
    vi.mocked(api.retryRecoverableGeneration).mockReset();
    vi.mocked(api.discardInterruptedTurn).mockReset();
    vi.mocked(useAppStore.setState).mockReset();
    vi.mocked(useAppStore.getState).mockReturnValue({
      previewSurfacesByKey: {},
      sendMessage: vi.fn().mockResolvedValue(undefined),
    } as never);
  });

  it("renders nothing when there are no recoverable generations", async () => {
    vi.mocked(api.listRecoverableGenerations).mockResolvedValue([]);

    const { container } = render(
      <GenerationRecoveryBanner conversationId="conv-1" />,
    );

    await waitFor(() => {
      expect(api.listRecoverableGenerations).toHaveBeenCalledWith("conv-1", 5);
    });
    expect(container.querySelector("[data-testid='generation-recovery']")).toBeNull();
  });

  it("shows Review, Retry, and Discard for a valid recoverable preview", async () => {
    vi.mocked(api.listRecoverableGenerations).mockResolvedValue([
      recoverableGen({ turnId: "turn-ok" }),
    ]);

    render(<GenerationRecoveryBanner conversationId="conv-1" />);

    expect(await screen.findByTestId("generation-recovery")).toBeDefined();
    expect(screen.getByText("Application generation interrupted")).toBeDefined();
    expect(screen.getByRole("button", { name: "Review Preview" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Retry Generation" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Discard Preview" })).toBeDefined();
  });

  it("hides Review when preview is unavailable but still offers Retry", async () => {
    vi.mocked(api.listRecoverableGenerations).mockResolvedValue([
      recoverableGen({
        turnId: "turn-empty",
        status: "previewUnavailable",
        canReview: false,
        opCount: 0,
        sequence: 0,
      }),
    ]);

    render(<GenerationRecoveryBanner conversationId="conv-1" />);

    expect(await screen.findByTestId("generation-recovery")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Review Preview" })).toBeNull();
    expect(screen.getByRole("button", { name: "Retry Generation" })).toBeDefined();
  });

  it("offers Retry and Discard for corrupted checkpoints without Review", async () => {
    vi.mocked(api.listRecoverableGenerations).mockResolvedValue([
      recoverableGen({
        turnId: "turn-corrupt",
        status: "checkpointCorrupted",
        canReview: false,
        canRetry: true,
        canDiscard: true,
        opCount: 0,
        sequence: 0,
      }),
    ]);

    render(<GenerationRecoveryBanner conversationId="conv-1" />);

    expect(await screen.findByTestId("generation-recovery")).toBeDefined();
    expect(screen.getByText("Saved preview cannot be restored")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Review Preview" })).toBeNull();
    expect(screen.getByRole("button", { name: "Retry Generation" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Discard Preview" })).toBeDefined();
  });

  it("loads a read-only recovered preview on Review", async () => {
    vi.mocked(api.listRecoverableGenerations).mockResolvedValue([
      recoverableGen({ turnId: "turn-review" }),
    ]);
    vi.mocked(api.getRecoverableGenerationPreview).mockResolvedValue({
      turnId: "turn-review",
      conversationId: "conv-1",
      attemptId: "attempt-1",
      sequence: 1,
      complete: false,
      readOnly: true,
      surfaces: [
        {
          surfaceId: "surf-1",
          toolId: "tool-1",
          applicationId: "app-1",
          definitionJson: {
            id: "tool-1",
            name: "Planner",
            layout: "stack",
            components: [],
          },
          stateJson: {},
          revision: 1,
          sequence: 1,
        },
      ],
      warnings: [],
    });

    render(<GenerationRecoveryBanner conversationId="conv-1" />);
    fireEvent.click(await screen.findByRole("button", { name: "Review Preview" }));

    await waitFor(() => {
      expect(api.getRecoverableGenerationPreview).toHaveBeenCalledWith({
        conversationId: "conv-1",
        turnId: "turn-review",
        expectedAttemptId: "attempt-1",
        expectedSequence: 1,
      });
    });
    await waitFor(() => {
      expect(useAppStore.setState).toHaveBeenCalled();
    });
    expect(
      await screen.findByRole("button", { name: "Refresh Preview" }),
    ).toBeDefined();
  });

  it("retries by retiring staging then sending the original request", async () => {
    const sendMessage = vi.fn().mockResolvedValue(undefined);
    vi.mocked(useAppStore.getState).mockReturnValue({
      previewSurfacesByKey: {},
      sendMessage,
    } as never);
    vi.mocked(api.listRecoverableGenerations)
      .mockResolvedValueOnce([recoverableGen({ turnId: "turn-retry" })])
      .mockResolvedValueOnce([]);
    vi.mocked(api.retryRecoverableGeneration).mockResolvedValue({
      retiredTurnId: "turn-retry",
      conversationId: "conv-1",
      originalContent: "Build a study planner",
      userMessageId: "msg-1",
      startsNewAttempt: true,
    });

    render(<GenerationRecoveryBanner conversationId="conv-1" />);
    fireEvent.click(
      await screen.findByRole("button", { name: "Retry Generation" }),
    );

    await waitFor(() => {
      expect(api.retryRecoverableGeneration).toHaveBeenCalledWith({
        conversationId: "conv-1",
        turnId: "turn-retry",
        expectedAttemptId: "attempt-1",
        expectedSequence: 1,
      });
    });
    await waitFor(() => {
      expect(sendMessage).toHaveBeenCalledWith("Build a study planner");
    });
  });

  it("discards with attempt and sequence CAS then clears the banner", async () => {
    vi.mocked(api.listRecoverableGenerations)
      .mockResolvedValueOnce([recoverableGen({ turnId: "turn-discard" })])
      .mockResolvedValueOnce([]);
    vi.mocked(api.discardInterruptedTurn).mockResolvedValue({
      id: "turn-discard",
      state: "failed",
      errorCategory: "discarded",
    });

    const { container } = render(
      <GenerationRecoveryBanner conversationId="conv-1" />,
    );

    fireEvent.click(
      await screen.findByRole("button", { name: "Discard Preview" }),
    );

    await waitFor(() => {
      expect(api.discardInterruptedTurn).toHaveBeenCalledWith(
        "turn-discard",
        "conv-1",
        "attempt-1",
        1,
      );
    });
    await waitFor(() => {
      expect(
        container.querySelector("[data-testid='generation-recovery']"),
      ).toBeNull();
    });
  });

  it("keeps the card and surfaces an error when discard fails", async () => {
    vi.mocked(api.listRecoverableGenerations).mockResolvedValue([
      recoverableGen({ turnId: "turn-stuck" }),
    ]);
    vi.mocked(api.discardInterruptedTurn).mockRejectedValue(
      new Error("turn_cas_conflict"),
    );

    render(<GenerationRecoveryBanner conversationId="conv-1" />);
    fireEvent.click(
      await screen.findByRole("button", { name: "Discard Preview" }),
    );

    expect(await screen.findByText("turn_cas_conflict")).toBeDefined();
    expect(screen.getByTestId("generation-recovery")).toBeDefined();
  });

  it("shows a load error when recoverable generations cannot be listed", async () => {
    vi.mocked(api.listRecoverableGenerations).mockRejectedValue(
      new Error("journal unavailable"),
    );

    render(<GenerationRecoveryBanner conversationId="conv-1" />);

    expect(await screen.findByText("journal unavailable")).toBeDefined();
    expect(screen.getByTestId("generation-recovery")).toBeDefined();
  });
});
