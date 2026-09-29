import { describe, expect, it, vi } from "vitest";
import {
  catchUpConversationEvents,
  type ConversationCatchUpApi,
  type ConversationCatchUpEvent,
} from "./conversation-catch-up";

type Store = {
  events: Map<string, ConversationCatchUpEvent[]>;
  durable: Map<string, number>;
};

function makeApi(store: Store, opts?: {
  onAdvance?: (conversationId: string, sequence: number) => void;
  failAdvanceAt?: number;
}): ConversationCatchUpApi {
  return {
    async getConversationSyncCursor(conversationId) {
      return store.durable.get(conversationId) ?? 0;
    },
    async getConversationEvents({ conversationId, afterSequence, limit }) {
      const after = afterSequence ?? 0;
      const pageLimit = limit ?? 200;
      const all = store.events.get(conversationId) ?? [];
      return all
        .filter((e) => typeof e.sequence === "number" && e.sequence > after)
        .slice(0, pageLimit);
    },
    async advanceConversationSyncCursor(conversationId, sequence) {
      opts?.onAdvance?.(conversationId, sequence);
      if (opts?.failAdvanceAt !== undefined && sequence >= opts.failAdvanceAt) {
        throw new Error("persist failed");
      }
      const current = store.durable.get(conversationId) ?? 0;
      const next = Math.max(current, sequence);
      store.durable.set(conversationId, next);
      return next;
    },
  };
}

function seedEvents(
  store: Store,
  conversationId: string,
  count: number,
  opts?: { startSeq?: number; eventType?: string },
) {
  const start = opts?.startSeq ?? 1;
  const eventType = opts?.eventType ?? "message.created";
  const list: ConversationCatchUpEvent[] = [];
  for (let i = 0; i < count; i++) {
    list.push({ sequence: start + i, eventType });
  }
  store.events.set(conversationId, list);
}

describe("catchUpConversationEvents", () => {
  const noYield = async () => {};

  it("fresh subscription with cursor 0 and no events is a no-op", async () => {
    const store: Store = { events: new Map(), durable: new Map() };
    const memoryCursor = new Map<string, number>();
    const onTransactionApplied = vi.fn();
    const advanceSpy = vi.fn();

    await catchUpConversationEvents({
      conversationId: "c-fresh",
      memoryCursor,
      api: makeApi(store, { onAdvance: advanceSpy }),
      isActive: () => true,
      onTransactionApplied,
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-fresh")).toBeUndefined();
    expect(store.durable.get("c-fresh")).toBeUndefined();
    expect(advanceSpy).not.toHaveBeenCalled();
    expect(onTransactionApplied).not.toHaveBeenCalled();
  });

  it.each([
    { count: 1, label: "1 event" },
    { count: 199, label: "199 events (under page)" },
    { count: 200, label: "200 events (exact page)" },
    { count: 201, label: "201 events (crosses page)" },
  ])("advances through $label", async ({ count }) => {
    const store: Store = { events: new Map(), durable: new Map([["c1", 0]]) };
    seedEvents(store, "c1", count);
    const memoryCursor = new Map<string, number>();
    const advances: number[] = [];

    await catchUpConversationEvents({
      conversationId: "c1",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: (_id, seq) => advances.push(seq),
      }),
      isActive: () => true,
      onTransactionApplied: () => {},
      pageLimit: 200,
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c1")).toBe(count);
    expect(store.durable.get("c1")).toBe(count);
    if (count <= 200) {
      expect(advances).toEqual([count]);
    } else {
      expect(advances).toEqual([200, 201]);
    }
  });

  it("pages through thousands of events across many pages", async () => {
    const total = 2500;
    const store: Store = { events: new Map(), durable: new Map([["c-big", 0]]) };
    seedEvents(store, "c-big", total);
    const memoryCursor = new Map<string, number>();
    const advances: number[] = [];

    await catchUpConversationEvents({
      conversationId: "c-big",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: (_id, seq) => advances.push(seq),
      }),
      isActive: () => true,
      onTransactionApplied: () => {},
      pageLimit: 200,
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-big")).toBe(total);
    expect(store.durable.get("c-big")).toBe(total);
    expect(advances.length).toBe(Math.ceil(total / 200));
    expect(advances[0]).toBe(200);
    expect(advances[advances.length - 1]).toBe(total);
  });

  it("reload clears memory cursor but resumes from durable watermark", async () => {
    const store: Store = {
      events: new Map(),
      durable: new Map([["c-reload", 150]]),
    };
    seedEvents(store, "c-reload", 50, { startSeq: 151 });
    // Simulate process restart: empty in-memory map.
    const memoryCursor = new Map<string, number>();

    await catchUpConversationEvents({
      conversationId: "c-reload",
      memoryCursor,
      api: makeApi(store),
      isActive: () => true,
      onTransactionApplied: () => {},
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-reload")).toBe(200);
    expect(store.durable.get("c-reload")).toBe(200);
  });

  it("ignores duplicate sequences already at or behind the cursor", async () => {
    const store: Store = {
      events: new Map([
        [
          "c-dup",
          [
            { sequence: 1, eventType: "message.created" },
            { sequence: 2, eventType: "message.created" },
            { sequence: 2, eventType: "message.created" },
            { sequence: 3, eventType: "message.created" },
          ],
        ],
      ]),
      durable: new Map([["c-dup", 1]]),
    };
    const memoryCursor = new Map<string, number>([["c-dup", 1]]);
    const advances: number[] = [];

    await catchUpConversationEvents({
      conversationId: "c-dup",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: (_id, seq) => advances.push(seq),
      }),
      isActive: () => true,
      onTransactionApplied: () => {},
      yieldBetweenPages: noYield,
    });

    expect(advances).toEqual([3]);
    expect(memoryCursor.get("c-dup")).toBe(3);
  });

  it("skips malformed events without advancing past good ones incorrectly", async () => {
    const store: Store = {
      events: new Map([
        [
          "c-mal",
          [
            { sequence: 1, eventType: "message.created" },
            { eventType: "message.created" } as ConversationCatchUpEvent,
            { sequence: Number.NaN, eventType: "message.created" },
            { sequence: 2, eventType: "message.created" },
          ],
        ],
      ]),
      durable: new Map([["c-mal", 0]]),
    };
    const memoryCursor = new Map<string, number>();

    await catchUpConversationEvents({
      conversationId: "c-mal",
      memoryCursor,
      api: {
        ...makeApi(store),
        // Return the mixed page as-is (including malformed) so catch-up must filter.
        async getConversationEvents() {
          return store.events.get("c-mal")!;
        },
      },
      isActive: () => true,
      onTransactionApplied: () => {},
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-mal")).toBe(2);
    expect(store.durable.get("c-mal")).toBe(2);
  });

  it("stops without tight-looping when a page contains only malformed events", async () => {
    const store: Store = {
      events: new Map(),
      durable: new Map([["c-bad-page", 0]]),
    };
    let fetchCount = 0;
    const api: ConversationCatchUpApi = {
      async getConversationSyncCursor() {
        return 0;
      },
      async getConversationEvents() {
        fetchCount += 1;
        // Full page of garbage — must not re-fetch forever.
        return Array.from({ length: 200 }, () => ({
          eventType: "message.created",
        })) as ConversationCatchUpEvent[];
      },
      async advanceConversationSyncCursor() {
        throw new Error("must not advance on malformed-only page");
      },
    };
    const memoryCursor = new Map<string, number>();

    await catchUpConversationEvents({
      conversationId: "c-bad-page",
      memoryCursor,
      api,
      isActive: () => true,
      onTransactionApplied: () => {},
      pageLimit: 200,
      yieldBetweenPages: noYield,
    });

    expect(fetchCount).toBe(1);
    expect(memoryCursor.get("c-bad-page")).toBeUndefined();
    expect(store.durable.get("c-bad-page")).toBe(0);
  });

  it("does not advance memory cursor past unpersisted work when persist fails", async () => {
    const store: Store = { events: new Map(), durable: new Map([["c-fail", 0]]) };
    seedEvents(store, "c-fail", 50);
    const memoryCursor = new Map<string, number>();

    await catchUpConversationEvents({
      conversationId: "c-fail",
      memoryCursor,
      api: makeApi(store, { failAdvanceAt: 1 }),
      isActive: () => true,
      onTransactionApplied: () => {},
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-fail")).toBeUndefined();
    expect(store.durable.get("c-fail")).toBe(0);
  });

  it("stops mid-catch-up when isActive becomes false (conversation switch)", async () => {
    const store: Store = { events: new Map(), durable: new Map([["c-switch", 0]]) };
    seedEvents(store, "c-switch", 500);
    const memoryCursor = new Map<string, number>();
    let pages = 0;
    let active = true;

    await catchUpConversationEvents({
      conversationId: "c-switch",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: () => {
          pages += 1;
          if (pages >= 1) active = false;
        },
      }),
      isActive: () => active,
      onTransactionApplied: () => {},
      pageLimit: 200,
      yieldBetweenPages: noYield,
    });

    expect(pages).toBe(1);
    expect(memoryCursor.get("c-switch")).toBe(200);
    expect(store.durable.get("c-switch")).toBe(200);
  });

  it("invokes onTransactionApplied for surface.transaction_applied events", async () => {
    const store: Store = {
      events: new Map([
        [
          "c-txn",
          [
            { sequence: 1, eventType: "message.created" },
            { sequence: 2, eventType: "surface.transaction_applied" },
            { sequence: 3, eventType: "message.created" },
          ],
        ],
      ]),
      durable: new Map([["c-txn", 0]]),
    };
    const memoryCursor = new Map<string, number>();
    const onTransactionApplied = vi.fn();

    await catchUpConversationEvents({
      conversationId: "c-txn",
      memoryCursor,
      api: makeApi(store),
      isActive: () => true,
      onTransactionApplied,
      yieldBetweenPages: noYield,
    });

    expect(onTransactionApplied).toHaveBeenCalledTimes(1);
    expect(memoryCursor.get("c-txn")).toBe(3);
  });

  it("does not advance cursor when onTransactionApplied fails", async () => {
    const store: Store = {
      events: new Map([
        [
          "c-apply-fail",
          [
            { sequence: 1, eventType: "message.created" },
            { sequence: 2, eventType: "surface.transaction_applied" },
            { sequence: 3, eventType: "message.created" },
          ],
        ],
      ]),
      durable: new Map([["c-apply-fail", 0]]),
    };
    const memoryCursor = new Map<string, number>();
    const advances: number[] = [];

    await catchUpConversationEvents({
      conversationId: "c-apply-fail",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: (_id, seq) => advances.push(seq),
      }),
      isActive: () => true,
      onTransactionApplied: () => {
        throw new Error("reload failed");
      },
      yieldBetweenPages: noYield,
    });

    expect(advances).toEqual([]);
    expect(memoryCursor.get("c-apply-fail")).toBeUndefined();
    expect(store.durable.get("c-apply-fail")).toBe(0);
  });

  it("overlapping catch-up leaves cursor unmoved when both hit failed reload", async () => {
    const store: Store = {
      events: new Map([
        [
          "c-overlap-fail",
          [
            { sequence: 1, eventType: "message.created" },
            { sequence: 2, eventType: "surface.transaction_applied" },
            { sequence: 3, eventType: "message.created" },
          ],
        ],
      ]),
      durable: new Map([["c-overlap-fail", 0]]),
    };
    const memoryCursor = new Map<string, number>();
    let releaseFirstReload: (() => void) | null = null;
    const firstReloadGate = new Promise<void>((resolve) => {
      releaseFirstReload = resolve;
    });
    let firstEnteredReload = false;
    const advances: number[] = [];

    const first = catchUpConversationEvents({
      conversationId: "c-overlap-fail",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: (_id, seq) => advances.push(seq),
      }),
      isActive: () => true,
      onTransactionApplied: async () => {
        firstEnteredReload = true;
        await firstReloadGate;
        throw new Error("first reload failed");
      },
      yieldBetweenPages: noYield,
    });

    await vi.waitFor(() => {
      expect(firstEnteredReload).toBe(true);
    });
    expect(memoryCursor.get("c-overlap-fail")).toBeUndefined();
    expect(store.durable.get("c-overlap-fail")).toBe(0);

    // Overlapping catch-up while the first is still mid-reload also fails closed.
    await catchUpConversationEvents({
      conversationId: "c-overlap-fail",
      memoryCursor,
      api: makeApi(store, {
        onAdvance: (_id, seq) => advances.push(seq),
      }),
      isActive: () => true,
      onTransactionApplied: () => {
        throw new Error("second reload failed");
      },
      yieldBetweenPages: noYield,
    });

    expect(advances).toEqual([]);
    expect(memoryCursor.get("c-overlap-fail")).toBeUndefined();
    expect(store.durable.get("c-overlap-fail")).toBe(0);

    releaseFirstReload!();
    await first;

    expect(advances).toEqual([]);
    expect(memoryCursor.get("c-overlap-fail")).toBeUndefined();
    expect(store.durable.get("c-overlap-fail")).toBe(0);
  });

  it("overlapping catch-up resumes from shared watermark without regressing", async () => {
    const total = 400;
    const store: Store = {
      events: new Map(),
      durable: new Map([["c-overlap", 0]]),
    };
    seedEvents(store, "c-overlap", total);
    const memoryCursor = new Map<string, number>();
    let releaseYield: (() => void) | null = null;
    const yieldGate = new Promise<void>((resolve) => {
      releaseYield = resolve;
    });
    let yields = 0;

    const first = catchUpConversationEvents({
      conversationId: "c-overlap",
      memoryCursor,
      api: makeApi(store),
      isActive: () => true,
      onTransactionApplied: () => {},
      pageLimit: 200,
      yieldBetweenPages: async () => {
        yields += 1;
        if (yields === 1) {
          await yieldGate;
        }
      },
    });

    await vi.waitFor(() => {
      expect(memoryCursor.get("c-overlap")).toBe(200);
    });
    expect(store.durable.get("c-overlap")).toBe(200);

    await catchUpConversationEvents({
      conversationId: "c-overlap",
      memoryCursor,
      api: makeApi(store),
      isActive: () => true,
      onTransactionApplied: () => {},
      pageLimit: 200,
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-overlap")).toBe(total);
    expect(store.durable.get("c-overlap")).toBe(total);

    releaseYield!();
    await first;

    expect(memoryCursor.get("c-overlap")).toBe(total);
    expect(store.durable.get("c-overlap")).toBe(total);
  });

  it("inFlight single-flight coalesces concurrent catch-ups into one run", async () => {
    const store: Store = {
      events: new Map(),
      durable: new Map([["c-sf", 0]]),
    };
    seedEvents(store, "c-sf", 50, {
      eventType: "surface.transaction_applied",
    });
    const memoryCursor = new Map<string, number>();
    const inFlight = new Map<string, Promise<void>>();
    let releaseReload: (() => void) | null = null;
    const reloadGate = new Promise<void>((resolve) => {
      releaseReload = resolve;
    });
    let reloadCalls = 0;
    let fetchCalls = 0;

    const api = makeApi(store);
    const wrappedApi: ConversationCatchUpApi = {
      ...api,
      async getConversationEvents(args) {
        fetchCalls += 1;
        return api.getConversationEvents(args);
      },
    };

    const shared = {
      conversationId: "c-sf",
      memoryCursor,
      inFlight,
      api: wrappedApi,
      isActive: () => true,
      onTransactionApplied: async () => {
        reloadCalls += 1;
        await reloadGate;
      },
      yieldBetweenPages: noYield,
    };

    const a = catchUpConversationEvents(shared);
    await vi.waitFor(() => {
      expect(reloadCalls).toBe(1);
    });
    const b = catchUpConversationEvents(shared);
    expect(b).toBe(a);
    expect(reloadCalls).toBe(1);
    expect(fetchCalls).toBe(1);

    releaseReload!();
    await Promise.all([a, b]);

    expect(reloadCalls).toBe(1);
    expect(memoryCursor.get("c-sf")).toBe(50);
    expect(inFlight.size).toBe(0);
  });

  it("keeps forged/wrong conversation cursors isolated", async () => {
    const store: Store = {
      events: new Map(),
      durable: new Map([
        ["c-a", 0],
        ["c-b", 0],
      ]),
    };
    seedEvents(store, "c-a", 10);
    seedEvents(store, "c-b", 3);
    // Attacker-shaped payload for c-a claiming c-b sequences — ignored because
    // getConversationEvents is scoped by conversationId.
    store.events.get("c-a")!.push(
      { sequence: 100, eventType: "message.created" },
    );
    const memoryCursor = new Map<string, number>();

    await catchUpConversationEvents({
      conversationId: "c-a",
      memoryCursor,
      api: makeApi(store),
      isActive: () => true,
      onTransactionApplied: () => {},
      yieldBetweenPages: noYield,
    });
    await catchUpConversationEvents({
      conversationId: "c-b",
      memoryCursor,
      api: makeApi(store),
      isActive: () => true,
      onTransactionApplied: () => {},
      yieldBetweenPages: noYield,
    });

    expect(memoryCursor.get("c-a")).toBe(100);
    expect(memoryCursor.get("c-b")).toBe(3);
    expect(store.durable.get("c-a")).toBe(100);
    expect(store.durable.get("c-b")).toBe(3);
  });
});
