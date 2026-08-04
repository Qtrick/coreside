/** Onboarding / tutorial types (RC3.4). */

export type TutorialStatus =
  | "not_started"
  | "in_progress"
  | "completed"
  | "skipped"
  | "superseded";

export type TutorialStepKind =
  | "welcome"
  | "spotlight"
  | "dialog"
  | "completion";

export type TutorialStep = {
  id: string;
  title: string;
  body: string;
  /** Stable `data-coreside-tour` value when this step spotlights UI. */
  target?: string;
  kind: TutorialStepKind;
  /** Optional primary CTA label override. */
  nextLabel?: string;
};

export type TutorialDefinition = {
  id: string;
  version: number;
  title: string;
  description: string;
  /** Auto-offer on first run when eligible. */
  firstRun: boolean;
  /** Hidden from consumer Help unless developer mode. */
  developerOnly?: boolean;
  estimatedMinutes?: number;
  steps: TutorialStep[];
};

/** One-shot nonmodal teaching hint (first-use moments). */
export type ContextualTipDefinition = {
  id: string;
  version: number;
  title: string;
  body: string;
};

export type TutorialProgress = {
  tutorialId: string;
  tutorialVersion: number;
  status: TutorialStatus;
  currentStepId: string | null;
  completedStepIds: string[];
  startedAt: string | null;
  updatedAt: string;
  completedAt: string | null;
  skippedAt: string | null;
  lastOpenedAt: string | null;
};

export type OnboardingState = {
  welcomeEligible: boolean;
  onboardingDisabled: boolean;
  isSecondaryWindow: boolean;
  recoveryMode: boolean;
  hasMeaningfulActivity: boolean;
  progress: TutorialProgress[];
};

export type TutorialSampleSeed = {
  conversationId: string;
  toolId: string;
  created: boolean;
};

export type UpsertTutorialProgressInput = {
  tutorialId: string;
  tutorialVersion: number;
  status: TutorialStatus;
  currentStepId?: string | null;
  completedStepIds?: string[];
};

export type CoordinatorPhase =
  | "idle"
  | "welcome"
  | "tour"
  | "completed"
  | "skipped";

export type CoordinatorState = {
  phase: CoordinatorPhase;
  activeTutorialId: string | null;
  stepIndex: number;
  progressById: Record<string, TutorialProgress>;
};
