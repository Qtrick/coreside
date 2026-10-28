import { getContextualTip } from "@/lib/onboarding/tutorials";
import { usePresence } from "@/lib/motion/usePresence";
import { useOnboardingStore } from "@/stores/onboarding-store";

/** Nonmodal single teaching hint — never stacks with the core tour or onboarding. */
export function ContextualHint() {
  const tipId = useOnboardingStore((s) => s.activeContextualId);
  const dismiss = useOnboardingStore((s) => s.dismissContextual);
  const tip = tipId ? getContextualTip(tipId) : undefined;
  const { mounted, phase, onEnterComplete, onExitComplete, reducedMotion } =
    usePresence(Boolean(tip));

  if (!mounted || !tip) return null;

  return (
    <aside
      className={`contextual-hint presence-${phase}${reducedMotion ? " reduced-motion" : ""}`}
      role="region"
      aria-labelledby="contextual-hint-title"
      aria-describedby="contextual-hint-body"
      data-testid="contextual-hint"
      onTransitionEnd={(e) => {
        if (e.target !== e.currentTarget) return;
        if (e.propertyName !== "opacity") return;
        if (phase === "entering") onEnterComplete();
        if (phase === "exiting") onExitComplete();
      }}
    >
      <div className="sr-only" role="status" aria-live="polite">
        {tip.title}
      </div>
      <header className="contextual-hint-header">
        <strong id="contextual-hint-title" className="contextual-hint-title">
          {tip.title}
        </strong>
        <button
          type="button"
          className="btn-icon contextual-hint-close"
          aria-label="Dismiss hint"
          data-testid="contextual-hint-close"
          onClick={() => void dismiss()}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              void dismiss();
            }
          }}
        >
          <span aria-hidden="true">×</span>
        </button>
      </header>
      <p id="contextual-hint-body" className="contextual-hint-body">
        {tip.body}
      </p>
      <footer className="contextual-hint-actions">
        <button
          type="button"
          className="btn btn-secondary btn-compact"
          data-testid="contextual-hint-got-it"
          onClick={() => void dismiss()}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              void dismiss();
            }
          }}
        >
          Got it
        </button>
      </footer>
    </aside>
  );
}
