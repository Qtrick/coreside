import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ModalPortal } from "./ModalPortal";

describe("ModalPortal", () => {
  let root: HTMLElement;

  beforeEach(() => {
    root = document.createElement("div");
    root.id = "root";
    document.body.appendChild(root);
  });

  afterEach(() => {
    root.remove();
    document.body.querySelectorAll("[data-modal-portal-test]").forEach((node) => node.remove());
  });

  it("sets inert and aria-hidden on #root while open", () => {
    render(
      <ModalPortal>
        <div role="dialog" aria-label="Test dialog">
          <button type="button">OK</button>
        </div>
      </ModalPortal>,
    );

    expect(root.hasAttribute("inert")).toBe(true);
    expect(root.getAttribute("aria-hidden")).toBe("true");
    expect(screen.getByRole("dialog", { name: "Test dialog" })).toBeInTheDocument();
  });

  it("clears inert and aria-hidden on #root after unmount", () => {
    const view = render(
      <ModalPortal>
        <div role="dialog" aria-label="Closing dialog">
          Content
        </div>
      </ModalPortal>,
    );

    expect(root.hasAttribute("inert")).toBe(true);
    view.unmount();
    expect(root.hasAttribute("inert")).toBe(false);
    expect(root.hasAttribute("aria-hidden")).toBe(false);
  });

  it("keeps #root inert until the last nested portal closes", () => {
    const view = render(
      <>
        <ModalPortal>
          <div role="dialog" aria-label="Outer">
            Outer
          </div>
        </ModalPortal>
        <ModalPortal>
          <div role="dialog" aria-label="Inner">
            Inner
          </div>
        </ModalPortal>
      </>,
    );

    expect(root.hasAttribute("inert")).toBe(true);
    view.unmount();
    expect(root.hasAttribute("inert")).toBe(false);
    expect(root.hasAttribute("aria-hidden")).toBe(false);
  });
});
