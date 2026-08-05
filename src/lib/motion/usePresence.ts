import { useEffect, useRef, useState } from "react";

export type PresencePhase = "entering" | "entered" | "exiting" | "exited";

/** Matches --motion-duration-panel (220ms) with a small completion margin. */
const PANEL_MOTION_MS = 250;

function reducedMotionPreferred(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/**
 * Keep a component mounted through exit animation.
 * Rapid reopen cancels a stale exit and returns to entered/entering.
 */
export function usePresence(open: boolean): {
  mounted: boolean;
  phase: PresencePhase;
  onEnterComplete: () => void;
  onExitComplete: () => void;
  reducedMotion: boolean;
} {
  const [phase, setPhase] = useState<PresencePhase>(open ? "entering" : "exited");
  const [mounted, setMounted] = useState(open);
  const generation = useRef(0);
  const enterTimeoutRef = useRef<number | null>(null);
  const exitTimeoutRef = useRef<number | null>(null);
  const [reducedMotion, setReducedMotion] = useState(reducedMotionPreferred());

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReducedMotion(mq.matches);
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    const gen = ++generation.current;
    if (open) {
      setMounted(true);
      if (reducedMotion) {
        setPhase("entered");
        return;
      }
      setPhase("entering");
      const finishEnter = () => {
        if (generation.current !== gen) return;
        enterTimeoutRef.current = null;
        setPhase("entered");
      };
      const id = window.requestAnimationFrame(finishEnter);
      enterTimeoutRef.current = window.setTimeout(finishEnter, PANEL_MOTION_MS);
      return () => {
        window.cancelAnimationFrame(id);
        if (enterTimeoutRef.current !== null) {
          window.clearTimeout(enterTimeoutRef.current);
          enterTimeoutRef.current = null;
        }
      };
    }
    if (!mounted) {
      setPhase("exited");
      return;
    }
    if (reducedMotion) {
      setPhase("exited");
      setMounted(false);
      return;
    }
    setPhase("exiting");
    exitTimeoutRef.current = window.setTimeout(() => {
      exitTimeoutRef.current = null;
      if (generation.current !== gen) return;
      setPhase("exited");
      setMounted(false);
    }, PANEL_MOTION_MS);
    return () => {
      if (exitTimeoutRef.current !== null) {
        window.clearTimeout(exitTimeoutRef.current);
        exitTimeoutRef.current = null;
      }
    };
  }, [open, reducedMotion]); // eslint-disable-line react-hooks/exhaustive-deps -- mounted gated intentionally

  const onEnterComplete = () => {
    if (phase !== "entering") return;
    if (enterTimeoutRef.current !== null) {
      window.clearTimeout(enterTimeoutRef.current);
      enterTimeoutRef.current = null;
    }
    setPhase("entered");
  };

  const onExitComplete = () => {
    if (phase !== "exiting") return;
    if (exitTimeoutRef.current !== null) {
      window.clearTimeout(exitTimeoutRef.current);
      exitTimeoutRef.current = null;
    }
    setPhase("exited");
    setMounted(false);
  };

  return { mounted, phase, onEnterComplete, onExitComplete, reducedMotion };
}
