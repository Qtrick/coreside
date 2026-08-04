import { useEffect, useRef, useState } from "react";

export type PresencePhase = "entering" | "entered" | "exiting" | "exited";

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
  onExitComplete: () => void;
  reducedMotion: boolean;
} {
  const [phase, setPhase] = useState<PresencePhase>(open ? "entering" : "exited");
  const [mounted, setMounted] = useState(open);
  const generation = useRef(0);
  const reducedMotion = reducedMotionPreferred();

  useEffect(() => {
    const gen = ++generation.current;
    if (open) {
      setMounted(true);
      if (reducedMotion) {
        setPhase("entered");
        return;
      }
      setPhase("entering");
      const id = window.requestAnimationFrame(() => {
        if (generation.current === gen) setPhase("entered");
      });
      return () => window.cancelAnimationFrame(id);
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
  }, [open, reducedMotion]); // eslint-disable-line react-hooks/exhaustive-deps -- mounted gated intentionally

  const onExitComplete = () => {
    if (phase !== "exiting") return;
    setPhase("exited");
    setMounted(false);
  };

  return { mounted, phase, onExitComplete, reducedMotion };
}
