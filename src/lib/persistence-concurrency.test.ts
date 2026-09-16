import { describe, expect, it, vi } from "vitest";
import { PersistenceScheduler } from "./persistence-scheduler";
import type { ToolState } from "@/types/tool";

describe("PersistenceScheduler concurrency & hydration races", () => {
  it("reconciles disk state when an in-flight save completes after hydration", async () => {
    const scheduler = new PersistenceScheduler(50);
    const toolId = "test-tool-1";

    const diskWrites: Array<{ toolId: string; state: ToolState }> = [];
    let resolveFirstSave!: () => void;
    const firstSavePromise = new Promise<void>((r) => {
      resolveFirstSave = r;
    });

    let saveCallCount = 0;
    const saveFn = vi.fn(async (tid: string, state: ToolState) => {
      saveCallCount++;
      if (saveCallCount === 1) {
        // First save is delayed
        await firstSavePromise;
      }
      diskWrites.push({ toolId: tid, state: structuredClone(state) });
    });

    // 1. Initial state A begins saving
    const stateA = { counter: 1, text: "draft from session A" };
    void scheduler.schedule(toolId, stateA, saveFn, { immediate: true });

    // Wait microtask so first saveFn has started and is awaiting firstSavePromise
    await new Promise((r) => setTimeout(r, 10));
    expect(saveCallCount).toBe(1);

    // 2. Hydration occurs: tool is rehydrated with state B (e.g. user switched chat or fetched server state)
    const stateB = { counter: 100, text: "authoritative hydrated state B" };
    scheduler.initToolState(toolId, stateB);

    // 3. Old save A finishes writing to disk
    resolveFirstSave();

    // Give scheduler time to complete and reconcile
    await new Promise((r) => setTimeout(r, 50));

    // The first write was stateA (before hydration)
    expect(diskWrites[0]?.state).toEqual(stateA);

    // CRITICAL REQUIREMENT: State A must NOT overwrite State B permanently on disk!
    // The scheduler must have reconciled by flushing State B to disk
    expect(diskWrites.length).toBeGreaterThanOrEqual(2);
    const lastDiskWrite = diskWrites[diskWrites.length - 1];
    expect(lastDiskWrite?.state).toEqual(stateB);
    expect(scheduler.getLastPersistedState(toolId)).toEqual(stateB);
  });

  it("handles rapid typing without dropping intermediate states or stale rollbacks", async () => {
    const scheduler = new PersistenceScheduler(20);
    const toolId = "rapid-tool";
    const diskWrites: ToolState[] = [];

    const saveFn = vi.fn(async (_tid: string, state: ToolState) => {
      await new Promise((r) => setTimeout(r, 10));
      diskWrites.push(structuredClone(state));
    });

    scheduler.initToolState(toolId, { text: "" });

    // Rapid edits
    for (let i = 1; i <= 5; i++) {
      void scheduler.schedule(toolId, { text: `step-${i}` }, saveFn);
      await new Promise((r) => setTimeout(r, 5));
    }

    // Explicit flush at end
    await scheduler.flush(toolId, saveFn);

    expect(diskWrites.length).toBeGreaterThan(0);
    const finalWrite = diskWrites[diskWrites.length - 1];
    expect(finalWrite).toEqual({ text: "step-5" });
  });

  it("does not roll back when an old save fails after newer state is scheduled", async () => {
    const scheduler = new PersistenceScheduler(50);
    const toolId = "fail-tool";
    let shouldFail = true;
    const rollbacks: ToolState[] = [];

    const saveFn = vi.fn(async () => {
      if (shouldFail) {
        shouldFail = false;
        throw new Error("Network transient error");
      }
    });

    scheduler.initToolState(toolId, { val: 0 });

    // Schedule save 1 (will fail)
    void scheduler.schedule(toolId, { val: 1 }, saveFn, {
      immediate: true,
      onRollback: (restored) => rollbacks.push(restored),
    });

    // Before save 1 settles, user makes change 2
    void scheduler.schedule(toolId, { val: 2 }, saveFn);

    await scheduler.flush(toolId, saveFn);

    // Save 1 failed, but newer state was scheduled, so rollback was NOT applied to stomp val 2
    expect(rollbacks.length).toBe(0);
    expect(scheduler.getCurrentState(toolId)).toEqual({ val: 2 });
  });
});
