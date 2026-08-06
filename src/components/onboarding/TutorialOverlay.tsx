import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { useAppStore } from "@/stores/app-store";
import { activeStep, activeTutorial } from "@/lib/onboarding/coordinator";
import { TOUR_TARGETS } from "@/lib/onboarding/tutorials";
import { usePresence } from "@/lib/motion/usePresence";
import { storeSettingsCategory } from "@/lib/settings-categories";
import { OverlayPortal } from "@/components/ui/OverlayPortal";

type AnchorRect = { top: number; left: number; width: number; height: number };

function measureTarget(target?: string): AnchorRect | null {
  if (!target || typeof document === "undefined") return null;
  const el = document.querySelector<HTMLElement>(
    `[data-coreside-tour="${CSS.escape(target)}"]`,
  );
  if (!el) return null;
  const r = el.getBoundingClientRect();
  if (r.width < 2 && r.height < 2) return null;
  return { top: r.top, left: r.left, width: r.width, height: r.height };
}

export function TutorialOverlay() {
  const phase = useOnboardingStore((s) => s.phase);
  const stepIndex = useOnboardingStore((s) => s.stepIndex);
  const tutorial = useOnboardingStore((s) => activeTutorial(s));
  const step = useOnboardingStore((s) => activeStep(s));
  const next = useOnboardingStore((s) => s.next);
  const back = useOnboardingStore((s) => s.back);
  const skip = useOnboardingStore((s) => s.skip);
  const pause = useOnboardingStore((s) => s.pause);

  const popoverRef = useRef<HTMLDivElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const [anchor, setAnchor] = useState<AnchorRect | null>(null);
  const tourOpen = phase === "tour" && Boolean(tutorial && step);
  const { mounted, phase: presencePhase, reducedMotion } = usePresence(tourOpen);

  // Ensure Help & learning / AI connections nav exists before measuring tour targets.
  useLayoutEffect(() => {
    if (!tourOpen || !step) return;
    if (step.target === TOUR_TARGETS.helpLearning) {
      storeSettingsCategory("help-learning");
      useAppStore.getState().navigateToSettings();
    }
    if (step.target === TOUR_TARGETS.aiConnections) {
      storeSettingsCategory("ai-access");
      useAppStore.getState().navigateToSettings();
    }
  }, [tourOpen, step]);

  useLayoutEffect(() => {
    if (!tourOpen || !step) {
      setAnchor(null);
      return;
    }
    // Defer one frame so Settings can mount before measuring help-learning-nav.
    const id = window.requestAnimationFrame(() => {
      setAnchor(measureTarget(step.target));
    });
    return () => window.cancelAnimationFrame(id);
  }, [tourOpen, step]);

  useEffect(() => {
    if (!tourOpen) return;
    const reposition = () => setAnchor(measureTarget(step?.target));
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [tourOpen, step]);

  // Non-modal coach-mark (aria-modal=false): Escape pauses; do not trap Tab.
  useEffect(() => {
    if (!tourOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void pause();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tourOpen, pause]);

  useEffect(() => {
    if (!tourOpen) return;
    const frame = window.requestAnimationFrame(() => {
      primaryRef.current?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [tourOpen, stepIndex, step?.id]);

  if (!mounted || !tutorial || !step) return null;

  // Overlay tour skips the welcome dialog step in the progress denominator.
  const overlaySteps = tutorial.steps.filter((s) => s.kind !== "welcome");
  const displayIndex = Math.max(
    1,
    overlaySteps.findIndex((s) => s.id === step.id) + 1,
  );
  const displayTotal = Math.max(1, overlaySteps.length);
  const canBack =
    stepIndex > (tutorial.steps[0]?.kind === "welcome" ? 1 : 0);

  const popoverStyle: CSSProperties = (() => {
    const width = Math.min(340, Math.max(240, window.innerWidth - 32));
    const approxHeight = 220;
    const margin = 16;
    if (!anchor) {
      return {
        position: "fixed",
        top: "50%",
        left: "50%",
        transform: "translate(-50%, -50%)",
        width,
        maxWidth: `calc(100vw - ${margin * 2}px)`,
      };
    }
    const below = anchor.top + anchor.height + 12;
    const above = anchor.top - approxHeight - 12;
    const top =
      below + approxHeight <= window.innerHeight - margin
        ? Math.max(margin, below)
        : above >= margin
          ? Math.max(margin, above)
          : margin;
    const left = Math.min(
      window.innerWidth - width - margin,
      Math.max(margin, anchor.left),
    );
    return {
      position: "fixed",
      top,
      left,
      width,
      maxWidth: `calc(100vw - ${margin * 2}px)`,
    };
  })();

  // Spotlight coach-mark: dialog (not tooltip) so AT announces title/body and
  // focus stays on the tour controls. Backdrop remains non-blocking for targets.
  // Portal out of AppShell so the broad shell child rule cannot demote fixed stacking.
  return (
    <OverlayPortal>
    <div
      className={`tutorial-overlay presence-${presencePhase}${reducedMotion ? " reduced-motion" : ""}`}
      role="presentation"
    >
      {anchor ? (
        <div
          className="tutorial-spotlight"
          style={{
            top: anchor.top - 6,
            left: anchor.left - 6,
            width: anchor.width + 12,
            height: anchor.height + 12,
          }}
          aria-hidden
        />
      ) : null}
      <div
        ref={popoverRef}
        className={`tutorial-popover presence-${presencePhase}${reducedMotion ? " reduced-motion" : ""}`}
        style={popoverStyle}
        role="dialog"
        aria-modal="false"
        aria-labelledby="tutorial-step-title"
        aria-describedby="tutorial-step-body"
      >
        <div className="tutorial-popover-progress muted">
          {displayIndex} of {displayTotal}
        </div>
        <h3 id="tutorial-step-title">{step.title}</h3>
        <p id="tutorial-step-body">{step.body}</p>
        <div className="button-row tutorial-popover-actions">
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => void skip()}
          >
            Skip
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            disabled={!canBack}
            onClick={() => void back()}
          >
            Back
          </button>
          <button
            ref={primaryRef}
            type="button"
            className="btn btn-primary"
            onClick={() => void next()}
          >
            {step.nextLabel ?? (displayIndex >= displayTotal ? "Done" : "Next")}
          </button>
        </div>
        <button
          type="button"
          className="btn-icon tutorial-popover-close"
          aria-label="Pause tour"
          onClick={() => void pause()}
        >
          ×
        </button>
      </div>
    </div>
    </OverlayPortal>
  );
}
