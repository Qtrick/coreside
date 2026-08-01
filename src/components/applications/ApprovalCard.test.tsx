import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ApprovalCard } from "@/components/applications/ApprovalCard";
import type { ApprovalRequest } from "@/types/application-kernel";

vi.mock("@/lib/tauri", () => ({
  api: {
    kernelDecideApproval: vi.fn(),
  },
}));

const baseApproval: ApprovalRequest = {
  id: "approval-1",
  applicationId: "app-1",
  actionName: "local_data.write",
  actionTitle: "Save note",
  risk: "write",
  critical: false,
  inputPreview: '{"table":"notes"}',
  callHash: "hash",
  descriptorHash: "desc",
  venue: "application",
  presence: "present",
  sessionId: null,
  runId: null,
  status: "pending",
  createdAt: new Date().toISOString(),
  expiresAt: new Date(Date.now() + 60_000).toISOString(),
  decidedAt: null,
  consumedAt: null,
  explanation: "Save your draft to local storage.",
  surfaceId: "surface-1",
  componentId: "save-btn",
};

describe("ApprovalCard", () => {
  it("renders risk text for write actions", () => {
    render(
      <ApprovalCard approval={baseApproval} applicationName="Notes" />,
    );

    expect(screen.getByText("Write — changes local data")).toBeInTheDocument();
    expect(screen.getByText("Save note")).toBeInTheDocument();
    expect(screen.getByText(/Save your draft/)).toBeInTheDocument();
  });

  it("hides Always allow for destructive actions", () => {
    render(
      <ApprovalCard
        approval={{
          ...baseApproval,
          risk: "destructive",
          actionTitle: "Delete note",
        }}
        applicationName="Notes"
      />,
    );

    expect(
      screen.getByText("Destructive — may remove data"),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Always allow for this application",
      }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Allow for this session" }),
    ).not.toBeInTheDocument();
  });

  it("hides standing remember options for critical actions", () => {
    render(
      <ApprovalCard
        approval={{
          ...baseApproval,
          critical: true,
          risk: "write",
        }}
        applicationName="Notes"
      />,
    );

    expect(screen.getByText("Critical action")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Always allow for this application",
      }),
    ).not.toBeInTheDocument();
  });
});
