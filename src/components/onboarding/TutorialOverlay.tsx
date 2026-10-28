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

function measureElement(el: HTMLElement | null): AnchorRect | null {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  if (r.width < 2 && r.height < 2) return null;
  return { top: r.top, left: r.left, width: r.width, height: r.height };
}

function resolveTargetElement(
  target?: string,
  fallbackTarget?: string,
): HTMLElement | null {
  if (typeof document === "undefined") return null;
  if (target) {
    const el = document.querySelector<HTMLElement>(
      `[data-coreside-tour="${CSS.escape(target)}"]`,
    );
    if (el) {
      const r = el.getBoundingClientRect();
      if (r.width >= 2 && r.height >= 2) return el;
    }
  }
  if (fallbackTarget) {
    const el = document.querySelector<HTMLElement>(
      `[data-coreside-tour="${CSS.escape(fallbackTarget)}"]`,
    );
    if (el) {
      const r = el.getBoundingClientRect();
      if (r.width >= 2 && r.height >= 2) return el;
    }
  }
  return null;
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
  const [popoverSize, setPopoverSize] = useState<{ width: number; height: number }>({
    width: 320,
    height: 200,
  });
  const tourOpen = phase === "tour" && Boolean(tutorial && step);
  const { mounted, phase: presencePhase, reducedMotion } = usePresence(tourOpen);

  // Auto-navigate to Settings when step requires settings tabs
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

  // Target readiness and bounded measurement loop
  useLayoutEffect(() => {
    if (!tourOpen || !step) {
      setAnchor(null);
      return;
    }

    let frameCount = 0;
    const maxFrames = 30; // ~500ms max
    let frameId: number;
    let observer: ResizeObserver | null = null;

    const checkAndMeasure = () => {
      const el = resolveTargetElement(step.target, step.fallbackTarget);
      if (el) {
        setAnchor(measureElement(el));
        if (typeof ResizeObserver !== "undefined") {
          observer = new ResizeObserver(() => {
            setAnchor(measureElement(el));
          });
          observer.observe(el);
        }
        return;
      }
      frameCount++;
      if (frameCount < maxFrames) {
        frameId = window.requestAnimationFrame(checkAndMeasure);
      } else {
        // Target absent after bounded check: fall back gracefully to centered dialog
        setAnchor(null);
      }
    };

    frameId = window.requestAnimationFrame(checkAndMeasure);

    return () => {
      window.cancelAnimationFrame(frameId);
      observer?.disconnect();
    };
  }, [tourOpen, stepIndex, step]);

  // Update measurements on window resize and scroll
  useEffect(() => {
    if (!tourOpen || !step) return;
    const update = () => {
      const el = resolveTargetElement(step.target, step.fallbackTarget);
      setAnchor(measureElement(el));
    };
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [tourOpen, step]);

  // Measure actual popover dimensions via ResizeObserver
  useLayoutEffect(() => {
    const el = popoverRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const updateSize = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) {
        setPopoverSize({
          width: Math.round(r.width),
          height: Math.round(r.height),
        });
      }
    };
    updateSize();
    const ro = new ResizeObserver(updateSize);
    ro.observe(el);
    return () => ro.disconnect();
  }, [mounted, stepIndex]);

  // Keyboard navigation: Escape pauses/closes; Tab cycles within popover
  useEffect(() => {
    if (!tourOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void pause();
        return;
      }
      if (e.key === "Tab") {
        const popover = popoverRef.current;
        if (!popover) return;
        const focusables = Array.from(
          popover.querySelectorAll<HTMLElement>(
            'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
          ),
        ).filter((el) => {
          if (el.hasAttribute("disabled") || el.getAttribute("aria-hidden") === "true") {
            return false;
          }
          if (typeof window !== "undefined" && typeof window.getComputedStyle === "function") {
            const style = window.getComputedStyle(el);
            if (style.display === "none" || style.visibility === "hidden") return false;
          }
          return true;
        });
        if (focusables.length === 0) return;
        const first = focusables[0];
        const last = focusables[focusables.length - 1];
        const active = document.activeElement;
        // Only trap when focus is inside the popover
        if (!popover.contains(active)) return;
        if (e.shiftKey && active === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && active === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tourOpen, pause]);

  // Focus primary action on step change
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
    const width = Math.min(340, Math.max(260, window.innerWidth - 32));
    const popoverHeight = popoverSize.height || 200;
    const margin = 16;
    const gap = 12;

    if (!anchor) {
      return {
        position: "fixed",
        top: "50%",
        left: "50%",
        transform: "translate(-50%, -50%)",
        width,
        maxWidth: `calc(100vw - ${margin * 2}px)`,
        zIndex: 1102,
      };
    }

    // If target is in the sidebar (left area), prefer placing to the right of target
    const rightSide = anchor.left + anchor.width + gap;
    if (anchor.left < 300 && rightSide + width <= window.innerWidth - margin) {
      const top = Math.max(
        margin,
        Math.min(window.innerHeight - popoverHeight - margin, anchor.top),
      );
      return {
        position: "fixed",
        top,
        left: rightSide,
        width,
        maxWidth: `calc(100vw - ${margin * 2}px)`,
        zIndex: 1102,
      };
    }

    // Otherwise evaluate below vs above
    const below = anchor.top + anchor.height + gap;
    const above = anchor.top - popoverHeight - gap;
    const maxTop = Math.max(margin, window.innerHeight - popoverHeight - margin);
    const top =
      below + popoverHeight <= window.innerHeight - margin
        ? Math.max(margin, below)
        : above >= margin
          ? Math.max(margin, Math.min(maxTop, above))
          : margin;

    const maxLeft = Math.max(margin, window.innerWidth - width - margin);
    const left = Math.max(
      margin,
      Math.min(maxLeft, anchor.left),
    );

    return {
      position: "fixed",
      top,
      left,
      width,
      maxWidth: `calc(100vw - ${margin * 2}px)`,
      zIndex: 1102,
    };
  })();

  const pad = 6;
  const t = anchor ? anchor.top - pad : 0;
  const l = anchor ? anchor.left - pad : 0;
  const w = anchor ? anchor.width + 2 * pad : 0;
  const h = anchor ? anchor.height + 2 * pad : 0;
  const vw = typeof window !== "undefined" ? window.innerWidth : 1200;
  const vh = typeof window !== "undefined" ? window.innerHeight : 800;

  const bandTop = Math.max(0, t);
  const bandBottom = Math.min(vh, Math.max(0, t + h));
  const bandHeight = Math.max(0, bandBottom - bandTop);

  return (
    <OverlayPortal>
      <div
        className={`tutorial-overlay presence-${presencePhase}${reducedMotion ? " reduced-motion" : ""}`}
        role="presentation"
      >
        {anchor ? (
          <>
            <div
              className="tutorial-scrim tutorial-scrim-top"
              style={{
                position: "fixed",
                top: 0,
                left: 0,
                width: "100vw",
                height: bandTop,
              }}
              aria-hidden
            />
            <div
              className="tutorial-scrim tutorial-scrim-bottom"
              style={{
                position: "fixed",
                top: bandBottom,
                left: 0,
                width: "100vw",
                height: Math.max(0, vh - bandBottom),
              }}
              aria-hidden
            />
            <div
              className="tutorial-scrim tutorial-scrim-left"
              style={{
                position: "fixed",
                top: bandTop,
                left: 0,
                width: Math.max(0, l),
                height: bandHeight,
              }}
              aria-hidden
            />
            <div
              className="tutorial-scrim tutorial-scrim-right"
              style={{
                position: "fixed",
                top: bandTop,
                left: Math.min(vw, l + w),
                width: Math.max(0, vw - (l + w)),
                height: bandHeight,
              }}
              aria-hidden
            />
            <div
              className="tutorial-spotlight tutorial-spotlight-ring"
              style={{
                top: t,
                left: l,
                width: w,
                height: h,
              }}
              aria-hidden
            />
          </>
        ) : (
          <div
            className="tutorial-scrim tutorial-scrim-fullscreen"
            style={{ position: "fixed", inset: 0 }}
            aria-hidden
          />
        )}
        <div
          className="tutorial-popover-positioner"
          style={popoverStyle}
        >
          <div
            ref={popoverRef}
            className={`tutorial-popover presence-${presencePhase}${reducedMotion ? " reduced-motion" : ""}`}
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
              aria-label="Close tour"
              onClick={() => void pause()}
            >
              ×
            </button>
          </div>
        </div>
      </div>
    </OverlayPortal>
  );
}
