import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useAppStore,
  _resetBootstrapInFlightForTests,
  _resetConversationSyncForTests,
  _ensureConversationSyncSubscriptionForTests,
  _getConversationSyncStateForTests,
} from "./app-store";

const mockGetBootstrapStatus = vi.fn();
const mockGetAppInfo = vi.fn();
const mockGetSettings = vi.fn();
const mockListConversations = vi.fn();
const mockGetAiStatus = vi.fn();
const mockListTools = vi.fn();
const mockListProjects = vi.fn();
const mockGetModelCatalog = vi.fn();
const mockGetMessages = vi.fn();
const mockSubscribeConversationSync = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    getBootstrapStatus: () => mockGetBootstrapStatus(),
    getAppInfo: () => mockGetAppInfo(),
    getSettings: () => mockGetSettings(),
    listConversations: () => mockListConversations(),
    getAiStatus: () => mockGetAiStatus(),
    listTools: () => mockListTools(),
    listProjects: () => mockListProjects(),
    getModelCatalog: () => mockGetModelCatalog(),
    getMessages: (...args: unknown[]) => mockGetMessages(...args),
    subscribeConversationSync: (...args: unknown[]) => mockSubscribeConversationSync(...args),
    setWorkspaceAppearance: vi.fn(),
    applyAppearanceBundle: vi.fn(),
    getSurface: vi.fn(async () => null),
    windowOrchestratorRestore: vi.fn(async () => undefined),
  },
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(message: string, code = "error") {
      super(message);
      this.code = code;
    }
  },
  listenAgentTurn: vi.fn(async () => () => undefined),
  shouldShowAppConflict: () => false,
  shouldApplyAgentTurnSync: () => false,
}));

describe("app-store bootstrap concurrency, idempotency, and decoupled hydration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    _resetBootstrapInFlightForTests();
    _resetConversationSyncForTests();

    // Reset store state between tests
    useAppStore.setState({
      bootstrapped: false,
      bootError: null,
      bootstrapStatus: null,
      appInfo: null,
      aiStatus: null,
      modelCatalog: null,
      conversations: [],
      activeConversationId: null,
      tools: [],
      projects: [],
      messages: [],
    });

    mockGetBootstrapStatus.mockResolvedValue({ status: "ready" });
    mockGetAppInfo.mockResolvedValue({ name: "Coreside", version: "0.1.0" });
    mockGetSettings.mockResolvedValue({
      theme: "dark",
      interfaceTransparency: 20,
    });
    mockListConversations.mockResolvedValue([
      { id: "conv-1", title: "First Conversation", createdAt: "2026-10-28T00:00:00Z" },
    ]);
    mockGetAiStatus.mockResolvedValue({
      provider: "openai",
      model: "gpt-4o",
      disclosure: { showProviderCatalog: false },
    });
    mockListTools.mockResolvedValue([]);
    mockListProjects.mockResolvedValue([]);
    mockGetMessages.mockResolvedValue([]);
    mockSubscribeConversationSync.mockResolvedValue(() => undefined);
  });

  it("coalesces concurrent bootstrap calls into a single in-flight promise", async () => {
    const store = useAppStore.getState();

    // Invoke bootstrap three times concurrently (simulating React 18 StrictMode mount + focus triggers)
    const [p1, p2, p3] = [store.bootstrap(), store.bootstrap(), store.bootstrap()];

    // All concurrent invocations must return the exact same in-flight promise instance
    expect(p1).toBe(p2);
    expect(p2).toBe(p3);

    await Promise.all([p1, p2, p3]);

    // Critical shell API calls should each be invoked exactly once, never duplicated
    expect(mockGetBootstrapStatus).toHaveBeenCalledTimes(1);
    expect(mockGetAppInfo).toHaveBeenCalledTimes(1);
    expect(mockGetSettings).toHaveBeenCalledTimes(1);
    expect(mockListConversations).toHaveBeenCalledTimes(1);

    expect(useAppStore.getState().bootstrapped).toBe(true);
    expect(useAppStore.getState().conversations).toHaveLength(1);
  });

  it("clears in-flight promise on completion allowing sequential bootstrap calls", async () => {
    const store = useAppStore.getState();

    const p1 = store.bootstrap();
    await p1;

    expect(mockGetBootstrapStatus).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().bootstrapped).toBe(true);

    // After p1 settles, a second sequential bootstrap can run without retaining a stale lock
    const p2 = store.bootstrap();
    expect(p2).not.toBe(p1);
    await p2;

    expect(mockGetBootstrapStatus).toHaveBeenCalledTimes(2);
    expect(useAppStore.getState().bootstrapped).toBe(true);
  });

  it("navigates to active chat immediately without blocking on secondary hydration", async () => {
    let resolveSecondary: () => void = () => {};
    const slowAiStatus = new Promise((resolve) => {
      resolveSecondary = () =>
        resolve({
          provider: "openai",
          model: "gpt-4o",
          disclosure: { showProviderCatalog: false },
        });
    });
    mockGetAiStatus.mockReturnValueOnce(slowAiStatus);

    const store = useAppStore.getState();
    const bootPromise = store.bootstrap();

    // Wait microtask tick for Phase 1 to settle
    await new Promise((r) => setTimeout(r, 10));

    // Active chat navigation should run immediately after Phase 1 shell hydration
    expect(mockGetMessages).toHaveBeenCalledWith("conv-1");
    expect(useAppStore.getState().activeConversationId).toBe("conv-1");

    // Secondary state is still pending while chat is already active
    expect(useAppStore.getState().aiStatus).toBeNull();

    // Now resolve secondary hydration and verify final store state
    resolveSecondary();
    await bootPromise;

    expect(useAppStore.getState().bootstrapped).toBe(true);
    expect(useAppStore.getState().aiStatus).toEqual({
      provider: "openai",
      model: "gpt-4o",
      disclosure: { showProviderCatalog: false },
    });
  });

  it("commits tools and projects without waiting for delayed getModelCatalog", async () => {
    mockGetAiStatus.mockResolvedValue({
      provider: "openai",
      model: "gpt-4o",
      disclosure: { showProviderCatalog: true },
    });
    mockListTools.mockResolvedValue([
      { id: "tool-1", name: "Calculator", description: "calc", version: "1.0.0" },
    ]);
    mockListProjects.mockResolvedValue([
      { id: "proj-1", name: "Project Alpha", createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z" },
    ]);

    let resolveCatalog: (val: unknown) => void = () => {};
    const slowCatalog = new Promise((resolve) => {
      resolveCatalog = resolve;
    });
    mockGetModelCatalog.mockReturnValueOnce(slowCatalog);

    const store = useAppStore.getState();
    const bootPromise = store.bootstrap();

    // Wait microtask ticks for Phase 1 & initial secondary hydration to run
    await new Promise((r) => setTimeout(r, 20));

    // tools and projects must already be committed to the store even though modelCatalog is still pending
    expect(useAppStore.getState().tools).toHaveLength(1);
    expect(useAppStore.getState().projects).toHaveLength(1);
    expect(useAppStore.getState().modelCatalog).toBeNull();

    // Settle model catalog
    resolveCatalog({
      selected: "gpt-4o",
      models: [{ id: "gpt-4o", name: "GPT-4o" }],
    });
    await bootPromise;

    expect(useAppStore.getState().modelCatalog).toEqual({
      selected: "gpt-4o",
      models: [{ id: "gpt-4o", name: "GPT-4o" }],
    });
  });

  it("halts early on recoveryRequired status without navigating to chat or loading secondary data", async () => {
    mockGetBootstrapStatus.mockResolvedValueOnce({
      status: "recoveryRequired",
      reason: "maintenance_journal",
    });

    const store = useAppStore.getState();
    await store.bootstrap();

    expect(useAppStore.getState().bootstrapped).toBe(true);
    expect(useAppStore.getState().bootstrapStatus).toEqual({
      status: "recoveryRequired",
      reason: "maintenance_journal",
    });

    // Shell settings, conversations, and secondary APIs must NOT be called in recovery mode
    expect(mockGetSettings).not.toHaveBeenCalled();
    expect(mockListConversations).not.toHaveBeenCalled();
    expect(mockGetAiStatus).not.toHaveBeenCalled();
    expect(mockGetMessages).not.toHaveBeenCalled();
  });

  it("handles bootstrap rejection cleanly and clears in-flight guard for subsequent retry", async () => {
    mockGetSettings.mockRejectedValueOnce(new Error("Database disk I/O error"));

    const store = useAppStore.getState();
    await store.bootstrap();

    expect(useAppStore.getState().bootstrapped).toBe(true);
    expect(useAppStore.getState().bootError).toBe("Database disk I/O error");

    // After failure, in-flight guard must be released so user retry can proceed
    mockGetSettings.mockResolvedValueOnce({ theme: "light" });
    await store.bootstrap();

    expect(mockGetSettings).toHaveBeenCalledTimes(2);
    expect(useAppStore.getState().bootError).toBeNull();
  });

  it("coalesces concurrent navigateToChat subscriptions for the same conversation without leaking unlisteners", async () => {
    let unlistenCalled = 0;
    const stopFn = vi.fn(() => {
      unlistenCalled += 1;
    });

    let resolveSub: (val: () => void) => void = () => {};
    mockSubscribeConversationSync.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSub = () => resolve(stopFn);
        }),
    );

    const store = useAppStore.getState();

    // Two concurrent calls for conv-1 while subscription is pending
    const [p1, p2] = [store.navigateToChat("conv-1"), store.navigateToChat("conv-1")];

    // Exactly one subscribe call should be initiated (coalesced)
    expect(mockSubscribeConversationSync).toHaveBeenCalledTimes(1);

    resolveSub(stopFn);
    await Promise.all([p1, p2]);

    expect(unlistenCalled).toBe(0);
  });

  it("disposes stale subscriptions and ensures only latest generation becomes active during rapid A -> B -> A navigation", async () => {
    const stops: Record<string, () => void> = {
      "sub-1": vi.fn(),
      "sub-2": vi.fn(),
      "sub-3": vi.fn(),
    };
    const resolvers: Record<string, () => void> = {};

    let callCount = 0;
    mockSubscribeConversationSync.mockImplementation(() => {
      callCount += 1;
      const key = `sub-${callCount}`;
      return new Promise<() => void>((resolve) => {
        resolvers[key] = () => resolve(stops[key]);
      });
    });

    const store = useAppStore.getState();

    // User navigates A -> B -> A
    const navA1 = store.navigateToChat("conv-1");
    const navB = store.navigateToChat("conv-2");
    const navA2 = store.navigateToChat("conv-1");

    // Settle out-of-order: first conv-1 settles, then conv-2 settles, then second conv-1 settles
    resolvers["sub-1"]();
    await navA1;

    // First subscription was superseded by navB and navA2, so its returned stopFn must be called immediately
    expect(stops["sub-1"]).toHaveBeenCalledTimes(1);

    resolvers["sub-2"]();
    await navB;
    // B was also superseded by second A, so its returned stopFn must be called immediately
    expect(stops["sub-2"]).toHaveBeenCalledTimes(1);

    resolvers["sub-3"]();
    await navA2;
    // Third subscription (second A) is current and active, so its stopFn must NOT be called
    expect(stops["sub-3"]).not.toHaveBeenCalled();
    expect(useAppStore.getState().activeConversationId).toBe("conv-1");
  });

  it("allows subscription retry when a previous attempt fails and clears sync state", async () => {
    mockSubscribeConversationSync.mockRejectedValueOnce(new Error("Channel connection failed"));

    _ensureConversationSyncSubscriptionForTests("conv-1");

    // Wait microtask tick for catch handler to settle
    await new Promise((r) => setTimeout(r, 10));

    // State must be cleared after failure to allow retry
    const failedState = _getConversationSyncStateForTests();
    expect(failedState.conversationId).toBeNull();
    expect(failedState.hasUnlisten).toBe(false);

    // Subsequent subscription attempt for the same conversation succeeds
    const stopFn = vi.fn();
    mockSubscribeConversationSync.mockResolvedValueOnce(stopFn);

    _ensureConversationSyncSubscriptionForTests("conv-1");
    await new Promise((r) => setTimeout(r, 10));

    expect(mockSubscribeConversationSync).toHaveBeenCalledTimes(2);
    const retryState = _getConversationSyncStateForTests();
    expect(retryState.conversationId).toBe("conv-1");
    expect(retryState.hasUnlisten).toBe(true);
  });

  it("ignores onEvent calls from superseded subscription ticket", async () => {
    let capturedOnEvent1: ((event: unknown) => void) | undefined;
    mockSubscribeConversationSync.mockImplementationOnce(
      ({ onEvent }: { onEvent: (event: unknown) => void }) => {
        capturedOnEvent1 = onEvent;
        return Promise.resolve(vi.fn());
      },
    );

    _ensureConversationSyncSubscriptionForTests("conv-1");
    await new Promise((r) => setTimeout(r, 10));
    expect(capturedOnEvent1).toBeDefined();

    // User switches to conv-2, advancing generation ticket
    mockSubscribeConversationSync.mockResolvedValueOnce(vi.fn());
    _ensureConversationSyncSubscriptionForTests("conv-2");
    await new Promise((r) => setTimeout(r, 10));

    // Stale event arriving from first subscription's channel
    const event = {
      kind: "conflict",
      conversationId: "conv-1",
      message: "stale conflict",
    };
    capturedOnEvent1!(event);

    expect(useAppStore.getState().appConflict).toBeNull();
  });

  it("tears down subscription when navigating to empty or null conversation", async () => {
    const stopFn = vi.fn();
    mockSubscribeConversationSync.mockResolvedValueOnce(stopFn);

    _ensureConversationSyncSubscriptionForTests("conv-1");
    await new Promise((r) => setTimeout(r, 10));

    expect(_getConversationSyncStateForTests().hasUnlisten).toBe(true);

    _ensureConversationSyncSubscriptionForTests(null);
    expect(stopFn).toHaveBeenCalledTimes(1);
    const state = _getConversationSyncStateForTests();
    expect(state.conversationId).toBeNull();
    expect(state.hasUnlisten).toBe(false);
  });

  it("forces resubscription when force option is true even for the same conversation", async () => {
    const stopFn1 = vi.fn();
    const stopFn2 = vi.fn();
    mockSubscribeConversationSync
      .mockResolvedValueOnce(stopFn1)
      .mockResolvedValueOnce(stopFn2);

    _ensureConversationSyncSubscriptionForTests("conv-1");
    await new Promise((r) => setTimeout(r, 10));
    expect(mockSubscribeConversationSync).toHaveBeenCalledTimes(1);

    // Call again with default force: false (no-op)
    _ensureConversationSyncSubscriptionForTests("conv-1");
    expect(mockSubscribeConversationSync).toHaveBeenCalledTimes(1);
    expect(stopFn1).not.toHaveBeenCalled();

    // Call with force: true
    _ensureConversationSyncSubscriptionForTests("conv-1", { force: true });
    expect(stopFn1).toHaveBeenCalledTimes(1);
    await new Promise((r) => setTimeout(r, 10));
    expect(mockSubscribeConversationSync).toHaveBeenCalledTimes(2);
    expect(_getConversationSyncStateForTests().hasUnlisten).toBe(true);
  });
});
