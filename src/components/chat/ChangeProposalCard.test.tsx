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

  it("disables Apply when the loaded kernel proposal has no exact operations", async () => {
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

    await screen.findByText(
      "This proposal has no changes to apply. Regenerate it or discard it.",
    );
    const apply = screen.getByRole("button", {
      name: "Apply proposed change",
    });
    expect(apply).toBeDisabled();

    fireEvent.click(apply);
    await waitFor(() => expect(applyPending).not.toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Discard proposed change" })).toBeEnabled();
  });

  it("discloses required capability packs and permissions when evolution adds new capabilities", async () => {
    kernelGetProposal.mockResolvedValue({
      summary: "Add Progress Chart",
      impactSummary: "Adds visual progress analytics",
      risk: "write",
      status: "pending",
      exactOperations: [{ op: "component.insert" }],
      requiredCapabilities: [
        {
          packId: "coreside.charts",
          displayName: "Charts",
          surfaceId: "surf-study",
          permissions: ["render"],
          userDescription: "Adds Charts to this application. Allows Coreside to render interactive visual charts using the application's data.",
        },
      ],
      error: null,
    });

    render(
      <ChangeProposalCard
        proposalId="proposal-charts-1"
        summary="Add Progress Chart"
        impactSummary="Adds visual progress analytics"
        risk="write"
        operations={[{ op: "component.insert" }]}
        messageId="message-1"
        conversationId="conversation-1"
      />,
    );

    await screen.findByTestId("proposal-capabilities-section");
    expect(screen.getByText("Capabilities & permissions")).toBeInTheDocument();
    expect(screen.getByText("Charts")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Adds Charts to this application. Allows Coreside to render interactive visual charts using the application's data.",
      ),
    ).toBeInTheDocument();
  });
});
