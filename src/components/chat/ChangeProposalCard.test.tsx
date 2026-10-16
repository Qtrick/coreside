import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChangeProposalCard } from "./ChangeProposalCard";

const { applyPending, discardPending, kernelGetProposal, store } = vi.hoisted(
  () => ({
    applyPending: vi.fn(async () => undefined),
    discardPending: vi.fn(async () => undefined),
    kernelGetProposal: vi.fn(),
    store: {
      pendingKernelProposal: null as Record<string, unknown> | null,
    },
  }),
);

vi.mock("@/lib/tauri", () => ({
  api: {
    kernelGetProposal,
  },
}));

vi.mock("@/stores/app-store", () => {
  const state = {
    ...store,
    applyPendingKernelProposal: applyPending,
    discardPendingKernelProposal: discardPending,
  };
  const useAppStore = Object.assign(
    (selector: (value: typeof state) => unknown) => selector(state),
    {
      setState: (next: Partial<typeof state>) => Object.assign(state, next),
    },
  );
  return { useAppStore };
});

vi.mock("@/components/tool-renderer/ToolRenderer", () => ({
  ToolRenderer: () => null,
}));

describe("ChangeProposalCard", () => {
  beforeEach(() => {
    applyPending.mockClear();
    discardPending.mockClear();
    kernelGetProposal.mockReset();
    store.pendingKernelProposal = null;
  });

  it("keeps Apply enabled for a pending proposal with an empty preview", async () => {
    kernelGetProposal.mockResolvedValue({
      summary: "Update app",
      impactSummary: "Updates the app",
      risk: "write",
      status: "pending",
      exactOperations: [],
      error: null,
    });

    render(
      <ChangeProposalCard
        proposalId="proposal-1"
        summary="Update app"
        impactSummary="Updates the app"
        risk="write"
        operations={[{ op: "surface.update" }]}
        messageId="message-1"
        conversationId="conversation-1"
      />,
    );

    await screen.findByText("No detailed preview is available for this proposal.");
    const apply = screen.getByRole("button", {
      name: "Apply proposed change",
    });
    expect(apply).toBeEnabled();

    fireEvent.click(apply);
    await waitFor(() => expect(applyPending).toHaveBeenCalledOnce());
  });
});
