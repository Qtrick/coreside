import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AnimatedRevealText } from "./AnimatedRevealText";

describe("AnimatedRevealText", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn().mockImplementation((query: string) => ({
        matches: query.includes("prefers-reduced-motion"),
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("snaps to full text when reduced motion is preferred", () => {
    render(<AnimatedRevealText text="Hello agent" />);
    expect(screen.getByText("Hello agent")).toBeTruthy();
  });

  it("reveals toward the target under motion (rAF catch-up)", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn().mockImplementation(() => ({
        matches: false,
        media: "",
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    );

    let rafCb: FrameRequestCallback | null = null;
    let now = 0;
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => {
      rafCb = cb;
      return 1;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
    vi.spyOn(performance, "now").mockImplementation(() => now);

    const { rerender, container } = render(<AnimatedRevealText text="Hi" />);

    act(() => {
      now = 1000;
      rafCb?.(now);
    });

    expect(container.textContent).toContain("Hi");

    rerender(<AnimatedRevealText text="Hi there" />);
    act(() => {
      now = 2000;
      rafCb?.(now);
    });

    expect(container.textContent?.startsWith("Hi")).toBe(true);
  });
});
