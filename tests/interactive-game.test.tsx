import { describe, expect, it, vi } from "vitest";
import { applyAction, applyActions } from "@/lib/actions";
import type { ToolComponent } from "@/types/tool";
import { render, screen } from "@testing-library/react";
import { ToolRuntimeProvider } from "@/components/tool-renderer/context";
import { ButtonNode, BadgeNode } from "@/components/tool-renderer/nodes";

describe("Interactive AI Applications & Games (Tic-Tac-Toe vs AI)", () => {
  it("executes sequential game move actions: updates cell, records lastMove, and dispatches submitToAgent", () => {
    const state: Record<string, unknown> = {
      c0: "",
      c1: "",
      c2: "",
      c3: "",
      c4: "",
      c5: "",
      c6: "",
      c7: "",
      c8: "",
      lastMove: null,
      turn: "X",
      status: "Your turn (X). Click any square to play!",
    };
    const allowedTargets = [
      "c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8",
      "lastMove", "turn", "status",
    ];
    const onSubmitToAgent = vi.fn();

    // User clicks cell 4 (center)
    const moveActions = [
      { type: "setValue" as const, target: "c4", value: "X" },
      { type: "setValue" as const, target: "lastMove", value: 4 },
      {
        type: "submitToAgent" as const,
        eventName: "game.move",
        includeFields: ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "lastMove", "turn"],
        silent: true,
      },
    ];

    const result = applyActions(moveActions, {
      state,
      toolId: "tool-tictactoe",
      allowedTargets,
      onSubmitToAgent,
    });

    // 1. State was sequentially updated
    expect(result.state.c4).toBe("X");
    expect(result.state.lastMove).toBe(4);

    // 2. submitToAgent was invoked with accurate payload
    expect(onSubmitToAgent).toHaveBeenCalledTimes(1);
    expect(onSubmitToAgent).toHaveBeenCalledWith({
      toolId: "tool-tictactoe",
      eventName: "game.move",
      values: {
        c0: "",
        c1: "",
        c2: "",
        c3: "",
        c4: "X",
        c5: "",
        c6: "",
        c7: "",
        c8: "",
        lastMove: 4,
        turn: "X",
      },
      silent: true,
    });
  });

  it("handles game.reset action to clear board", () => {
    const state: Record<string, unknown> = {
      c0: "X",
      c1: "O",
      c4: "X",
      status: "AI played square 2. Your turn (X)!",
    };
    const allowedTargets = ["c0", "c1", "c4", "status"];
    const onSubmitToAgent = vi.fn();

    const resetActions = [
      {
        type: "submitToAgent" as const,
        eventName: "game.reset",
        includeFields: ["status"],
        silent: true,
      },
    ];

    applyActions(resetActions, {
      state,
      toolId: "tool-tictactoe",
      allowedTargets,
      onSubmitToAgent,
    });

    expect(onSubmitToAgent).toHaveBeenCalledTimes(1);
    expect(onSubmitToAgent).toHaveBeenCalledWith({
      toolId: "tool-tictactoe",
      eventName: "game.reset",
      values: {
        status: "AI played square 2. Your turn (X)!",
      },
      silent: true,
    });
  });

  it("renders dynamic button label from bound stateKey (c0 -> 'X')", () => {
    const component: ToolComponent = {
      id: "ttt-c0",
      type: "button",
      props: { label: "·", valueKey: "c0" },
      actions: [],
    };

    const mockState = { c0: "X" };
    const mockGetValue = vi.fn((key: string, fallback?: unknown) => mockState[key as keyof typeof mockState] ?? fallback);

    render(
      <ToolRuntimeProvider
        value={{
          toolId: "tool-tictactoe",
          state: mockState,
          getValue: mockGetValue as unknown as <T>(key: string, fallback?: T) => T,
          setValue: vi.fn(),
          setValueOptimistic: vi.fn(),
          runActions: vi.fn(),
        }}
      >
        <ButtonNode component={component} renderChild={vi.fn()} />
      </ToolRuntimeProvider>
    );

    expect(screen.getByRole("button")).toHaveTextContent("X");
  });

  it("renders default button label when bound state key is empty or null", () => {
    const component: ToolComponent = {
      id: "ttt-c0",
      type: "button",
      props: { label: "·", valueKey: "c0" },
      actions: [],
    };

    const mockState = { c0: "" };
    const mockGetValue = vi.fn((key: string, fallback?: unknown) => mockState[key as keyof typeof mockState] ?? fallback);

    render(
      <ToolRuntimeProvider
        value={{
          toolId: "tool-tictactoe",
          state: mockState,
          getValue: mockGetValue as unknown as <T>(key: string, fallback?: T) => T,
          setValue: vi.fn(),
          setValueOptimistic: vi.fn(),
          runActions: vi.fn(),
        }}
      >
        <ButtonNode component={component} renderChild={vi.fn()} />
      </ToolRuntimeProvider>
    );

    expect(screen.getByRole("button")).toHaveTextContent("·");
  });

  it("renders dynamic badge text from bound stateKey (status message)", () => {
    const component: ToolComponent = {
      id: "ttt-status",
      type: "badge",
      props: { text: "Initial", valueKey: "status" },
    };

    const mockState = { status: "AI played square 5. Your turn (X)!" };
    const mockGetValue = vi.fn((key: string, fallback?: unknown) => mockState[key as keyof typeof mockState] ?? fallback);

    const { container } = render(
      <ToolRuntimeProvider
        value={{
          toolId: "tool-tictactoe",
          state: mockState,
          getValue: mockGetValue as unknown as <T>(key: string, fallback?: T) => T,
          setValue: vi.fn(),
          setValueOptimistic: vi.fn(),
          runActions: vi.fn(),
        }}
      >
        <BadgeNode component={component} renderChild={vi.fn()} />
      </ToolRuntimeProvider>
    );

    expect(container).toHaveTextContent("AI played square 5. Your turn (X)!");
  });

  it("rejects unauthorized state mutation on undeclared targets", () => {
    const state: Record<string, unknown> = {
      c0: "",
      secretAdminKey: "protected",
    };
    const allowedTargets = ["c0"];

    const result = applyAction(
      { type: "setValue", target: "secretAdminKey", value: "tampered" },
      { state, toolId: "tool-tictactoe", allowedTargets }
    );

    expect(result.errors.length).toBeGreaterThan(0);
    expect(result.errors[0]).toMatch(/outside the current tool scope/);
    expect(result.state.secretAdminKey).toBe("protected");
  });
});
