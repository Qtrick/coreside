import { describe, it, expect, vi, beforeEach } from "vitest";
import { PersistenceScheduler } from "./persistence-scheduler";

describe("PersistenceScheduler", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it("batches rapid typing into a single debounced persistence call", async () => {
    const scheduler = new PersistenceScheduler(100);
    const saveFn = vi.fn().mockResolvedValue(undefined);

    scheduler.schedule("tool-1", { text: "h" }, saveFn);
    scheduler.schedule("tool-1", { text: "he" }, saveFn);
    scheduler.schedule("tool-1", { text: "hel" }, saveFn);
    scheduler.schedule("tool-1", { text: "hell" }, saveFn);
    scheduler.schedule("tool-1", { text: "hello" }, saveFn);

    expect(saveFn).not.toHaveBeenCalled();
    expect(scheduler.getCurrentGen("tool-1")).toBe(5);

    // Fast-forward debounce timer
    await vi.advanceTimersByTimeAsync(100);

    expect(saveFn).toHaveBeenCalledTimes(1);
    expect(saveFn).toHaveBeenCalledWith("tool-1", { text: "hello" });
    expect(scheduler.getPendingCount("tool-1")).toBe(0);
  });

  it("supports immediate flush for explicit user actions", async () => {
    const scheduler = new PersistenceScheduler(250);
    const saveFn = vi.fn().mockResolvedValue(undefined);

    await scheduler.schedule("tool-1", { submitted: true }, saveFn, { immediate: true });

    expect(saveFn).toHaveBeenCalledTimes(1);
    expect(saveFn).toHaveBeenCalledWith("tool-1", { submitted: true });
    expect(scheduler.getPendingCount("tool-1")).toBe(0);
  });

  it("does not roll back newer state if an older in-flight save fails", async () => {
    const scheduler = new PersistenceScheduler(50);
    let rejectFirstSave: (err: Error) => void;
    const firstSavePromise = new Promise<void>((_, reject) => {
      rejectFirstSave = reject;
    });

    const saveFn = vi.fn().mockImplementation((_toolId, state) => {
      if (state.count === 1) {
        return firstSavePromise;
      }
      return Promise.resolve();
    });

    const onRollback = vi.fn();

    // Start saving count: 1 immediately
    scheduler.schedule("tool-1", { count: 1 }, saveFn, { immediate: true, onRollback });
    expect(saveFn).toHaveBeenCalledWith("tool-1", { count: 1 });

    // While count: 1 is in-flight, user increments to count: 2
    scheduler.schedule("tool-1", { count: 2 }, saveFn, { onRollback });

    // Now count: 1 fails
    rejectFirstSave!(new Error("Network error during save 1"));
    await vi.advanceTimersByTimeAsync(10);

    // CRITICAL: onRollback must NOT have been called with old state because currentGen > saveGen!
    expect(onRollback).not.toHaveBeenCalled();

    // Advance timer to allow count: 2 to flush
    await vi.advanceTimersByTimeAsync(100);
    expect(saveFn).toHaveBeenCalledWith("tool-1", { count: 2 });
    expect(scheduler.getCurrentState("tool-1")).toEqual({ count: 2 });
  });

  it("rolls back to last persisted state if save fails and no newer state arrived", async () => {
    const scheduler = new PersistenceScheduler(50);
    scheduler.initToolState("tool-1", { count: 0 });

    const saveFn = vi.fn().mockRejectedValue(new Error("Disk full"));
    const onRollback = vi.fn();

    scheduler.schedule("tool-1", { count: 1 }, saveFn, { immediate: true, onRollback });
    await vi.advanceTimersByTimeAsync(10);

    expect(onRollback).toHaveBeenCalledTimes(1);
    expect(onRollback).toHaveBeenCalledWith({ count: 0 });
    expect(scheduler.getCurrentState("tool-1")).toEqual({ count: 0 });
  });

  it("flushes pending state on manual flush during tool switch or close", async () => {
    const scheduler = new PersistenceScheduler(500);
    const saveFn = vi.fn().mockResolvedValue(undefined);

    scheduler.schedule("tool-1", { activeTab: "settings" }, saveFn);
    expect(saveFn).not.toHaveBeenCalled();

    // Flush explicitly before switching
    await scheduler.flush("tool-1", saveFn);

    expect(saveFn).toHaveBeenCalledTimes(1);
    expect(saveFn).toHaveBeenCalledWith("tool-1", { activeTab: "settings" });
    expect(scheduler.getPendingCount("tool-1")).toBe(0);
  });
});
