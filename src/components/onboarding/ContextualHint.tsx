import { getContextualTip } from "@/lib/onboarding/tutorials";
import { usePresence } from "@/lib/motion/usePresence";
import { useOnboardingStore } from "@/stores/onboarding-store";

/** Nonmodal single teaching hint — never stacks with the core tour. */
export function ContextualHint() {
  const tipId = useOnboardingStore((s) => s.activeContextualId);
  const dismiss = useOnboardingStore((s) => s.dismissContextual);
  const tip = tipId ? getContextualTip(tipId) : undefined;
  const { mounted, phase, onEnterComplete, onExitComplete, reducedMotion } =
    usePresence(Boolean(tip));

  if (!mounted || !tip) return null;

  return (
    <div
      className={`contextual-hint presence-${phase}${reducedMotion ? " reduced-motion" : ""}`}
      role="region"
      aria-labelledby="contextual-hint-title"
      aria-describedby="contextual-hint-body"
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
      <div className="contextual-hint-body">
        <strong id="contextual-hint-title">{tip.title}</strong>
        <p id="contextual-hint-body">{tip.body}</p>
      </div>
      <button
        type="button"
        className="btn btn-secondary btn-compact"
        onClick={() => void dismiss()}
      >
        Got it
      </button>
      <button
        type="button"
        className="btn-icon contextual-hint-close"
        aria-label="Dismiss hint"
        onClick={() => void dismiss()}
      >
        ×
      </button>
    </div>
  );
}
