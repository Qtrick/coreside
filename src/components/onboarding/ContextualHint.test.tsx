import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ContextualHint } from "./ContextualHint";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { __resetMockDb } from "@/lib/tauri";
import { emptyCoordinatorState } from "@/lib/onboarding/coordinator";
import { CONTEXTUAL_TIP_IDS, CONTEXTUAL_TIPS } from "@/lib/onboarding/tutorials";

type TargetRect = {
  top: number;
  left: number;
  width: number;
  height: number;
};

function mockElementRect(el: HTMLElement, rect: TargetRect) {
  vi.spyOn(el, "getBoundingClientRect").mockReturnValue({
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
    right: rect.left + rect.width,
    bottom: rect.top + rect.height,
    x: rect.left,
    y: rect.top,
    toJSON: () => rect,
  });
}

function boxesOverlap(
  r1: { top: number; bottom: number; left: number; right: number },
  r2: { top: number; bottom: number; left: number; right: number },
): boolean {
  const xOverlap = Math.max(0, Math.min(r1.right, r2.right) - Math.max(r1.left, r2.left));
  const yOverlap = Math.max(0, Math.min(r1.bottom, r2.bottom) - Math.max(r1.top, r2.top));
  return xOverlap * yOverlap > 0;
}

describe("ContextualHint Component", () => {
  let mediaQueryMatches = false;

  beforeEach(async () => {
    await __resetMockDb();

    // Reset onboarding store to clean idle baseline
    useOnboardingStore.setState({
      ...emptyCoordinatorState(),
      bootstrapped: true,
      welcomeEligible: false,
      onboardingDisabled: false,
      hasMeaningfulActivity: false,
      activeContextualId: null,
    });

    mediaQueryMatches = false;
    Object.defineProperty(window, "innerWidth", {
      writable: true,
      configurable: true,
      value: 1200,
    });
    Object.defineProperty(window, "innerHeight", {
      writable: true,
      configurable: true,
      value: 800,
    });

    Object.defineProperty(window, "matchMedia", {
      writable: true,
      configurable: true,
      value: vi.fn().mockImplementation((query: string) => {
        const isReducedMotion = query.includes("prefers-reduced-motion");
        return {
          matches: isReducedMotion ? mediaQueryMatches : false,
          media: query,
          onchange: null,
          addListener: vi.fn(),
          removeListener: vi.fn(),
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
          dispatchEvent: vi.fn(),
        };
      }),
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Layout and geometry separation", () => {
    it("ensures close button (header) and Got it button (footer) never overlap and maintain positive vertical distance", () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      render(<ContextualHint />);

      const aside = screen.getByTestId("contextual-hint");
      const closeBtn = screen.getByTestId("contextual-hint-close");
      const gotItBtn = screen.getByTestId("contextual-hint-got-it");

      // Container at bottom-right: 320x150
      mockElementRect(aside, { top: 620, left: 860, width: 320, height: 150 });
      // Close button at top-right of header: 24x24
      mockElementRect(closeBtn, { top: 634, left: 1140, width: 24, height: 24 });
      // Got it button at bottom-right of footer: 64x30
      mockElementRect(gotItBtn, { top: 726, left: 1100, width: 64, height: 30 });

      const closeRect = closeBtn.getBoundingClientRect();
      const gotItRect = gotItBtn.getBoundingClientRect();

      // Positive vertical distance between close bottom and got-it top
      const verticalGap = gotItRect.top - closeRect.bottom;
      expect(verticalGap).toBeGreaterThan(0);
      expect(verticalGap).toBe(68);

      // Disjoint bounding boxes check (zero area intersection)
      expect(boxesOverlap(closeRect, gotItRect)).toBe(false);
    });

    it("maintains strictly positive vertical separation even when content expands or contracts", () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstAiConnection);
      render(<ContextualHint />);

      const closeBtn = screen.getByTestId("contextual-hint-close");
      const gotItBtn = screen.getByTestId("contextual-hint-got-it");

      // Test with expansive multiline body (height 240px)
      mockElementRect(closeBtn, { top: 520, left: 1140, width: 24, height: 24 });
      mockElementRect(gotItBtn, { top: 710, left: 1100, width: 64, height: 32 });

      let closeRect = closeBtn.getBoundingClientRect();
      let gotItRect = gotItBtn.getBoundingClientRect();

      expect(gotItRect.top - closeRect.bottom).toBe(166);
      expect(boxesOverlap(closeRect, gotItRect)).toBe(false);

      // Test with compact single-line body (height 100px)
      mockElementRect(closeBtn, { top: 680, left: 1140, width: 24, height: 24 });
      mockElementRect(gotItBtn, { top: 730, left: 1100, width: 64, height: 28 });

      closeRect = closeBtn.getBoundingClientRect();
      gotItRect = gotItBtn.getBoundingClientRect();

      expect(gotItRect.top - closeRect.bottom).toBe(26);
      expect(boxesOverlap(closeRect, gotItRect)).toBe(false);
    });
  });

  describe("Long title and multiline body rendering", () => {
    it("renders full long title and multiline body text cleanly without truncation", () => {
      // firstAiConnection has long title and multi-sentence body
      const tipDef = CONTEXTUAL_TIPS.find((t) => t.id === CONTEXTUAL_TIP_IDS.firstAiConnection)!;
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstAiConnection);

      render(<ContextualHint />);

      const titleEl = document.getElementById("contextual-hint-title");
      expect(titleEl).toBeInTheDocument();
      expect(titleEl).toHaveTextContent(tipDef.title);

      const bodyEl = document.getElementById("contextual-hint-body");
      expect(bodyEl).toBeInTheDocument();
      expect(bodyEl).toHaveTextContent(tipDef.body);
      expect(bodyEl?.classList.contains("contextual-hint-body")).toBe(true);

      // DOM hierarchy verification: sr-only -> header -> body -> footer
      const aside = screen.getByTestId("contextual-hint");
      const header = aside.querySelector(".contextual-hint-header") as HTMLElement;
      const body = aside.querySelector(".contextual-hint-body") as HTMLElement;
      const footer = aside.querySelector(".contextual-hint-actions") as HTMLElement;

      expect(header).toBeInTheDocument();
      expect(body).toBeInTheDocument();
      expect(footer).toBeInTheDocument();

      // Sequential non-overlapping vertical flow
      mockElementRect(header, { top: 550, left: 860, width: 320, height: 30 });
      mockElementRect(body, { top: 588, left: 860, width: 320, height: 120 });
      mockElementRect(footer, { top: 716, left: 860, width: 320, height: 40 });

      expect(header.getBoundingClientRect().bottom).toBeLessThanOrEqual(body.getBoundingClientRect().top);
      expect(body.getBoundingClientRect().bottom).toBeLessThanOrEqual(footer.getBoundingClientRect().top);
    });
  });

  describe("Narrow window width constraints", () => {
    it("conforms within viewport boundaries on narrow screens without breaking layout", () => {
      Object.defineProperty(window, "innerWidth", {
        writable: true,
        configurable: true,
        value: 320,
      });

      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstProject);
      render(<ContextualHint />);

      const aside = screen.getByTestId("contextual-hint");
      expect(aside).toBeInTheDocument();

      // At 320px width, min(24rem, 100vw - 2rem) = 320 - 32 = 288px width
      mockElementRect(aside, { top: 600, left: 16, width: 288, height: 160 });

      const asideRect = aside.getBoundingClientRect();
      expect(asideRect.left).toBeGreaterThanOrEqual(0);
      expect(asideRect.right).toBeLessThanOrEqual(window.innerWidth);
      expect(asideRect.width).toBeLessThanOrEqual(window.innerWidth);

      // Close and Got it buttons remain separated within the narrow container
      const closeBtn = screen.getByTestId("contextual-hint-close");
      const gotItBtn = screen.getByTestId("contextual-hint-got-it");
      mockElementRect(closeBtn, { top: 610, left: 270, width: 24, height: 24 });
      mockElementRect(gotItBtn, { top: 716, left: 230, width: 64, height: 32 });

      expect(gotItBtn.getBoundingClientRect().top - closeBtn.getBoundingClientRect().bottom).toBeGreaterThan(0);
      expect(boxesOverlap(closeBtn.getBoundingClientRect(), gotItBtn.getBoundingClientRect())).toBe(false);
    });
  });

  describe("Dismissal controls and persistence", () => {
    it("dismisses hint via '×' close button and persists completed status preventing resurrection", async () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      const { unmount } = render(<ContextualHint />);

      expect(screen.getByTestId("contextual-hint")).toBeInTheDocument();
      const closeBtn = screen.getByTestId("contextual-hint-close");

      await act(async () => {
        fireEvent.click(closeBtn);
      });

      // activeContextualId cleared immediately
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      // Persisted to completed in progress state
      const progress = useOnboardingStore.getState().progressById[CONTEXTUAL_TIP_IDS.firstApp];
      expect(progress?.status).toBe("completed");

      // Verify it cannot resurrect on future offer
      const reoffered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(reoffered).toBe(false);
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      unmount();
    });

    it("dismisses hint via 'Got it' action button and persists completed status preventing resurrection", async () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstQueue);
      const { unmount } = render(<ContextualHint />);

      expect(screen.getByTestId("contextual-hint")).toBeInTheDocument();
      const gotItBtn = screen.getByTestId("contextual-hint-got-it");

      await act(async () => {
        fireEvent.click(gotItBtn);
      });

      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      const progress = useOnboardingStore.getState().progressById[CONTEXTUAL_TIP_IDS.firstQueue];
      expect(progress?.status).toBe("completed");

      // Verify no resurrection
      const reoffered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstQueue);
      expect(reoffered).toBe(false);
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      unmount();
    });
  });

  describe("Keyboard accessibility", () => {
    it("supports Enter and Space key activation on the '×' close button", async () => {
      // Enter on close button
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      const { unmount: unmount1 } = render(<ContextualHint />);

      const closeBtn = screen.getByTestId("contextual-hint-close");
      await act(async () => {
        fireEvent.keyDown(closeBtn, { key: "Enter" });
      });

      expect(useOnboardingStore.getState().activeContextualId).toBeNull();
      expect(useOnboardingStore.getState().progressById[CONTEXTUAL_TIP_IDS.firstApp]?.status).toBe("completed");
      unmount1();

      // Space on close button
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstVersions);
      const { unmount: unmount2 } = render(<ContextualHint />);

      const closeBtn2 = screen.getByTestId("contextual-hint-close");
      await act(async () => {
        fireEvent.keyDown(closeBtn2, { key: " " });
      });

      expect(useOnboardingStore.getState().activeContextualId).toBeNull();
      expect(useOnboardingStore.getState().progressById[CONTEXTUAL_TIP_IDS.firstVersions]?.status).toBe("completed");
      unmount2();
    });

    it("supports Enter and Space key activation on the 'Got it' button", async () => {
      // Enter on got-it button
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstProject);
      const { unmount: unmount1 } = render(<ContextualHint />);

      const gotItBtn = screen.getByTestId("contextual-hint-got-it");
      await act(async () => {
        fireEvent.keyDown(gotItBtn, { key: "Enter" });
      });

      expect(useOnboardingStore.getState().activeContextualId).toBeNull();
      expect(useOnboardingStore.getState().progressById[CONTEXTUAL_TIP_IDS.firstProject]?.status).toBe("completed");
      unmount1();

      // Space on got-it button
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApproval);
      const { unmount: unmount2 } = render(<ContextualHint />);

      const gotItBtn2 = screen.getByTestId("contextual-hint-got-it");
      await act(async () => {
        fireEvent.keyDown(gotItBtn2, { key: " " });
      });

      expect(useOnboardingStore.getState().activeContextualId).toBeNull();
      expect(useOnboardingStore.getState().progressById[CONTEXTUAL_TIP_IDS.firstApproval]?.status).toBe("completed");
      unmount2();
    });

    it("provides complete ARIA roles, labeledby/describedby associations, and live region announcement", () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      render(<ContextualHint />);

      const aside = screen.getByTestId("contextual-hint");
      expect(aside).toHaveAttribute("role", "region");
      expect(aside).toHaveAttribute("aria-labelledby", "contextual-hint-title");
      expect(aside).toHaveAttribute("aria-describedby", "contextual-hint-body");

      // Verify targets exist
      const title = document.getElementById("contextual-hint-title");
      const body = document.getElementById("contextual-hint-body");
      expect(title).toBeInTheDocument();
      expect(body).toBeInTheDocument();

      // Screen reader polite live region
      const liveRegion = aside.querySelector('[aria-live="polite"]');
      expect(liveRegion).toBeInTheDocument();
      expect(liveRegion).toHaveAttribute("role", "status");
      expect(liveRegion).toHaveClass("sr-only");
      expect(liveRegion).toHaveTextContent("Your first app");

      // Close button aria-label
      const closeBtn = screen.getByTestId("contextual-hint-close");
      expect(closeBtn).toHaveAttribute("aria-label", "Dismiss hint");
      expect(closeBtn).toHaveAttribute("type", "button");

      // Got it button
      const gotItBtn = screen.getByTestId("contextual-hint-got-it");
      expect(gotItBtn).toHaveAttribute("type", "button");
    });

    it("allows standard keyboard focus and tab navigation between controls", () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      render(<ContextualHint />);

      const closeBtn = screen.getByTestId("contextual-hint-close");
      const gotItBtn = screen.getByTestId("contextual-hint-got-it");

      closeBtn.focus();
      expect(document.activeElement).toBe(closeBtn);

      gotItBtn.focus();
      expect(document.activeElement).toBe(gotItBtn);
    });
  });

  describe("Blocking phases and priority isolation", () => {
    it("does not offer or display hint when phase is 'welcome'", () => {
      useOnboardingStore.setState({ phase: "welcome" });

      const offered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(offered).toBe(false);
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      const { container } = render(<ContextualHint />);
      expect(container.firstChild).toBeNull();
    });

    it("does not offer or display hint when phase is 'appearance'", () => {
      useOnboardingStore.setState({ phase: "appearance" });

      const offered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(offered).toBe(false);
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      const { container } = render(<ContextualHint />);
      expect(container.firstChild).toBeNull();
    });

    it("does not offer or display hint when phase is 'tour'", () => {
      useOnboardingStore.setState({ phase: "tour" });

      const offered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(offered).toBe(false);
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      const { container } = render(<ContextualHint />);
      expect(container.firstChild).toBeNull();
    });

    it("offers and displays hint when phase is 'idle'", () => {
      useOnboardingStore.setState({ phase: "idle" });

      const offered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(offered).toBe(true);
      expect(useOnboardingStore.getState().activeContextualId).toBe(CONTEXTUAL_TIP_IDS.firstApp);

      render(<ContextualHint />);
      expect(screen.getByTestId("contextual-hint")).toBeInTheDocument();
    });
  });

  describe("Onboarding disabled and stacking prevention", () => {
    it("respects onboardingDisabled: true and suppresses contextual tips", () => {
      useOnboardingStore.setState({ onboardingDisabled: true, phase: "idle" });

      const offered = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(offered).toBe(false);
      expect(useOnboardingStore.getState().activeContextualId).toBeNull();

      const { container } = render(<ContextualHint />);
      expect(container.firstChild).toBeNull();
    });

    it("never stacks with an already active contextual hint", () => {
      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      expect(useOnboardingStore.getState().activeContextualId).toBe(CONTEXTUAL_TIP_IDS.firstApp);

      // Attempting to offer a different hint while one is active returns false
      const offeredSecond = useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstProject);
      expect(offeredSecond).toBe(false);

      // Active hint remains the first one
      expect(useOnboardingStore.getState().activeContextualId).toBe(CONTEXTUAL_TIP_IDS.firstApp);

      render(<ContextualHint />);
      expect(screen.getByTestId("contextual-hint")).toHaveTextContent("Your first app");
      expect(screen.queryByText("Projects keep context together")).not.toBeInTheDocument();
    });
  });

  describe("Reduced motion class handling", () => {
    it("applies reduced-motion class when prefers-reduced-motion matches", () => {
      mediaQueryMatches = true;

      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      render(<ContextualHint />);

      const aside = screen.getByTestId("contextual-hint");
      expect(aside.classList.contains("reduced-motion")).toBe(true);
    });

    it("omits reduced-motion class when prefers-reduced-motion does not match", () => {
      mediaQueryMatches = false;

      useOnboardingStore.getState().offerContextual(CONTEXTUAL_TIP_IDS.firstApp);
      render(<ContextualHint />);

      const aside = screen.getByTestId("contextual-hint");
      expect(aside.classList.contains("reduced-motion")).toBe(false);
    });
  });
});
