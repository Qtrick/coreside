import type { CoordinatorPhase, TutorialProgress } from "./types";
import { getContextualTip } from "./tutorials";

const TERMINAL = new Set(["completed", "skipped", "superseded"]);

export function isCoreTourBlocking(phase: CoordinatorPhase): boolean {
  return phase === "welcome" || phase === "tour";
}

export function isProgressTerminal(
  progressById: Record<string, TutorialProgress>,
  id: string,
): boolean {
  const row = progressById[id];
  return Boolean(row && TERMINAL.has(row.status));
}

/**
 * Eligibility for a single contextual tip.
 * Never stacks with the core welcome/tour, and never stacks another tip.
 */
export function canOfferContextualTip(input: {
  tipId: string;
  phase: CoordinatorPhase;
  onboardingDisabled: boolean;
  activeContextualId: string | null;
  progressById: Record<string, TutorialProgress>;
}): boolean {
  if (input.onboardingDisabled) return false;
  if (!getContextualTip(input.tipId)) return false;
  if (isCoreTourBlocking(input.phase)) return false;
  if (isProgressTerminal(input.progressById, input.tipId)) return false;
  if (input.activeContextualId && input.activeContextualId !== input.tipId) {
    return false;
  }
  return true;
}

/** Optional upgrade banner — not a full welcome. */
export function canShowWhatsNewBanner(input: {
  phase: CoordinatorPhase;
  onboardingDisabled: boolean;
  hasMeaningfulActivity: boolean;
  progressById: Record<string, TutorialProgress>;
  essentialsId: string;
  whatsNewId: string;
}): boolean {
  if (input.onboardingDisabled) return false;
  if (isCoreTourBlocking(input.phase)) return false;
  if (!input.hasMeaningfulActivity) return false;
  const essentials = input.progressById[input.essentialsId];
  if (!essentials || !TERMINAL.has(essentials.status)) return false;
  if (isProgressTerminal(input.progressById, input.whatsNewId)) return false;
  return true;
}
