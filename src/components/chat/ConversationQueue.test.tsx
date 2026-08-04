import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConversationQueue } from "./ConversationQueue";

vi.mock("@/lib/tauri", () => ({
  api: {
    listAgentQueue: vi.fn(),
    cancelQueueItem: vi.fn(),
  },
}));

vi.mock("@/stores/app-store", () => ({
  useAppStore: (selector: (s: { sending: boolean }) => unknown) =>
    selector({ sending: false }),
}));

import { api } from "@/lib/tauri";

describe("ConversationQueue", () => {
  beforeEach(() => {
    vi.mocked(api.listAgentQueue).mockReset();
    vi.mocked(api.cancelQueueItem).mockReset();
  });

  it("renders nothing when the queue is empty", async () => {
    vi.mocked(api.listAgentQueue).mockResolvedValue([]);
    const { container } = render(
      <ConversationQueue conversationId="conv-1" />,
    );
    await waitFor(() => {
      expect(api.listAgentQueue).toHaveBeenCalledWith("conv-1");
    });
    expect(container).toBeEmptyDOMElement();
  });

  it("shows queued/active counts, preview, status, and cancel for queued items", async () => {
    vi.mocked(api.listAgentQueue).mockResolvedValue([
      {
        id: "q-1",
        conversationId: "conv-1",
        status: "queued",
        prompt: { content: "Follow up after the current reply" },
      },
      {
        id: "q-2",
        conversationId: "conv-1",
        status: "active",
        prompt: { content: "Running now" },
      },
    ]);

    render(<ConversationQueue conversationId="conv-1" />);

    const region = await screen.findByLabelText("Conversation queue");
    expect(region).toBeInTheDocument();
    expect(screen.getByText("1 queued · 1 active")).toBeInTheDocument();
    expect(
      screen.getByText("Follow up after the current reply"),
    ).toBeInTheDocument();
    expect(screen.getByText("queued")).toBeInTheDocument();
    expect(screen.getByText("active")).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Cancel queued message: Follow up after the current reply",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: /Cancel queued message: Running now/,
      }),
    ).not.toBeInTheDocument();
  });

  it("does not offer cancel while a cancel is in flight", async () => {
    let resolveCancel: (() => void) | undefined;
    vi.mocked(api.listAgentQueue).mockResolvedValue([
      {
        id: "q-1",
        conversationId: "conv-1",
        status: "queued",
        prompt: { content: "Hold this" },
      },
    ]);
    vi.mocked(api.cancelQueueItem).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveCancel = () => resolve({ id: "q-1", status: "cancelled" });
        }),
    );

    render(<ConversationQueue conversationId="conv-1" />);
    const cancel = await screen.findByRole("button", {
      name: "Cancel queued message: Hold this",
    });
    fireEvent.click(cancel);
    await waitFor(() => expect(cancel).toBeDisabled());
    resolveCancel?.();
    await waitFor(() => expect(api.cancelQueueItem).toHaveBeenCalledWith("q-1"));
  });
});
