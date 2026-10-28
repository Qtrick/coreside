import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TutorialOverlay } from "./TutorialOverlay";
import { WelcomeDialog } from "./WelcomeDialog";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { api, __resetMockDb } from "@/lib/tauri";
import { emptyCoordinatorState } from "@/lib/onboarding/coordinator";
import { ESSENTIALS_TUTORIAL_ID } from "@/lib/onboarding/tutorials";

type TargetRect = { top: number; left: number; width: number; height: number };

function mockElementBounds(el: HTMLElement, rect: TargetRect) {
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

describe("TutorialOverlay & Onboarding Flow", () => {
  let rafQueue: FrameRequestCallback[] = [];
  let fixtureContainer: HTMLDivElement;

  function flushRaf() {
    act(() => {
      let cycles = 0;
      while (rafQueue.length > 0 && cycles < 35) {
        cycles++;
        const currentQueue = rafQueue;
        rafQueue = [];
        for (const cb of currentQueue) {
          cb(performance.now());
        }
      }
    });
  }

  beforeEach(async () => {
    await __resetMockDb();

    // Reset onboarding store state
    useOnboardingStore.setState({
      ...emptyCoordinatorState(),
      bootstrapped: false,
      welcomeEligible: false,
      onboardingDisabled: false,
      hasMeaningfulActivity: false,
      activeContextualId: null,
    });

    rafQueue = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => {
      rafQueue.push(cb);
      return rafQueue.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});

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
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });

    // Ensure CSS.escape exists in test environment
    if (typeof globalThis.CSS === "undefined" || !globalThis.CSS.escape) {
      globalThis.CSS = {
        escape: (s: string) => s,
      } as unknown as typeof CSS;
    }

    fixtureContainer = document.createElement("div");
    fixtureContainer.id = "test-fixture-root";
    document.body.appendChild(fixtureContainer);
    document.getElementById("coreside-window-overlay-root")?.remove();
  });

  afterEach(() => {
    fixtureContainer.remove();
    document.getElementById("coreside-window-overlay-root")?.remove();
    vi.restoreAllMocks();
  });

  describe("Fresh first-run state with NO chats and NO apps", () => {
    it("recognizes fresh first-run state as eligible for welcome dialog", async () => {
      await useOnboardingStore.getState().hydrate();
      const state = useOnboardingStore.getState();

      expect(state.welcomeEligible).toBe(true);
      expect(state.hasMeaningfulActivity).toBe(false);
      expect(state.phase).toBe("idle");
    });

    it("opens WelcomeDialog when eligible, and 'Later' dismisses immediately marking essentials skipped", async () => {
      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("welcome");

      render(<WelcomeDialog />);
      expect(
        screen.getByRole("heading", { name: "Welcome to Coreside" }),
      ).toBeInTheDocument();

      const laterButton = document.querySelector<HTMLButtonElement>(
        'button[data-action="dismiss-welcome"]',
      )!;
      expect(laterButton).toBeInTheDocument();

      await act(async () => {
        fireEvent.click(laterButton);
      });

      // Synchronously returns to idle
      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);

      // Persisted status is skipped
      const progress =
        useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("skipped");
    });

    it("launches Essentials Tour directly from WelcomeDialog at Step 1", async () => {
      await useOnboardingStore.getState().openWelcomeIfEligible();
      render(<WelcomeDialog />);

      const takeTourButton = screen.getByRole("button", {
        name: /Take the 3-minute tour/i,
      });

      await act(async () => {
        fireEvent.click(takeTourButton);
      });

      expect(useOnboardingStore.getState().phase).toBe("tour");
      expect(useOnboardingStore.getState().activeTutorialId).toBe(
        ESSENTIALS_TUTORIAL_ID,
      );
      // Skips welcome step 0, starts directly at step 1
      expect(useOnboardingStore.getState().stepIndex).toBe(1);
    });
  });

  describe("Relaunch eligibility", () => {
    it("is NOT eligible for welcome and does not auto-open tour when essentials is skipped", async () => {
      await api.upsertTutorialProgress({
        tutorialId: ESSENTIALS_TUTORIAL_ID,
        tutorialVersion: 1,
        status: "skipped",
      });

      await useOnboardingStore.getState().hydrate();
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);

      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("idle");
    });

    it("is NOT eligible for welcome when essentials is completed", async () => {
      await api.upsertTutorialProgress({
        tutorialId: ESSENTIALS_TUTORIAL_ID,
        tutorialVersion: 1,
        status: "completed",
      });

      await useOnboardingStore.getState().hydrate();
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);

      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("idle");
    });

    it("is NOT eligible for welcome when essentials is in_progress", async () => {
      await api.upsertTutorialProgress({
        tutorialId: ESSENTIALS_TUTORIAL_ID,
        tutorialVersion: 1,
        status: "in_progress",
        currentStepId: "sidebar-new-chat",
      });

      await useOnboardingStore.getState().hydrate();
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);

      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("idle");
    });
  });

  describe("Step targeting and fallback mechanisms", () => {
    it("Step 1 targets sidebar-new-chat when mounted", async () => {
      const targetEl = document.createElement("button");
      targetEl.setAttribute("data-coreside-tour", "sidebar-new-chat");
      targetEl.textContent = "New chat";
      fixtureContainer.appendChild(targetEl);
      mockElementBounds(targetEl, { top: 40, left: 16, width: 200, height: 36 });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      expect(
        screen.getByRole("heading", { name: "Start with a conversation" }),
      ).toBeInTheDocument();

      const spotlight = document.querySelector(".tutorial-spotlight-ring");
      expect(spotlight).toBeInTheDocument();
      // target (16, 40, 200, 36) with 6px padding: left=10, top=34, width=212, height=48
      expect((spotlight as HTMLElement).style.top).toBe("34px");
      expect((spotlight as HTMLElement).style.left).toBe("10px");
      expect((spotlight as HTMLElement).style.width).toBe("212px");
      expect((spotlight as HTMLElement).style.height).toBe("48px");
    });

    it("Step 2 targets chat-composer when present", async () => {
      const composerEl = document.createElement("div");
      composerEl.setAttribute("data-coreside-tour", "chat-composer");
      fixtureContainer.appendChild(composerEl);
      mockElementBounds(composerEl, {
        top: 700,
        left: 320,
        width: 600,
        height: 60,
      });

      await useOnboardingStore.getState().startEssentials(false);
      // Advance to step 2 (chat-composer)
      await act(async () => {
        await useOnboardingStore.getState().next();
      });

      render(<TutorialOverlay />);
      flushRaf();

      expect(
        screen.getByRole("heading", { name: "Ask in chat" }),
      ).toBeInTheDocument();

      const spotlight = document.querySelector(".tutorial-spotlight-ring") as HTMLElement;
      expect(spotlight).toBeInTheDocument();
      // target (320, 700, 600, 60) with pad=6 => top=694, left=314, width=612, height=72
      expect(spotlight.style.top).toBe("694px");
      expect(spotlight.style.left).toBe("314px");
      expect(spotlight.style.width).toBe("612px");
      expect(spotlight.style.height).toBe("72px");
    });

    it("Step 2 falls back to chat-panel when no conversation is open (no composer in DOM)", async () => {
      // In fresh state with no active conversation, only empty chat-panel is mounted
      const panelEl = document.createElement("div");
      panelEl.setAttribute("data-coreside-tour", "chat-panel");
      fixtureContainer.appendChild(panelEl);
      mockElementBounds(panelEl, {
        top: 60,
        left: 280,
        width: 880,
        height: 680,
      });

      await useOnboardingStore.getState().startEssentials(false);
      await act(async () => {
        await useOnboardingStore.getState().next();
      });

      render(<TutorialOverlay />);
      flushRaf();

      expect(
        screen.getByRole("heading", { name: "Ask in chat" }),
      ).toBeInTheDocument();

      const spotlight = document.querySelector(".tutorial-spotlight-ring") as HTMLElement;
      expect(spotlight).toBeInTheDocument();
      // target (280, 60, 880, 680) with pad=6 => top=54, left=274, width=892, height=692
      expect(spotlight.style.top).toBe("54px");
      expect(spotlight.style.left).toBe("274px");
      expect(spotlight.style.width).toBe("892px");
      expect(spotlight.style.height).toBe("692px");
    });

    it("Step 3 (app-panel) falls back to sidebar-apps when no app canvas is mounted", async () => {
      // No app canvas in DOM; sidebar apps nav is present
      const sidebarAppsEl = document.createElement("button");
      sidebarAppsEl.setAttribute("data-coreside-tour", "sidebar-apps");
      sidebarAppsEl.textContent = "Apps";
      fixtureContainer.appendChild(sidebarAppsEl);
      mockElementBounds(sidebarAppsEl, {
        top: 120,
        left: 16,
        width: 200,
        height: 36,
      });

      await useOnboardingStore.getState().startEssentials(false);
      // Advance to step 3 (app-panel)
      await act(async () => {
        await useOnboardingStore.getState().next();
        await useOnboardingStore.getState().next();
      });

      render(<TutorialOverlay />);
      flushRaf();

      expect(
        screen.getByRole("heading", { name: "Apps live beside chat" }),
      ).toBeInTheDocument();

      const spotlight = document.querySelector(".tutorial-spotlight-ring") as HTMLElement;
      expect(spotlight).toBeInTheDocument();
      // target (16, 120, 200, 36) with pad=6 => top=114, left=10, width=212, height=48
      expect(spotlight.style.top).toBe("114px");
      expect(spotlight.style.left).toBe("10px");
      expect(spotlight.style.width).toBe("212px");
      expect(spotlight.style.height).toBe("48px");
    });

    it("Step 3 targets app-panel when app canvas is mounted", async () => {
      const appPanelEl = document.createElement("div");
      appPanelEl.setAttribute("data-coreside-tour", "app-panel");
      fixtureContainer.appendChild(appPanelEl);
      mockElementBounds(appPanelEl, {
        top: 50,
        left: 650,
        width: 500,
        height: 700,
      });

      await useOnboardingStore.getState().startEssentials(false);
      await act(async () => {
        await useOnboardingStore.getState().next();
        await useOnboardingStore.getState().next();
      });

      render(<TutorialOverlay />);
      flushRaf();

      const spotlight = document.querySelector(".tutorial-spotlight-ring") as HTMLElement;
      expect(spotlight).toBeInTheDocument();
      // target (650, 50, 500, 700) with pad=6 => top=44, left=644, width=512, height=712
      expect(spotlight.style.top).toBe("44px");
      expect(spotlight.style.left).toBe("644px");
      expect(spotlight.style.width).toBe("512px");
      expect(spotlight.style.height).toBe("712px");
    });

    it("renders .tutorial-scrim-fullscreen when step has no target or target is absent", async () => {
      // Tour started with no matching target elements in DOM
      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      const fullScrim = document.querySelector(".tutorial-scrim-fullscreen");
      expect(fullScrim).toBeInTheDocument();

      // Individual directional scrims and spotlight ring should NOT be present
      expect(document.querySelector(".tutorial-scrim-top")).not.toBeInTheDocument();
      expect(document.querySelector(".tutorial-scrim-bottom")).not.toBeInTheDocument();
      expect(document.querySelector(".tutorial-scrim-left")).not.toBeInTheDocument();
      expect(document.querySelector(".tutorial-scrim-right")).not.toBeInTheDocument();
      expect(document.querySelector(".tutorial-spotlight-ring")).not.toBeInTheDocument();
    });
  });

  describe("Scrim panels and spotlight bounds", () => {
    it("renders the 4 scrim panels around the target, darkening outer regions while leaving target clear", async () => {
      const targetEl = document.createElement("div");
      targetEl.setAttribute("data-coreside-tour", "sidebar-new-chat");
      fixtureContainer.appendChild(targetEl);
      mockElementBounds(targetEl, { top: 100, left: 50, width: 200, height: 40 });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      // Target geometry with pad = 6:
      // t = 94, l = 44, w = 212, h = 52. vw = 1200, vh = 800
      const scrimTop = document.querySelector(".tutorial-scrim-top") as HTMLElement;
      const scrimBottom = document.querySelector(".tutorial-scrim-bottom") as HTMLElement;
      const scrimLeft = document.querySelector(".tutorial-scrim-left") as HTMLElement;
      const scrimRight = document.querySelector(".tutorial-scrim-right") as HTMLElement;
      const ring = document.querySelector(".tutorial-spotlight-ring") as HTMLElement;

      expect(scrimTop).toBeInTheDocument();
      expect(scrimBottom).toBeInTheDocument();
      expect(scrimLeft).toBeInTheDocument();
      expect(scrimRight).toBeInTheDocument();
      expect(ring).toBeInTheDocument();

      // Top scrim covers 0 to t
      expect(scrimTop.style.top).toBe("0px");
      expect(scrimTop.style.left).toBe("0px");
      expect(scrimTop.style.width).toBe("100vw");
      expect(scrimTop.style.height).toBe("94px");

      // Bottom scrim covers t + h to vh
      expect(scrimBottom.style.top).toBe("146px"); // 94 + 52
      expect(scrimBottom.style.left).toBe("0px");
      expect(scrimBottom.style.width).toBe("100vw");
      expect(scrimBottom.style.height).toBe("654px"); // 800 - 146

      // Left scrim covers 0 to l at vertical band t to t + h
      expect(scrimLeft.style.top).toBe("94px");
      expect(scrimLeft.style.left).toBe("0px");
      expect(scrimLeft.style.width).toBe("44px");
      expect(scrimLeft.style.height).toBe("52px");

      // Right scrim covers l + w to vw at vertical band t to t + h
      expect(scrimRight.style.top).toBe("94px");
      expect(scrimRight.style.left).toBe("256px"); // 44 + 212
      expect(scrimRight.style.width).toBe("944px"); // 1200 - 256
      expect(scrimRight.style.height).toBe("52px");

      // Spotlight ring bounds
      expect(ring.style.top).toBe("94px");
      expect(ring.style.left).toBe("44px");
      expect(ring.style.width).toBe("212px");
      expect(ring.style.height).toBe("52px");
    });

    it("positions popover to the right of sidebar targets without overlapping target", async () => {
      const sidebarTarget = document.createElement("button");
      sidebarTarget.setAttribute("data-coreside-tour", "sidebar-new-chat");
      fixtureContainer.appendChild(sidebarTarget);
      // Target in sidebar: left < 300
      mockElementBounds(sidebarTarget, {
        top: 80,
        left: 20,
        width: 180,
        height: 36,
      });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      const popoverPositioner = document.querySelector(
        ".tutorial-popover-positioner",
      ) as HTMLElement;
      expect(popoverPositioner).toBeInTheDocument();

      // rightSide = left(20) + width(180) + gap(12) = 212px
      expect(popoverPositioner.style.left).toBe("212px");
      // Target right edge is 20 + 180 = 200px. Popover begins at 212px, completely clear of target!
      const popoverLeftNum = parseInt(popoverPositioner.style.left, 10);
      expect(popoverLeftNum).toBeGreaterThan(20 + 180);
    });
  });

  describe("Keyboard and dismissal controls", () => {
    it("Escape key triggers pause, immediately returning to idle and preserving in_progress status", async () => {
      const targetEl = document.createElement("button");
      targetEl.setAttribute("data-coreside-tour", "sidebar-new-chat");
      fixtureContainer.appendChild(targetEl);
      mockElementBounds(targetEl, { top: 40, left: 16, width: 200, height: 36 });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      expect(useOnboardingStore.getState().phase).toBe("tour");

      await act(async () => {
        fireEvent.keyDown(window, { key: "Escape" });
      });

      expect(useOnboardingStore.getState().phase).toBe("idle");
      const progress =
        useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("in_progress");

      // Verify progress and active step were persisted to backend DB
      const onboarding = await api.getOnboardingState();
      const persisted = onboarding.progress.find(
        (p) => p.tutorialId === ESSENTIALS_TUTORIAL_ID,
      );
      expect(persisted?.status).toBe("in_progress");
      expect(persisted?.currentStepId).toBe("sidebar-new-chat");
    });

    it("Tab key cycles focus within popover buttons without leaking to document", async () => {
      const targetEl = document.createElement("button");
      targetEl.setAttribute("data-coreside-tour", "sidebar-new-chat");
      fixtureContainer.appendChild(targetEl);
      mockElementBounds(targetEl, { top: 40, left: 16, width: 200, height: 36 });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      const skipBtn = screen.getByRole("button", { name: "Skip" });
      const closeBtn = screen.getByRole("button", { name: "Close tour" });
      const nextBtn = screen.getByRole("button", { name: "Next" });

      // Initially next action is focused
      expect(document.activeElement).toBe(nextBtn);

      // Shift+Tab from skip (first element) wraps to close (last element)
      skipBtn.focus();
      expect(document.activeElement).toBe(skipBtn);
      fireEvent.keyDown(window, { key: "Tab", shiftKey: true });
      expect(document.activeElement).toBe(closeBtn);

      // Tab from close (last element) wraps to skip (first element)
      closeBtn.focus();
      fireEvent.keyDown(window, { key: "Tab", shiftKey: false });
      expect(document.activeElement).toBe(skipBtn);
    });

    it("clamps popover position within safe viewport margins on compact screens", async () => {
      Object.defineProperty(window, "innerWidth", { value: 320, configurable: true });
      Object.defineProperty(window, "innerHeight", { value: 400, configurable: true });

      const targetEl = document.createElement("div");
      targetEl.setAttribute("data-coreside-tour", "chat-composer");
      fixtureContainer.appendChild(targetEl);
      // Target positioned near corner
      mockElementBounds(targetEl, { top: 380, left: 290, width: 60, height: 40 });

      await useOnboardingStore.getState().startEssentials(false);
      await act(async () => {
        await useOnboardingStore.getState().next();
      });

      render(<TutorialOverlay />);
      flushRaf();

      const positioner = document.querySelector(".tutorial-popover-positioner") as HTMLElement;
      expect(positioner).toBeInTheDocument();
      const topNum = parseInt(positioner.style.top, 10);
      const leftNum = parseInt(positioner.style.left, 10);
      expect(topNum).toBeGreaterThanOrEqual(16);
      expect(leftNum).toBeGreaterThanOrEqual(16);
    });

    it("Close button (×) triggers pause, immediately returning to idle", async () => {
      const targetEl = document.createElement("button");
      targetEl.setAttribute("data-coreside-tour", "sidebar-new-chat");
      fixtureContainer.appendChild(targetEl);
      mockElementBounds(targetEl, { top: 40, left: 16, width: 200, height: 36 });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      const closeBtn = screen.getByRole("button", { name: "Close tour" });

      await act(async () => {
        fireEvent.click(closeBtn);
      });

      expect(useOnboardingStore.getState().phase).toBe("idle");
      const progress =
        useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("in_progress");

      // Verify progress and active step were persisted to backend DB
      const onboarding = await api.getOnboardingState();
      const persisted = onboarding.progress.find(
        (p) => p.tutorialId === ESSENTIALS_TUTORIAL_ID,
      );
      expect(persisted?.status).toBe("in_progress");
      expect(persisted?.currentStepId).toBe("sidebar-new-chat");
    });

    it("Skip button triggers skip, immediately returning to idle and marking status skipped", async () => {
      const targetEl = document.createElement("button");
      targetEl.setAttribute("data-coreside-tour", "sidebar-new-chat");
      fixtureContainer.appendChild(targetEl);
      mockElementBounds(targetEl, { top: 40, left: 16, width: 200, height: 36 });

      await useOnboardingStore.getState().startEssentials(false);
      render(<TutorialOverlay />);
      flushRaf();

      const skipBtn = screen.getByRole("button", { name: "Skip" });

      await act(async () => {
        fireEvent.click(skipBtn);
      });

      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);
      const progress =
        useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("skipped");
    });
  });
});
