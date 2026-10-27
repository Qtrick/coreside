import { render, screen, waitFor, fireEvent, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AutomationsPanel } from "./AutomationsPanel";
import { api } from "@/lib/tauri";
import { listenAutomationsChanged } from "@/lib/tauri/events";

vi.mock("@/lib/tauri", () => ({
  api: {
    listAutomations: vi.fn(),
    listAutomationRuns: vi.fn(),
    runAutomationNow: vi.fn(),
    setAutomationEnabled: vi.fn(),
    deleteAutomation: vi.fn(),
  },
}));

vi.mock("@/lib/tauri/events", () => ({
  listenAutomationsChanged: vi.fn(),
}));

describe("AutomationsPanel", () => {
  let unlistenMock = vi.fn<() => void>();
  let changeListener: (() => void) | null = null;

  beforeEach(() => {
    vi.clearAllMocks();
    unlistenMock = vi.fn();
    changeListener = null;

    vi.mocked(listenAutomationsChanged).mockImplementation(async (handler) => {
      changeListener = handler;
      return () => {
        unlistenMock();
      };
    });

    vi.mocked(api.listAutomations).mockResolvedValue([
      {
        id: "auto-1",
        name: "Morning Study Brief",
        enabled: true,
        requiresAi: true,
        waitingApproval: false,
        permissionReady: true,
        nextRunAt: "2026-10-27T08:00:00Z",
        lastRunAt: "2026-10-26T08:00:00Z",
        lastStatus: "success",
        consecutiveFailures: 0,
        trigger: { type: "daily", time: "08:00" },
      },
    ]);

    vi.mocked(api.listAutomationRuns).mockResolvedValue([]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders automations list and registers live event listener", async () => {
    render(<AutomationsPanel onBack={vi.fn()} />);

    expect(await screen.findByText("Morning Study Brief")).toBeInTheDocument();
    expect(listenAutomationsChanged).toHaveBeenCalledTimes(1);
  });

  it("cleans up event listener on unmount", async () => {
    const { unmount } = render(<AutomationsPanel onBack={vi.fn()} />);

    await waitFor(() => {
      expect(listenAutomationsChanged).toHaveBeenCalled();
    });

    unmount();
    expect(unlistenMock).toHaveBeenCalledTimes(1);
  });

  it("discards out-of-order reload responses", async () => {
    type AutomationList = Awaited<ReturnType<typeof api.listAutomations>>;
    let resolveFirst: ((val: AutomationList) => void) | null = null;
    let resolveSecond: ((val: AutomationList) => void) | null = null;

    const firstPromise = new Promise<AutomationList>((resolve) => {
      resolveFirst = resolve;
    });
    const secondPromise = new Promise<AutomationList>((resolve) => {
      resolveSecond = resolve;
    });

    vi.mocked(api.listAutomations)
      .mockImplementationOnce(() => firstPromise)
      .mockImplementationOnce(() => secondPromise);

    render(<AutomationsPanel onBack={vi.fn()} />);

    // Second reload triggered via background event while first is pending
    await waitFor(() => {
      expect(changeListener).not.toBeNull();
    });
    changeListener!();

    // Second (newer) call resolves first with "Updated Title"
    resolveSecond!([
      {
        id: "auto-1",
        name: "Updated Title",
        enabled: true,
        requiresAi: true,
        waitingApproval: false,
        permissionReady: true,
        consecutiveFailures: 0,
        trigger: { type: "daily", time: "08:00" },
      },
    ]);

    expect(await screen.findByText("Updated Title")).toBeInTheDocument();

    // First (stale) call resolves late with "Stale Title"
    resolveFirst!([
      {
        id: "auto-1",
        name: "Stale Title",
        enabled: true,
        requiresAi: true,
        waitingApproval: false,
        permissionReady: true,
        consecutiveFailures: 0,
        trigger: { type: "daily", time: "08:00" },
      },
    ]);

    // Stale response must be discarded; "Updated Title" remains
    await new Promise((r) => setTimeout(r, 10));
    expect(screen.getByText("Updated Title")).toBeInTheDocument();
    expect(screen.queryByText("Stale Title")).not.toBeInTheDocument();
  });

  it("updates automation status to running when live event fires", async () => {
    render(<AutomationsPanel onBack={vi.fn()} />);

    expect(await screen.findByText("Morning Study Brief")).toBeInTheDocument();
    expect(screen.getByText("success")).toBeInTheDocument();

    // Backend emits start event: lastStatus is now 'running'
    vi.mocked(api.listAutomations).mockResolvedValueOnce([
      {
        id: "auto-1",
        name: "Morning Study Brief",
        enabled: true,
        requiresAi: true,
        waitingApproval: false,
        permissionReady: true,
        nextRunAt: "2026-10-27T08:00:00Z",
        lastRunAt: "2026-10-26T08:00:00Z",
        lastStatus: "running",
        consecutiveFailures: 0,
        trigger: { type: "daily", time: "08:00" },
      },
    ]);

    changeListener!();

    expect(await screen.findByText("running")).toBeInTheDocument();
  });

  it("refreshes run history for expanded automation when live event fires", async () => {
    render(<AutomationsPanel onBack={vi.fn()} />);

    expect(await screen.findByText("Morning Study Brief")).toBeInTheDocument();

    // Click history button
    const historyBtn = screen.getByRole("button", { name: /history/i });
    fireEvent.click(historyBtn);

    await waitFor(() => {
      expect(api.listAutomationRuns).toHaveBeenCalledWith("auto-1");
    });

    // Provide a newly completed run on event notification
    vi.mocked(api.listAutomationRuns).mockResolvedValueOnce([
      {
        id: "run-99",
        automationId: "auto-1",
        startedAt: "2026-10-27T08:00:00Z",
        completedAt: "2026-10-27T08:00:05Z",
        status: "success",
        resultSummary: "Produced study digest",
        createdAt: "2026-10-27T08:00:00Z",
      },
    ]);

    await act(async () => {
      changeListener!();
    });

    expect(await screen.findByText("Produced study digest")).toBeInTheDocument();
  });
});
