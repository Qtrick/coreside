import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConversationQueue } from "./ConversationQueue";
import type { QueueChangedEvent } from "@/lib/tauri";

const queueHandlers = new Set<(event: QueueChangedEvent) => void>();

vi.mock("@/lib/tauri", () => ({
  api: {
    listAgentQueue: vi.fn(),
    cancelQueueItem: vi.fn(),
    subscribeConversationQueue: vi.fn(
      async (args: {
        conversationId: string;
        onEvent: (event: QueueChangedEvent) => void;
      }) => {
        queueHandlers.add(args.onEvent);
        return () => {
          queueHandlers.delete(args.onEvent);
        };
      },
    ),
  },
  isQueueEventForConversation: (
    event: { conversationId: string },
    conversationId: string,
  ) =>
    Boolean(conversationId) &&
    Boolean(event.conversationId) &&
    event.conversationId === conversationId,
}));

vi.mock("@/stores/app-store", () => ({
  useAppStore: (selector: (s: {
    sending: boolean;
    sendingConversationId: string | null;
  }) => unknown) =>
    selector({ sending: false, sendingConversationId: null }),
}));

import { api } from "@/lib/tauri";

function emitQueue(event: QueueChangedEvent) {
  for (const handler of queueHandlers) handler(event);
}

describe("ConversationQueue", () => {
  beforeEach(() => {
    queueHandlers.clear();
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

  it("refreshes on matching queue events and ignores other conversations", async () => {
    vi.mocked(api.listAgentQueue).mockResolvedValue([]);
    render(<ConversationQueue conversationId="conv-1" />);
    await waitFor(() => expect(queueHandlers.size).toBe(1));
    const callsAfterMount = vi.mocked(api.listAgentQueue).mock.calls.length;

    emitQueue({
      kind: "itemAdded",
      conversationId: "conv-other",
      itemId: "q-x",
    });
    await Promise.resolve();
    expect(vi.mocked(api.listAgentQueue).mock.calls.length).toBe(
      callsAfterMount,
    );

    emitQueue({
      kind: "itemAdded",
      conversationId: "conv-1",
      itemId: "q-1",
    });
    await waitFor(() =>
      expect(vi.mocked(api.listAgentQueue).mock.calls.length).toBeGreaterThan(
        callsAfterMount,
      ),
    );
  });
});
