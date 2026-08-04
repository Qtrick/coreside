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
import { storeSettingsCategory } from "@/lib/settings-categories";

type AnchorRect = { top: number; left: number; width: number; height: number };

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

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
  const reducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

  // Ensure Help & learning nav exists before measuring the completion step.
  useLayoutEffect(() => {
    if (phase !== "tour" || !step) return;
    if (step.target === TOUR_TARGETS.helpLearning) {
      storeSettingsCategory("help-learning");
      useAppStore.getState().navigateToSettings();
    }
  }, [phase, step?.id, step?.target]);

  useLayoutEffect(() => {
    if (phase !== "tour" || !step) {
      setAnchor(null);
      return;
    }
    // Defer one frame so Settings can mount before measuring help-learning-nav.
    const id = window.requestAnimationFrame(() => {
      setAnchor(measureTarget(step.target));
    });
    return () => window.cancelAnimationFrame(id);
  }, [phase, step, stepIndex]);

  useEffect(() => {
    if (phase !== "tour") return;
    const reposition = () => setAnchor(measureTarget(step?.target));
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [phase, step]);

  useEffect(() => {
    if (phase !== "tour") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void pause();
        return;
      }
      if (e.key !== "Tab" || !popoverRef.current) return;
      const items = Array.from(
        popoverRef.current.querySelectorAll<HTMLElement>(FOCUSABLE),
      ).filter((el) => el.offsetParent !== null || el === document.activeElement);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (!popoverRef.current.contains(active)) {
        e.preventDefault();
        first.focus();
      } else if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [phase, pause]);

  useEffect(() => {
    if (phase !== "tour") return;
    const frame = window.requestAnimationFrame(() => {
      primaryRef.current?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [phase, stepIndex, step?.id]);

  if (phase !== "tour" || !tutorial || !step) return null;

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
  return (
    <div
      className={`tutorial-overlay${reducedMotion ? " reduced-motion" : ""}`}
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
        className="tutorial-popover"
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
  );
}
