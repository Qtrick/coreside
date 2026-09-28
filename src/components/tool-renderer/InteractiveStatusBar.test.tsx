import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { InteractiveStatusBar } from "./InteractiveStatusBar";
import type { InteractiveHistoryEntry, InteractiveView } from "@/types/runtime-v2";

function view(overrides: Partial<InteractiveView> = {}): InteractiveView {
  return {
    surfaceId: "surf-1",
    applicationId: "game-tictactoe",
    stateRevision: 3,
    state: { currentPlayer: "O" },
    actor: "O",
    waitingFor: null,
    status: "playing",
    winner: null,
    message: null,
    legalActions: [],
    seq: 3,
    duplicate: false,
    reinitialized: false,
    ...overrides,
  };
}

const history: InteractiveHistoryEntry[] = [
  {
    seq: 1,
    eventId: "init",
    actionId: "__init",
    actor: "system",
    params: null,
    checkpoint: true,
  },
  {
    seq: 2,
    eventId: "m-1",
    actionId: "move",
    actor: "X",
    params: { index: 4 },
    checkpoint: false,
  },
];

describe("InteractiveStatusBar", () => {
  it("renders nothing without a view", () => {
    const { container } = render(
      <InteractiveStatusBar
        view={null}
        error={null}
        pending={false}
        onUndo={vi.fn()}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("loads history when the panel opens and lists recorded actions", async () => {
    const onLoadHistory = vi.fn();
    const onReplayAt = vi.fn();
    const { rerender } = render(
      <InteractiveStatusBar
        view={view()}
        error={null}
        pending={false}
        history={[]}
        onUndo={vi.fn()}
        onLoadHistory={onLoadHistory}
        onReplayAt={onReplayAt}
      />,
    );

    expect(screen.queryByLabelText("Action history")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show history" }));
    await waitFor(() => expect(onLoadHistory).toHaveBeenCalledTimes(1));
    expect(screen.getByText("No recorded actions yet.")).toBeInTheDocument();

    rerender(
      <InteractiveStatusBar
        view={view()}
        error={null}
        pending={false}
        history={history}
        onUndo={vi.fn()}
        onLoadHistory={onLoadHistory}
        onReplayAt={onReplayAt}
      />,
    );

    expect(screen.getByText("move")).toBeInTheDocument();
    expect(screen.getByText("checkpoint")).toBeInTheDocument();
    fireEvent.click(screen.getByText("move"));
    expect(onReplayAt).toHaveBeenCalledWith(2);
  });

  it("disables undo while pending, at the initial seq, or while inspecting", () => {
    const onUndo = vi.fn();
    const { rerender } = render(
      <InteractiveStatusBar
        view={view({ seq: 3 })}
        error={null}
        pending
        onUndo={onUndo}
      />,
    );
    expect(screen.getByRole("button", { name: "Undo last move" })).toBeDisabled();
    expect(screen.getByText("Applying…")).toBeInTheDocument();

    rerender(
      <InteractiveStatusBar
        view={view({ seq: 1 })}
        error={null}
        pending={false}
        onUndo={onUndo}
      />,
    );
    expect(screen.getByRole("button", { name: "Undo last move" })).toBeDisabled();

    rerender(
      <InteractiveStatusBar
        view={view({ seq: 3 })}
        error={null}
        pending={false}
        replaySeq={2}
        onUndo={onUndo}
        onClearReplay={vi.fn()}
      />,
    );
    expect(screen.getByText("Inspecting step 2")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo last move" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Exit history inspection" }),
    ).toBeInTheDocument();
  });

  it("invokes undo when enabled", () => {
    const onUndo = vi.fn();
    render(
      <InteractiveStatusBar
        view={view({ seq: 3 })}
        error={null}
        pending={false}
        onUndo={onUndo}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Undo last move" }));
    expect(onUndo).toHaveBeenCalledTimes(1);
  });
});
