import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WelcomeDialog } from "./WelcomeDialog";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { useAppStore } from "@/stores/app-store";
import { api, __resetMockDb } from "@/lib/tauri";
import { emptyCoordinatorState } from "@/lib/onboarding/coordinator";
import { ESSENTIALS_TUTORIAL_ID } from "@/lib/onboarding/tutorials";

describe("ShortOnboarding Flow (Welcome & Appearance)", () => {
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

    // Reset app store appearance state
    useAppStore.setState({
      theme: "system",
      resolvedTheme: "dark",
    });

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

    // Clean any prior modal portals attached to body
    const priorBackdrops = document.querySelectorAll(".provider-modal-root, .provider-modal-backdrop");
    priorBackdrops.forEach((el) => el.remove());
  });

  afterEach(() => {
    const portals = document.querySelectorAll(".provider-modal-root, .provider-modal-backdrop");
    portals.forEach((el) => el.remove());
    vi.restoreAllMocks();
  });

  describe("Screen 1: Welcome presentation and transitions", () => {
    it("renders Screen 1 for fresh profiles with indicator '01 · Welcome' and title 'Welcome to Coreside'", async () => {
      await useOnboardingStore.getState().hydrate();
      expect(useOnboardingStore.getState().welcomeEligible).toBe(true);

      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("welcome");

      render(<WelcomeDialog />);

      // Indicator and title
      const indicator = document.querySelector(".onboarding-step-indicator");
      expect(indicator).toHaveTextContent("01 · Welcome");

      const title = screen.getByRole("heading", { name: "Welcome to Coreside" });
      expect(title).toBeInTheDocument();

      const desc = document.getElementById("onboarding-step-desc");
      expect(desc).toHaveTextContent(
        "Create tools, organize your work, and build personal software through conversation.",
      );

      // Feature value props
      expect(screen.getByText("Conversations become software")).toBeInTheDocument();
      expect(screen.getByText("Local-first and private")).toBeInTheDocument();
      expect(screen.getByText("Evolve over time")).toBeInTheDocument();

      // Action buttons
      expect(screen.getByRole("button", { name: "Continue" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Skip setup" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /Take the 3-minute tour/i })).toBeInTheDocument();
    });

    it("transitions to Screen 2 (Appearance) when clicking 'Continue'", async () => {
      await useOnboardingStore.getState().openWelcomeIfEligible();
      render(<WelcomeDialog />);

      const continueBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="continue-onboarding"]',
      )!;
      expect(continueBtn).toBeInTheDocument();

      await act(async () => {
        fireEvent.click(continueBtn);
      });

      // State transitions to appearance phase
      expect(useOnboardingStore.getState().phase).toBe("appearance");

      // Screen 2 content is rendered
      const indicator = document.querySelector(".onboarding-step-indicator");
      expect(indicator).toHaveTextContent("02 · Appearance");

      const title = screen.getByRole("heading", { name: "Choose your appearance" });
      expect(title).toBeInTheDocument();

      const desc = document.getElementById("onboarding-step-desc");
      expect(desc).toHaveTextContent(
        "Select how Coreside looks. You can customize themes anytime in Settings.",
      );
    });

    it("transitions back to Screen 1 when clicking 'Back' on Screen 2", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      render(<WelcomeDialog />);

      expect(document.querySelector(".onboarding-step-indicator")).toHaveTextContent("02 · Appearance");

      const backBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="back-to-welcome"]',
      )!;
      expect(backBtn).toBeInTheDocument();

      await act(async () => {
        fireEvent.click(backBtn);
      });

      expect(useOnboardingStore.getState().phase).toBe("welcome");
      expect(document.querySelector(".onboarding-step-indicator")).toHaveTextContent("01 · Welcome");
      expect(screen.getByRole("heading", { name: "Welcome to Coreside" })).toBeInTheDocument();
    });
  });

  describe("Skip setup and dismissal behavior", () => {
    it("synchronously dismisses setup on Screen 1, sets phase to idle, and persists status skipped without starting tutorial", async () => {
      await useOnboardingStore.getState().openWelcomeIfEligible();
      render(<WelcomeDialog />);

      const skipBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="dismiss-welcome"]',
      )!;

      await act(async () => {
        fireEvent.click(skipBtn);
      });

      // Synchronously idle
      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);
      expect(useOnboardingStore.getState().activeTutorialId).toBeNull();
      expect(useOnboardingStore.getState().stepIndex).toBe(0);

      // Persisted status is skipped
      const progress = useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("skipped");
    });

    it("synchronously dismisses setup on Screen 2, sets phase to idle, and persists status skipped without starting tutorial", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      render(<WelcomeDialog />);

      const skipBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="skip-setup"]',
      )!;

      await act(async () => {
        fireEvent.click(skipBtn);
      });

      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);
      expect(useOnboardingStore.getState().activeTutorialId).toBeNull();
      expect(useOnboardingStore.getState().stepIndex).toBe(0);

      const progress = useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("skipped");
    });

    it("dismisses setup and sets phase to idle when clicking modal backdrop", async () => {
      await useOnboardingStore.getState().openWelcomeIfEligible();
      render(<WelcomeDialog />);

      const backdrop = document.querySelector<HTMLButtonElement>(".provider-modal-backdrop")!;
      expect(backdrop).toBeInTheDocument();

      await act(async () => {
        fireEvent.click(backdrop);
      });

      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID]?.status).toBe("skipped");
    });
  });

  describe("Theme options and local draft isolation", () => {
    it("renders theme options for Light, Dark, and System", () => {
      useOnboardingStore.setState({ phase: "appearance" });
      render(<WelcomeDialog />);

      const radiogroup = screen.getByRole("radiogroup", { name: "Choose application appearance" });
      expect(radiogroup).toBeInTheDocument();

      const lightOption = screen.getByTestId("theme-option-light");
      const darkOption = screen.getByTestId("theme-option-dark");
      const systemOption = screen.getByTestId("theme-option-system");

      expect(lightOption).toBeInTheDocument();
      expect(darkOption).toBeInTheDocument();
      expect(systemOption).toBeInTheDocument();

      expect(lightOption).toHaveTextContent("Light");
      expect(lightOption).toHaveTextContent("Clean, high-contrast monochrome surfaces");

      expect(darkOption).toHaveTextContent("Dark");
      expect(darkOption).toHaveTextContent("Calm, focused workspace for deep work");

      expect(systemOption).toHaveTextContent("System");
      expect(systemOption).toHaveTextContent("Follows macOS appearance");
    });

    it("clicking Light or Dark selects in draft state and updates radio checked state without prematurely calling api.setSetting or modifying app-store theme", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      useAppStore.setState({ theme: "system" });

      const setSettingSpy = vi.spyOn(api, "setSetting");
      render(<WelcomeDialog />);

      const lightOption = screen.getByTestId("theme-option-light");
      const darkOption = screen.getByTestId("theme-option-dark");
      const systemOption = screen.getByTestId("theme-option-system");

      // Initial draft is system
      expect(systemOption).toHaveAttribute("aria-checked", "true");
      expect(lightOption).toHaveAttribute("aria-checked", "false");
      expect(darkOption).toHaveAttribute("aria-checked", "false");

      // Select Dark in draft
      await act(async () => {
        fireEvent.click(darkOption);
      });

      expect(darkOption).toHaveAttribute("aria-checked", "true");
      expect(darkOption.classList.contains("selected")).toBe(true);
      expect(lightOption).toHaveAttribute("aria-checked", "false");
      expect(systemOption).toHaveAttribute("aria-checked", "false");

      // Crucial requirement: api.setSetting was NOT called yet, store committed theme unchanged
      expect(setSettingSpy).not.toHaveBeenCalledWith("theme", expect.anything());
      expect(useAppStore.getState().theme).toBe("system");

      // Select Light in draft
      await act(async () => {
        fireEvent.click(lightOption);
      });

      expect(lightOption).toHaveAttribute("aria-checked", "true");
      expect(lightOption.classList.contains("selected")).toBe(true);
      expect(darkOption).toHaveAttribute("aria-checked", "false");
      expect(systemOption).toHaveAttribute("aria-checked", "false");

      // Still NOT prematurely persisted
      expect(setSettingSpy).not.toHaveBeenCalledWith("theme", expect.anything());
      expect(useAppStore.getState().theme).toBe("system");
    });

    it("allows keyboard selection on theme options via Enter and Space keys", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      useAppStore.setState({ theme: "system" });
      render(<WelcomeDialog />);

      const lightOption = screen.getByTestId("theme-option-light");
      const darkOption = screen.getByTestId("theme-option-dark");
      const systemOption = screen.getByTestId("theme-option-system");

      // Press Enter on dark
      await act(async () => {
        fireEvent.keyDown(darkOption, { key: "Enter" });
      });
      expect(darkOption).toHaveAttribute("aria-checked", "true");

      // Press Space on light
      await act(async () => {
        fireEvent.keyDown(lightOption, { key: " " });
      });
      expect(lightOption).toHaveAttribute("aria-checked", "true");

      // Press Enter on system
      await act(async () => {
        fireEvent.keyDown(systemOption, { key: "Enter" });
      });
      expect(systemOption).toHaveAttribute("aria-checked", "true");
    });

    it("allows full arrow key, Home, and End navigation across theme options in radiogroup", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      useAppStore.setState({ theme: "light" });
      render(<WelcomeDialog />);

      const lightOption = screen.getByTestId("theme-option-light");
      const darkOption = screen.getByTestId("theme-option-dark");
      const systemOption = screen.getByTestId("theme-option-system");

      // Initially light
      expect(lightOption).toHaveAttribute("aria-checked", "true");

      // ArrowRight from light -> dark
      await act(async () => {
        fireEvent.keyDown(lightOption, { key: "ArrowRight" });
      });
      expect(darkOption).toHaveAttribute("aria-checked", "true");

      // ArrowDown from dark -> system
      await act(async () => {
        fireEvent.keyDown(darkOption, { key: "ArrowDown" });
      });
      expect(systemOption).toHaveAttribute("aria-checked", "true");

      // ArrowRight wraps around from system -> light
      await act(async () => {
        fireEvent.keyDown(systemOption, { key: "ArrowRight" });
      });
      expect(lightOption).toHaveAttribute("aria-checked", "true");

      // ArrowLeft wraps around backwards from light -> system
      await act(async () => {
        fireEvent.keyDown(lightOption, { key: "ArrowLeft" });
      });
      expect(systemOption).toHaveAttribute("aria-checked", "true");

      // ArrowUp from system -> dark
      await act(async () => {
        fireEvent.keyDown(systemOption, { key: "ArrowUp" });
      });
      expect(darkOption).toHaveAttribute("aria-checked", "true");

      // Home key jumps to first (light)
      await act(async () => {
        fireEvent.keyDown(darkOption, { key: "Home" });
      });
      expect(lightOption).toHaveAttribute("aria-checked", "true");

      // End key jumps to last (system)
      await act(async () => {
        fireEvent.keyDown(lightOption, { key: "End" });
      });
      expect(systemOption).toHaveAttribute("aria-checked", "true");
    });
  });

  describe("Confirming appearance and application entry", () => {
    it("clicking 'Start using Coreside' persists selected theme via setTheme, completes onboarding, sets phase to idle, and does NOT launch the 5-step tutorial", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      useAppStore.setState({ theme: "system" });

      const setThemeSpy = vi.spyOn(useAppStore.getState(), "setTheme");
      const completeOnboardingSpy = vi.spyOn(useOnboardingStore.getState(), "completeOnboarding");

      render(<WelcomeDialog />);

      // Select Dark in draft
      const darkOption = screen.getByTestId("theme-option-dark");
      await act(async () => {
        fireEvent.click(darkOption);
      });

      const startBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="start-using-coreside"]',
      )!;
      expect(startBtn).toBeInTheDocument();

      await act(async () => {
        fireEvent.click(startBtn);
      });

      // Verify theme persisted
      expect(setThemeSpy).toHaveBeenCalledWith("dark");
      expect(useAppStore.getState().theme).toBe("dark");

      // Verify onboarding completed
      expect(completeOnboardingSpy).toHaveBeenCalled();
      const progress = useOnboardingStore.getState().progressById[ESSENTIALS_TUTORIAL_ID];
      expect(progress?.status).toBe("completed");

      // Verify phase is idle and NOT tour
      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(useOnboardingStore.getState().activeTutorialId).toBeNull();
      expect(useOnboardingStore.getState().stepIndex).toBe(0);

      // Dialog is no longer visible
      expect(document.querySelector(".onboarding-modal")).toBeNull();
    });
  });

  describe("Returning user & relaunch checks", () => {
    it("does not show WelcomeDialog when essentials is already completed", async () => {
      await api.upsertTutorialProgress({
        tutorialId: ESSENTIALS_TUTORIAL_ID,
        tutorialVersion: 1,
        status: "completed",
      });

      await useOnboardingStore.getState().hydrate();
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);

      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("idle");

      const { container } = render(<WelcomeDialog />);
      expect(container.firstChild).toBeNull();
      expect(document.querySelector(".onboarding-modal")).toBeNull();
    });

    it("does not show WelcomeDialog when essentials is already skipped", async () => {
      await api.upsertTutorialProgress({
        tutorialId: ESSENTIALS_TUTORIAL_ID,
        tutorialVersion: 1,
        status: "skipped",
      });

      await useOnboardingStore.getState().hydrate();
      expect(useOnboardingStore.getState().welcomeEligible).toBe(false);

      await useOnboardingStore.getState().openWelcomeIfEligible();
      expect(useOnboardingStore.getState().phase).toBe("idle");

      const { container } = render(<WelcomeDialog />);
      expect(container.firstChild).toBeNull();
      expect(document.querySelector(".onboarding-modal")).toBeNull();
    });
  });

  describe("Error handling during appearance confirmation", () => {
    it("displays error banner if theme persistence fails and preserves uncorrupted application entry", async () => {
      useOnboardingStore.setState({ phase: "appearance" });
      useAppStore.setState({ theme: "system" });

      // Mock setTheme to throw an error
      vi.spyOn(useAppStore.getState(), "setTheme").mockRejectedValueOnce(
        new Error("Database write failed: disk full"),
      );

      render(<WelcomeDialog />);

      const startBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="start-using-coreside"]',
      )!;

      await act(async () => {
        fireEvent.click(startBtn);
      });

      // Error banner is displayed
      const alertBanner = screen.getByRole("alert");
      expect(alertBanner).toBeInTheDocument();
      expect(alertBanner).toHaveTextContent(
        "Could not save preference. You can continue and change appearance anytime in Settings.",
      );

      // Button returns from saving state
      expect(startBtn).not.toBeDisabled();
      expect(startBtn).toHaveTextContent("Start using Coreside");

      // Application entry remains intact - user can still click Skip setup to exit cleanly
      const skipBtn = document.querySelector<HTMLButtonElement>(
        'button[data-action="skip-setup"]',
      )!;

      await act(async () => {
        fireEvent.click(skipBtn);
      });

      expect(useOnboardingStore.getState().phase).toBe("idle");
      expect(document.querySelector(".onboarding-modal")).toBeNull();
    });
  });
});
