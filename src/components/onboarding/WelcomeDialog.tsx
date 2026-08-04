import { ModalPortal } from "@/components/ui/ModalPortal";
import { useOnboardingStore } from "@/stores/onboarding-store";

export function WelcomeDialog() {
  const phase = useOnboardingStore((s) => s.phase);
  const startEssentials = useOnboardingStore((s) => s.startEssentials);
  const dismissWelcome = useOnboardingStore((s) => s.dismissWelcome);

  if (phase !== "welcome") return null;

  return (
    <ModalPortal onClose={() => void dismissWelcome()}>
      <div className="provider-modal-root" role="presentation">
        <button
          type="button"
          className="provider-modal-backdrop"
          aria-label="Explore on my own"
          tabIndex={-1}
          onClick={() => void dismissWelcome()}
        />
        <div
          className="provider-modal onboarding-welcome"
          role="dialog"
          aria-modal="true"
          aria-labelledby="onboarding-welcome-title"
          aria-describedby="onboarding-welcome-desc"
          tabIndex={-1}
        >
          <header className="provider-modal-header">
            <div>
              <h2 id="onboarding-welcome-title">Welcome to Coreside</h2>
              <p id="onboarding-welcome-desc">
                Start with a calm chat, then grow personal apps beside your
                conversations. A short tour works offline — no provider required.
              </p>
            </div>
          </header>
          <div className="button-row provider-form-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void startEssentials(false)}
            >
              Take the 3-minute tour
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void dismissWelcome()}
            >
              Explore on my own
            </button>
          </div>
        </div>
      </div>
    </ModalPortal>
  );
}
