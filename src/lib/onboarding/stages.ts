/**
 * Architectural extension points for Coreside Onboarding Stages.
 * Supports dormant future stages (account sign-in, offline choice, sync)
 * while ensuring only active stages are exposed in consumer UI.
 *
 * Invariant: Dormant stages MUST have enabled: false and are strictly
 * excluded from rendered navigation, step counts, and completion gates.
 */

export type OnboardingStageId =
  | "welcome"
  | "appearance"
  | "account_sign_in"
  | "offline_choice"
  | "account_sync";

export type OnboardingStageDefinition = {
  id: OnboardingStageId;
  version: number;
  title: string;
  order: number;
  enabled: boolean;
  required: boolean;
  requiresNetwork: boolean;
  description: string;
};

export const ONBOARDING_STAGES: readonly OnboardingStageDefinition[] = [
  {
    id: "welcome",
    version: 1,
    title: "Welcome",
    order: 1,
    enabled: true,
    required: true,
    requiresNetwork: false,
    description: "Introduction to Coreside conversational software creation.",
  },
  {
    id: "appearance",
    version: 1,
    title: "Appearance",
    order: 2,
    enabled: true,
    required: false,
    requiresNetwork: false,
    description: "Interactive theme selection with live miniature application preview.",
  },
  // Future Dormant Stages (PLANNED / FOUNDATION — Never expose in current consumer UI)
  {
    id: "account_sign_in",
    version: 1,
    title: "Sign in",
    order: 3,
    enabled: false,
    required: false,
    requiresNetwork: true,
    description: "Future account authentication. Strictly disabled until backend sync lands.",
  },
  {
    id: "offline_choice",
    version: 1,
    title: "Continue without account",
    order: 4,
    enabled: false,
    required: false,
    requiresNetwork: false,
    description: "Explicit local-only session confirmation. Coreside is local-first by default.",
  },
  {
    id: "account_sync",
    version: 1,
    title: "Account sync",
    order: 5,
    enabled: false,
    required: false,
    requiresNetwork: true,
    description: "Future encrypted cross-device synchronization preferences.",
  },
] as const;

/**
 * Returns only the currently enabled, user-facing onboarding stages in display order.
 */
export function getActiveOnboardingStages(): OnboardingStageDefinition[] {
  return ONBOARDING_STAGES.filter((s) => s.enabled).sort((a, b) => a.order - b.order);
}

/**
 * Checks whether a given stage ID is currently active and eligible for UI display.
 */
export function isStageActive(stageId: OnboardingStageId): boolean {
  const stage = ONBOARDING_STAGES.find((s) => s.id === stageId);
  return Boolean(stage?.enabled);
}

