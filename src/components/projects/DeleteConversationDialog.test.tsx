import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DeleteConversationDialog } from "./DeleteConversationDialog";

describe("DeleteConversationDialog", () => {
  it("calls onDelete when Delete is confirmed", () => {
    const onDelete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(
      <DeleteConversationDialog
        open
        title="Build me a task manager"
        onClose={onClose}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByTestId("confirm-delete-conversation"));
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it("does not render when closed", () => {
    const { container } = render(
      <DeleteConversationDialog
        open={false}
        title="Hidden"
        onClose={() => {}}
        onDelete={async () => {}}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});
