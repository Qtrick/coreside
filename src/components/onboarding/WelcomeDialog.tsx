import { useState, useEffect } from "react";
import { ModalPortal } from "@/components/ui/ModalPortal";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { useAppStore } from "@/stores/app-store";
import { AppAppearancePreview } from "./AppAppearancePreview";
import type { ThemePreference } from "@/types/agent";

export function WelcomeDialog() {
  const phase = useOnboardingStore((s) => s.phase);
  const goToAppearance = useOnboardingStore((s) => s.goToAppearance);
  const goToWelcome = useOnboardingStore((s) => s.goToWelcome);
  const completeOnboarding = useOnboardingStore((s) => s.completeOnboarding);
  const dismissWelcome = useOnboardingStore((s) => s.dismissWelcome);
  const startEssentials = useOnboardingStore((s) => s.startEssentials);

  const currentTheme = useAppStore((s) => s.theme);
  const resolvedTheme = useAppStore((s) => s.resolvedTheme);
  const setTheme = useAppStore((s) => s.setTheme);

  // Local draft selection - provisional only, does not persist immediately
  const [draftTheme, setDraftTheme] = useState<ThemePreference>(currentTheme ?? "system");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Sync draftTheme if store theme changes externally
  useEffect(() => {
    if (currentTheme) {
      setDraftTheme(currentTheme);
    }
  }, [currentTheme]);

  const THEME_OPTIONS: readonly ThemePreference[] = ["light", "dark", "system"];

  const handleRadioKeyDown = (
    e: React.KeyboardEvent<HTMLDivElement>,
    currentIndex: number,
  ) => {
    let nextIndex = -1;
    if (e.key === "ArrowDown" || e.key === "ArrowRight") {
      e.preventDefault();
      nextIndex = (currentIndex + 1) % THEME_OPTIONS.length;
    } else if (e.key === "ArrowUp" || e.key === "ArrowLeft") {
      e.preventDefault();
      nextIndex = (currentIndex - 1 + THEME_OPTIONS.length) % THEME_OPTIONS.length;
    } else if (e.key === "Home") {
      e.preventDefault();
      nextIndex = 0;
    } else if (e.key === "End") {
      e.preventDefault();
      nextIndex = THEME_OPTIONS.length - 1;
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setDraftTheme(THEME_OPTIONS[currentIndex]);
      return;
    }

    if (nextIndex >= 0) {
      const nextTheme = THEME_OPTIONS[nextIndex];
      setDraftTheme(nextTheme);
      const targetEl = document.querySelector<HTMLDivElement>(
        `[data-testid="theme-option-${nextTheme}"]`,
      );
      targetEl?.focus();
    }
  };

  if (phase !== "welcome" && phase !== "appearance") return null;

  const handleConfirmAppearance = async () => {
    setSaving(true);
    setSaveError(null);
    try {
      // 1. Persist the selected preference through the existing setting API
      await setTheme(draftTheme);
      // 2. Mark onboarding completed in store and sqlite
      await completeOnboarding();
    } catch (err) {
      console.error("Failed to persist theme or complete onboarding", err);
      setSaveError(
        "Could not save preference. You can continue and change appearance anytime in Settings.",
      );
    } finally {
      setSaving(false);
    }
  };

  const isAppearanceStep = phase === "appearance";

  return (
    <ModalPortal onClose={() => void dismissWelcome()}>
      <div className="provider-modal-root" role="presentation">
        <button
          type="button"
          className="provider-modal-backdrop"
          aria-label="Close setup and explore on my own"
          tabIndex={-1}
          onClick={() => void dismissWelcome()}
        />
        <div
          className="onboarding-modal onboarding-welcome"
          role="dialog"
          aria-modal="true"
          aria-labelledby={isAppearanceStep ? "onboarding-appearance-title" : "onboarding-welcome-title"}
          aria-describedby="onboarding-step-desc"
          tabIndex={-1}
        >
          {/* Header */}
          <header className="onboarding-step-header">
            <div className="onboarding-step-indicator">
              {isAppearanceStep ? "02 · Appearance" : "01 · Welcome"}
            </div>
            <h2
              id={isAppearanceStep ? "onboarding-appearance-title" : "onboarding-welcome-title"}
              data-testid="onboarding-step-title"
            >
              {isAppearanceStep ? "Choose your appearance" : "Welcome to Coreside"}
            </h2>
            <p id="onboarding-step-desc">
              {isAppearanceStep
                ? "Select how Coreside looks. You can customize themes anytime in Settings."
                : "Create tools, organize your work, and build personal software through conversation."}
            </p>
          </header>

          {/* Body */}
          <div className="onboarding-step-body">
            {!isAppearanceStep ? (
              /* SCREEN 1: Welcome */
              <div className="onboarding-welcome-content">
                <div className="onboarding-welcome-illustration">
                  <div className="onboarding-welcome-feature-row">
                    <span className="onboarding-feature-badge" aria-hidden="true">
                      ✦
                    </span>
                    <div className="onboarding-feature-text">
                      <strong>Conversations become software</strong>
                      <span>
                        Ask Coreside to create trackers, planners, or utilities that stay in your
                        workspace.
                      </span>
                    </div>
                  </div>
                  <div className="onboarding-welcome-feature-row">
                    <span className="onboarding-feature-badge" aria-hidden="true">
                      🔒
                    </span>
                    <div className="onboarding-feature-text">
                      <strong>Local-first and private</strong>
                      <span>
                        Keys stay in your OS keychain. Your data and conversations stay on your
                        device.
                      </span>
                    </div>
                  </div>
                  <div className="onboarding-welcome-feature-row">
                    <span className="onboarding-feature-badge" aria-hidden="true">
                      ⚡
                    </span>
                    <div className="onboarding-feature-text">
                      <strong>Evolve over time</strong>
                      <span>
                        Refine your tools naturally. Coreside modifies existing views without
                        breaking your data.
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              /* SCREEN 2: Choose Appearance */
              <div className="onboarding-appearance-content">
                {saveError && (
                  <div
                    className="alert-banner alert-warning"
                    role="alert"
                    aria-live="assertive"
                    style={{ marginBottom: "0.75rem" }}
                  >
                    {saveError}
                  </div>
                )}
                <div
                  className="theme-preview-grid"
                  role="radiogroup"
                  aria-label="Choose application appearance"
                >
                  {/* Light Theme Option */}
                  <div
                    role="radio"
                    aria-checked={draftTheme === "light"}
                    tabIndex={0}
                    className={`theme-card-option ${draftTheme === "light" ? "selected" : ""}`}
                    onClick={() => setDraftTheme("light")}
                    onKeyDown={(e) => handleRadioKeyDown(e, 0)}
                    data-testid="theme-option-light"
                  >
                    {draftTheme === "light" && (
                      <span className="theme-card-check" aria-hidden="true">
                        ✓
                      </span>
                    )}
                    <AppAppearancePreview mode="light" />
                    <div className="theme-card-info">
                      <strong>Light</strong>
                      <span>Clean, high-contrast monochrome surfaces</span>
                    </div>
                  </div>

                  {/* Dark Theme Option */}
                  <div
                    role="radio"
                    aria-checked={draftTheme === "dark"}
                    tabIndex={0}
                    className={`theme-card-option ${draftTheme === "dark" ? "selected" : ""}`}
                    onClick={() => setDraftTheme("dark")}
                    onKeyDown={(e) => handleRadioKeyDown(e, 1)}
                    data-testid="theme-option-dark"
                  >
                    {draftTheme === "dark" && (
                      <span className="theme-card-check" aria-hidden="true">
                        ✓
                      </span>
                    )}
                    <AppAppearancePreview mode="dark" />
                    <div className="theme-card-info">
                      <strong>Dark</strong>
                      <span>Calm, focused workspace for deep work</span>
                    </div>
                  </div>

                  {/* System Theme Option */}
                  <div
                    role="radio"
                    aria-checked={draftTheme === "system"}
                    tabIndex={0}
                    className={`theme-card-option theme-system-option ${draftTheme === "system" ? "selected" : ""}`}
                    onClick={() => setDraftTheme("system")}
                    onKeyDown={(e) => handleRadioKeyDown(e, 2)}
                    data-testid="theme-option-system"
                  >
                    <div className="theme-card-info" style={{ margin: 0 }}>
                      <strong>System</strong>
                      <span>Follows macOS appearance (currently {resolvedTheme})</span>
                    </div>
                    {draftTheme === "system" && (
                      <span
                        className="theme-card-check"
                        style={{ position: "static" }}
                        aria-hidden="true"
                      >
                        ✓
                      </span>
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Footer Actions */}
          <footer className="onboarding-step-footer">
            {isAppearanceStep ? (
              <>
                <button
                  type="button"
                  className="btn btn-secondary"
                  data-action="back-to-welcome"
                  onClick={goToWelcome}
                >
                  Back
                </button>
                <div className="onboarding-step-footer-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    data-action="skip-setup"
                    onClick={() => void dismissWelcome()}
                  >
                    Skip setup
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    data-action="start-using-coreside"
                    disabled={saving}
                    onClick={() => void handleConfirmAppearance()}
                  >
                    {saving ? "Saving..." : "Start using Coreside"}
                  </button>
                </div>
              </>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn-secondary"
                  data-action="dismiss-welcome"
                  onClick={() => void dismissWelcome()}
                >
                  Skip setup
                </button>
                <div className="onboarding-step-footer-actions">
                  <button
                    type="button"
                    className="btn btn-secondary btn-ghost"
                    style={{ fontSize: "0.8rem", opacity: 0.8 }}
                    onClick={() => void startEssentials(false)}
                  >
                    Take the 3-minute tour
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    data-action="continue-onboarding"
                    onClick={goToAppearance}
                  >
                    Continue
                  </button>
                </div>
              </>
            )}
          </footer>
        </div>
      </div>
    </ModalPortal>
  );
}
