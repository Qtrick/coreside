import { describe, expect, it } from "vitest";
import {
  applyTextDelta,
  createTurnLiveState,
  pendingTurnId,
} from "@/lib/turn-registry";

describe("applyTextDelta", () => {
  it("applies sequential text checkpoints (Channel always sends full text)", () => {
    const state = createTurnLiveState("t1", "c1", 1);
    const next = applyTextDelta(state, {
      text: "Hello",
      delta: "Hello",
      sequence: 1,
    });
    expect(next.text).toBe("Hello");
    expect(next.lastSequence).toBe(1);

    const again = applyTextDelta(next, {
      text: "Hello world",
      delta: " world",
      sequence: 2,
    });
    expect(again.text).toBe("Hello world");
    expect(again.lastSequence).toBe(2);
  });

  it("uses text as checkpoint / full replace when delta is not next", () => {
    const state = {
      ...createTurnLiveState("t1", "c1", 1),
      text: "partial",
      lastSequence: 1,
    };
    // Gap: sequence 3 with no usable next-delta path → checkpoint replace.
    const next = applyTextDelta(state, {
      text: "full checkpoint",
      delta: " checkpoint",
      sequence: 3,
    });
    expect(next.text).toBe("full checkpoint");
    expect(next.lastSequence).toBe(3);
  });

  it("prefers text checkpoint over appending full-body delta (non-prefix replace)", () => {
    const state = {
      ...createTurnLiveState("t1", "c1", 1),
      text: "Hello",
      lastSequence: 1,
    };
    // Rust sends delta=full text when the new body does not start with previous.
    const next = applyTextDelta(state, {
      text: "Hi there",
      delta: "Hi there",
      sequence: 2,
    });
    expect(next.text).toBe("Hi there");
    expect(next.lastSequence).toBe(2);
  });

  it("appends delta-only events when sequence is next", () => {
    const state = {
      ...createTurnLiveState("t1", "c1", 1),
      text: "Hello",
      lastSequence: 1,
    };
    const next = applyTextDelta(state, {
      delta: " world",
      sequence: 2,
    });
    expect(next.text).toBe("Hello world");
    expect(next.lastSequence).toBe(2);
  });

  it("soft-rejects out-of-order / duplicate sequences", () => {
    const state = {
      ...createTurnLiveState("t1", "c1", 1),
      text: "Hello",
      lastSequence: 2,
    };
    const dup = applyTextDelta(state, {
      text: "Hello!",
      delta: "!",
      sequence: 2,
    });
    expect(dup).toBe(state);

    const older = applyTextDelta(state, {
      text: "Hel",
      delta: "Hel",
      sequence: 1,
    });
    expect(older).toBe(state);

    // Gap with only delta (no text) — soft reject.
    const gap = applyTextDelta(state, {
      delta: " world",
      sequence: 4,
    });
    expect(gap).toBe(state);
  });
});

describe("pendingTurnId", () => {
  it("scopes provisional ids to conversation", () => {
    expect(pendingTurnId("abc")).toBe("pending:abc");
  });
});
