import { describe, expect, it, vi, beforeEach } from "vitest";

const listenMock = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock("./runtime", () => ({
  isTauriRuntime: () => true,
}));

describe("listenApplicationDataChanged", () => {
  beforeEach(() => {
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => undefined);
  });

  it("subscribes to runtime-application-data-changed and normalizes payload", async () => {
    const { listenApplicationDataChanged } = await import("./events");
    const handler = vi.fn();
    await listenApplicationDataChanged(handler);

    expect(listenMock).toHaveBeenCalledWith(
      "runtime-application-data-changed",
      expect.any(Function),
    );
    const inner = listenMock.mock.calls[0]![1] as (event: {
      payload: unknown;
    }) => void;

    inner({
      payload: {
        applicationId: "  tool-study-planner  ",
        modelId: "  studyTasks  ",
        action: "local_data.write",
      },
    });
    expect(handler).toHaveBeenCalledWith({
      applicationId: "tool-study-planner",
      modelId: "studyTasks",
      action: "local_data.write",
      revision: null,
    });

    handler.mockClear();
    inner({ payload: { applicationId: "   " } });
    expect(handler).not.toHaveBeenCalled();

    inner({ payload: { applicationId: "app-a", modelId: "  ", revision: 7 } });
    expect(handler).toHaveBeenCalledWith({
      applicationId: "app-a",
      modelId: null,
      action: null,
      revision: 7,
    });
  });
});
