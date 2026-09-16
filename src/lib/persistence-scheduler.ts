import type { ToolState } from "@/types/tool";

export interface ScheduleOptions {
  /** If true, flushes persistence immediately instead of debouncing. */
  immediate?: boolean;
  /** Custom debounce delay in milliseconds. Defaults to 250ms. */
  debounceMs?: number;
  /** Callback fired when a failure causes a rollback to the last known persisted state. */
  onRollback?: (restoredState: ToolState) => void;
  /** Callback fired on error. */
  onError?: (error: unknown) => void;
}

interface ToolStateTracker {
  toolId: string;
  currentGen: number;
  currentState: ToolState;
  lastPersistedGen: number;
  lastPersistedState: ToolState;
  timer: ReturnType<typeof setTimeout> | null;
  inFlightGen: number | null;
  inFlightEpoch: number | null;
  inFlightPromise: Promise<void> | null;
  hydrationEpoch: number;
  onRollback?: (restoredState: ToolState) => void;
  onError?: (error: unknown) => void;
}

/**
 * Centralized, monotonic state persistence scheduler.
 * 
 * Guarantees:
 * 1. Monotonic revision tracking per tool (currentGen increments on each change).
 * 2. Rapid keystrokes/toggles are debounced (250ms default) to avoid IPC write flooding.
 * 3. Stale rollback prevention: If save(gen 1) fails while gen 2 is in flight or applied,
 *    gen 1's failure CANNOT roll back gen 2's newer state.
 * 4. Hydration epoch concurrency: If tool is rehydrated with new state while an older save is in flight,
 *    the older save cannot overwrite or roll back the newly hydrated state.
 * 5. Explicit flushes (actions, tool switch, window unload, unmount) save immediately.
 * 6. Single source of persistence truth.
 */
export class PersistenceScheduler {
  private trackers = new Map<string, ToolStateTracker>();
  private defaultDebounceMs: number;

  constructor(defaultDebounceMs = 250) {
    this.defaultDebounceMs = defaultDebounceMs;
  }

  private getOrCreateTracker(toolId: string, initialState: ToolState = {}): ToolStateTracker {
    let tracker = this.trackers.get(toolId);
    if (!tracker) {
      tracker = {
        toolId,
        currentGen: 0,
        currentState: initialState,
        lastPersistedGen: 0,
        lastPersistedState: initialState,
        timer: null,
        inFlightGen: null,
        inFlightEpoch: null,
        inFlightPromise: null,
        hydrationEpoch: 0,
      };
      this.trackers.set(toolId, tracker);
    }
    return tracker;
  }

  /**
   * Initializes or refreshes the known persisted state for a tool (e.g. upon tool select / hydration).
   * Cancels pending timers, advances hydrationEpoch, and disarms old callbacks so older in-flight
   * saves cannot overwrite or rollback the new state.
   */
  public initToolState(toolId: string, state: ToolState): void {
    const tracker = this.getOrCreateTracker(toolId, state);
    if (tracker.timer) {
      clearTimeout(tracker.timer);
      tracker.timer = null;
    }
    tracker.hydrationEpoch += 1;
    tracker.currentGen = 0;
    tracker.lastPersistedGen = 0;
    tracker.currentState = state;
    tracker.lastPersistedState = state;
    tracker.onRollback = undefined;
    tracker.onError = undefined;
  }

  /**
   * Schedules a state update for persistence.
   */
  public async schedule(
    toolId: string,
    state: ToolState,
    saveFn: (toolId: string, state: ToolState) => Promise<void>,
    options?: ScheduleOptions,
  ): Promise<void> {
    const tracker = this.getOrCreateTracker(toolId, state);
    tracker.currentGen += 1;
    tracker.currentState = state;
    if (options?.onRollback) tracker.onRollback = options.onRollback;
    if (options?.onError) tracker.onError = options.onError;

    if (tracker.timer) {
      clearTimeout(tracker.timer);
      tracker.timer = null;
    }

    if (options?.immediate) {
      await this.flush(toolId, saveFn);
    } else {
      const delay = options?.debounceMs ?? this.defaultDebounceMs;
      tracker.timer = setTimeout(() => {
        tracker.timer = null;
        void this.flush(toolId, saveFn);
      }, delay);
    }
  }

  /**
   * Flushes any pending unpersisted state for the given tool, or all tools if omitted.
   */
  public async flush(
    toolId?: string,
    saveFn?: (toolId: string, state: ToolState) => Promise<void>,
  ): Promise<void> {
    if (!toolId) {
      const allPromises = Array.from(this.trackers.keys()).map((id) =>
        this.flush(id, saveFn),
      );
      await Promise.all(allPromises);
      return;
    }

    const tracker = this.trackers.get(toolId);
    if (!tracker) return;

    if (tracker.timer) {
      clearTimeout(tracker.timer);
      tracker.timer = null;
    }

    // If another save is already in-flight, await its completion
    if (tracker.inFlightPromise) {
      try {
        await tracker.inFlightPromise;
      } catch {
        // Handled inside inFlightPromise
      }
    }

    // If no newer changes have occurred beyond what was persisted, nothing to do
    if (tracker.currentGen <= tracker.lastPersistedGen) {
      return;
    }

    if (!saveFn) {
      return;
    }

    const saveGen = tracker.currentGen;
    const saveEpoch = tracker.hydrationEpoch;
    const stateToSave = tracker.currentState;
    tracker.inFlightGen = saveGen;
    tracker.inFlightEpoch = saveEpoch;

    let saveFailed = false;
    const promise = (async () => {
      try {
        await saveFn(toolId, stateToSave);
        // Stale epoch check: if rehydration occurred while save was in flight, ignore
        if (tracker.hydrationEpoch !== saveEpoch) {
          return;
        }
        if (saveGen > tracker.lastPersistedGen) {
          tracker.lastPersistedGen = saveGen;
          tracker.lastPersistedState = stateToSave;
        }
      } catch (err) {
        saveFailed = true;
        // Stale epoch check: DO NOT roll back if rehydrated with newer state
        if (tracker.hydrationEpoch !== saveEpoch) {
          return;
        }
        // Stale failure check: ONLY roll back if no newer state was scheduled in the meantime
        if (tracker.currentGen === saveGen) {
          tracker.currentState = tracker.lastPersistedState;
          tracker.onRollback?.(tracker.lastPersistedState);
          tracker.onError?.(err);
        }
      } finally {
        if (tracker.inFlightGen === saveGen && tracker.inFlightEpoch === saveEpoch) {
          tracker.inFlightGen = null;
          tracker.inFlightEpoch = null;
          tracker.inFlightPromise = null;
        }
      }
    })();

    tracker.inFlightPromise = promise;
    await promise;

    // If save succeeded and newer state was scheduled while in flight, flush again to reach steady state
    if (!saveFailed && tracker.currentGen > tracker.lastPersistedGen) {
      await this.flush(toolId, saveFn);
    }
  }

  public cancel(toolId: string): void {
    const tracker = this.trackers.get(toolId);
    if (tracker?.timer) {
      clearTimeout(tracker.timer);
      tracker.timer = null;
    }
  }

  public reset(toolId?: string): void {
    if (toolId) {
      this.cancel(toolId);
      this.trackers.delete(toolId);
    } else {
      for (const tracker of this.trackers.values()) {
        if (tracker.timer) clearTimeout(tracker.timer);
      }
      this.trackers.clear();
    }
  }

  public getPendingCount(toolId?: string): number {
    if (toolId) {
      const tracker = this.trackers.get(toolId);
      if (!tracker) return 0;
      return tracker.currentGen > tracker.lastPersistedGen ? 1 : 0;
    }
    let count = 0;
    for (const tracker of this.trackers.values()) {
      if (tracker.currentGen > tracker.lastPersistedGen) count++;
    }
    return count;
  }

  public getCurrentState(toolId: string): ToolState | undefined {
    return this.trackers.get(toolId)?.currentState;
  }

  public getLastPersistedState(toolId: string): ToolState | undefined {
    return this.trackers.get(toolId)?.lastPersistedState;
  }

  public getCurrentGen(toolId: string): number {
    return this.trackers.get(toolId)?.currentGen ?? 0;
  }
}

export const persistenceScheduler = new PersistenceScheduler();
