import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GenerationRecoveryBanner } from "./GenerationRecoveryBanner";
import { api } from "@/lib/tauri";

vi.mock("@/lib/tauri", () => ({
  api: {
    listInterruptedTurns: vi.fn(),
    discardInterruptedTurn: vi.fn(),
  },
}));

type InterruptedRow = {
  id: string;
  conversationId: string;
  attemptId: string;
  state: string;
  operationsJson?: string | null;
  provisionalText?: string | null;
  updatedAt: string;
};

function recoverableTurn(
  overrides: Partial<InterruptedRow> = {},
): InterruptedRow {
  return {
    id: "turn-1",
    conversationId: "conv-1",
    attemptId: "attempt-1",
    state: "interrupted_recoverable",
    operationsJson: JSON.stringify({
      v: "coreside.ops.checkpoint.v1",
      ops: [{ id: "op-1" }],
    }),
    provisionalText: "Building…",
    updatedAt: "2026-10-17T00:00:00Z",
    ...overrides,
  };
}

describe("GenerationRecoveryBanner", () => {
  beforeEach(() => {
    vi.mocked(api.listInterruptedTurns).mockReset();
    vi.mocked(api.discardInterruptedTurn).mockReset();
  });

  it("renders nothing when there are no recoverable staged turns", async () => {
    vi.mocked(api.listInterruptedTurns).mockResolvedValue([]);

    const { container } = render(
      <GenerationRecoveryBanner conversationId="conv-1" />,
    );

    await waitFor(() => {
      expect(api.listInterruptedTurns).toHaveBeenCalledWith("conv-1", 5);
    });
    expect(container.querySelector("[data-testid='generation-recovery']")).toBeNull();
    expect(screen.queryByText("Application generation interrupted")).toBeNull();
  });

  it("hides turns that lack durable ops staging or are not interrupted_recoverable", async () => {
    vi.mocked(api.listInterruptedTurns).mockResolvedValue([
      recoverableTurn({
        id: "turn-wrong-state",
        state: "failed",
      }),
      recoverableTurn({
        id: "turn-empty-array",
        operationsJson: "[]",
      }),
      recoverableTurn({
        id: "turn-null-ops",
        operationsJson: null,
      }),
      recoverableTurn({
        id: "turn-missing-ops",
        operationsJson: undefined,
      }),
    ]);

    const { container } = render(
      <GenerationRecoveryBanner conversationId="conv-1" />,
    );

    await waitFor(() => {
      expect(api.listInterruptedTurns).toHaveBeenCalled();
    });
    expect(container.querySelector("[data-testid='generation-recovery']")).toBeNull();
    expect(screen.queryByRole("button", { name: "Discard Preview" })).toBeNull();
  });

  it("shows a recovery card for interrupted turns with durable ops staging", async () => {
    vi.mocked(api.listInterruptedTurns).mockResolvedValue([
      recoverableTurn({ id: "turn-ok" }),
    ]);

    render(<GenerationRecoveryBanner conversationId="conv-1" />);

    expect(
      await screen.findByTestId("generation-recovery"),
    ).toBeDefined();
    expect(screen.getByText("Application generation interrupted")).toBeDefined();
    expect(
      screen.getByRole("button", { name: "Discard Preview" }),
    ).toBeDefined();
  });

  it("discards staging then refreshes so the banner clears when nothing remains", async () => {
    vi.mocked(api.listInterruptedTurns)
      .mockResolvedValueOnce([recoverableTurn({ id: "turn-discard" })])
      .mockResolvedValueOnce([]);
    vi.mocked(api.discardInterruptedTurn).mockResolvedValue({
      id: "turn-discard",
      state: "failed",
      errorCategory: "discarded",
    });

    const { container } = render(
      <GenerationRecoveryBanner conversationId="conv-1" />,
    );

    const discard = await screen.findByRole("button", {
      name: "Discard Preview",
    });
    fireEvent.click(discard);

    await waitFor(() => {
      expect(api.discardInterruptedTurn).toHaveBeenCalledWith("turn-discard");
    });
    await waitFor(() => {
      expect(api.listInterruptedTurns).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(
        container.querySelector("[data-testid='generation-recovery']"),
      ).toBeNull();
    });
  });

  it("keeps the card and surfaces an error when discard fails", async () => {
    vi.mocked(api.listInterruptedTurns).mockResolvedValue([
      recoverableTurn({ id: "turn-stuck" }),
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
    expect(
      screen.getByRole("button", { name: "Discard Preview" }),
    ).not.toBeDisabled();
  });

  it("shows a load error when interrupted turns cannot be listed", async () => {
    vi.mocked(api.listInterruptedTurns).mockRejectedValue(
      new Error("journal unavailable"),
    );

    render(<GenerationRecoveryBanner conversationId="conv-1" />);

    expect(await screen.findByText("journal unavailable")).toBeDefined();
    expect(screen.getByTestId("generation-recovery")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Discard Preview" })).toBeNull();
  });
});
