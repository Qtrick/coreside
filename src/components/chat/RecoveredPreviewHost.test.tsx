import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RecoveredPreviewHost } from "./RecoveredPreviewHost";

vi.mock("@/lib/tauri", () => ({
  api: {},
  listenApprovalsChanged: vi.fn(async () => () => undefined),
  listenApplicationDataChanged: vi.fn(async () => () => undefined),
}));

describe("RecoveredPreviewHost", () => {
  it("renders speculative surfaces and close does not invoke privileged APIs", () => {
    const onClose = vi.fn();
    render(
      <RecoveredPreviewHost
        onClose={onClose}
        preview={{
          turnId: "turn-1",
          conversationId: "conv-1",
          attemptId: "attempt-1",
          fidelity: "full",
          warnings: [],
          surfaces: [
            {
              surfaceId: "surf-new",
              toolId: "tool-new",
              applicationId: null,
              revision: 1,
              sequence: 1,
              state: {},
              tool: {
                id: "tool-new",
                name: "Never Applied",
                description: "",
                layout: { type: "single-column" },
                components: [
                  {
                    id: "btn1",
                    type: "button",
                    props: { label: "Submit" },
                  },
                ],
              },
            },
          ],
        }}
      />,
    );

    expect(screen.getByTestId("recovered-preview-host")).toBeDefined();
    expect(screen.getByTestId("recovered-preview-surface")).toBeDefined();
    expect(screen.getByText("Speculative preview")).toBeDefined();
    fireEvent.click(
      screen.getByRole("button", { name: "Close recovered preview" }),
    );
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
