/**
 * Durable conversation event catch-up — Partial Update reconnect semantics
 * without Durable Object state: SQLite event log + monotonic per-window watermark.
 *
 * The TypeScript API takes only `conversationId`. The authoritative client /
 * window identity is bound in Rust from `WebviewWindow::label()` and cannot be
 * forged from the frontend. Each webview therefore has an independent cursor
 * for the same conversation.
 */

export type ConversationCatchUpEvent = {
  sequence: number;
  eventType: string;
};

export type ConversationCatchUpApi = {
  getConversationSyncCursor: (conversationId: string) => Promise<number>;
  getConversationEvents: (args: {
    conversationId: string;
    afterSequence?: number | null;
    limit?: number | null;
  }) => Promise<ConversationCatchUpEvent[]>;
  advanceConversationSyncCursor: (
    conversationId: string,
    sequence: number,
  ) => Promise<number>;
};

export type CatchUpOptions = {
  conversationId: string;
  /** In-memory cursor map (optional cache; durable SQLite is authoritative). */
  memoryCursor: Map<string, number>;
  api: ConversationCatchUpApi;
  /** Still catching up this conversation? */
  isActive: () => boolean;
  onTransactionApplied: () => void | Promise<void>;
  pageLimit?: number;
  /** Injected yield between pages (tests can no-op). */
  yieldBetweenPages?: () => Promise<void>;
  /**
   * Optional per-conversation single-flight map. Concurrent catch-ups for the
   * same id share one run (subscribe success + error paths, remount thrash).
   */
  inFlight?: Map<string, Promise<void>>;
};

/**
 * Resume from the durable watermark, apply bounded pages, persist after each
 * successful page (including any transaction reload). Never advances past
 * unapplied work. Duplicate sequences are ignored. Stops when the conversation
 * is no longer active.
 *
 * Not `async`: when `inFlight` is set we must return the same Promise object
 * to coalesced callers (an async wrapper would allocate a new Promise each time).
 */
export function catchUpConversationEvents(
  options: CatchUpOptions,
): Promise<void> {
  const flights = options.inFlight;
  if (!flights) {
    return runCatchUpPages(options);
  }
  const key = options.conversationId;
  const existing = flights.get(key);
  if (existing) return existing;
  const run = runCatchUpPages(options).finally(() => {
    if (flights.get(key) === run) flights.delete(key);
  });
  flights.set(key, run);
  return run;
}

async function runCatchUpPages(options: CatchUpOptions): Promise<void> {
  const {
    conversationId,
    memoryCursor,
    api,
    isActive,
    onTransactionApplied,
    pageLimit = 200,
    yieldBetweenPages = () =>
      new Promise<void>((resolve) => {
        setTimeout(resolve, 0);
      }),
  } = options;

  let after = memoryCursor.get(conversationId) ?? 0;
  try {
    const durable = await api.getConversationSyncCursor(conversationId);
    if (typeof durable === "number" && Number.isFinite(durable) && durable > after) {
      after = Math.max(0, Math.floor(durable));
      memoryCursor.set(conversationId, after);
    }
  } catch {
    // Best-effort hydrate.
  }

  for (;;) {
    if (!isActive()) break;

    let events: ConversationCatchUpEvent[];
    try {
      events = await api.getConversationEvents({
        conversationId,
        afterSequence: after,
        limit: pageLimit,
      });
    } catch {
      break;
    }
    if (!Array.isArray(events) || events.length === 0) break;

    let maxSeq = after;
    let pageNeedsReload = false;
    for (const ev of events) {
      if (typeof ev?.sequence !== "number" || !Number.isFinite(ev.sequence)) {
        continue;
      }
      if (ev.sequence <= after) continue;
      if (ev.sequence > maxSeq) maxSeq = ev.sequence;
      if (ev.eventType === "surface.transaction_applied") {
        pageNeedsReload = true;
      }
    }

    // Apply side effects before persisting the watermark — a failed reload
    // must leave the durable cursor behind so the next catch-up retries.
    if (pageNeedsReload) {
      if (!isActive()) break;
      try {
        await onTransactionApplied();
      } catch {
        break;
      }
    }

    if (maxSeq > after) {
      try {
        const stored = await api.advanceConversationSyncCursor(
          conversationId,
          maxSeq,
        );
        const next =
          typeof stored === "number" && Number.isFinite(stored)
            ? Math.max(maxSeq, Math.floor(stored))
            : maxSeq;
        memoryCursor.set(conversationId, next);
        after = next;
      } catch {
        break;
      }
    } else {
      break;
    }

    if (events.length < pageLimit) break;
    await yieldBetweenPages();
  }
}
