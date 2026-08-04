import type { ContextualTipDefinition, TutorialDefinition } from "./types";

/** Tour target ids — must match `data-coreside-tour` in the shell. */
export const TOUR_TARGETS = {
  composer: "chat-composer",
  composerSend: "chat-composer-send",
  appPanel: "app-panel",
  sidebarProjects: "sidebar-projects",
  helpLearning: "help-learning-nav",
} as const;

export const ESSENTIALS_TUTORIAL_ID = "coreside-essentials";

/** Persisted via `tutorial_progress` — dismissed What's new for this release slice. */
export const WHATS_NEW_ID = "whats-new:rc3.4";

export const CONTEXTUAL_TIP_IDS = {
  firstApp: "contextual-first-app",
  firstApproval: "contextual-first-approval",
  firstQueue: "contextual-first-queue",
  firstProject: "contextual-first-project",
  firstVersions: "contextual-first-versions",
} as const;

/** Single-shot first-use hints (not listed as Help tour modules). */
export const CONTEXTUAL_TIPS: readonly ContextualTipDefinition[] = [
  {
    id: CONTEXTUAL_TIP_IDS.firstApp,
    version: 1,
    title: "Your first app",
    body: "Apps open beside chat. Keep talking to refine them — changes can be previewed before they land.",
  },
  {
    id: CONTEXTUAL_TIP_IDS.firstApproval,
    version: 1,
    title: "Review before granting",
    body: "Apps ask before sensitive actions. Approve once, always, or deny — you stay in control.",
  },
  {
    id: CONTEXTUAL_TIP_IDS.firstQueue,
    version: 1,
    title: "Messages can wait in line",
    body: "While Coreside is busy, new messages queue here so you can keep typing without interrupting.",
  },
  {
    id: CONTEXTUAL_TIP_IDS.firstProject,
    version: 1,
    title: "Projects keep context together",
    body: "Group related chats under a project so Coreside remembers the topic across conversations.",
  },
  {
    id: CONTEXTUAL_TIP_IDS.firstVersions,
    version: 1,
    title: "History and versions",
    body: "Branches and snapshots live here. You can revisit earlier states without losing your place.",
  },
] as const;

export const TUTORIALS: readonly TutorialDefinition[] = [
  {
    id: ESSENTIALS_TUTORIAL_ID,
    version: 1,
    title: "Coreside essentials",
    description: "A short tour of chat, apps, and review.",
    firstRun: true,
    estimatedMinutes: 3,
    steps: [
      {
        id: "welcome",
        title: "Welcome to Coreside",
        body: "Coreside starts as a calm chat — and can grow into personal apps beside your conversations. This short tour works offline.",
        kind: "welcome",
        nextLabel: "Start tour",
      },
      {
        id: "chat-composer",
        title: "Ask in chat",
        body: "Type here to talk with Coreside. Mentions and attachments appear in this composer when you need them.",
        target: TOUR_TARGETS.composer,
        kind: "spotlight",
      },
      {
        id: "app-panel",
        title: "Apps live beside chat",
        body: "When you create or open an app, it appears in this panel. Your apps stay in the sidebar under Apps.",
        target: TOUR_TARGETS.appPanel,
        kind: "spotlight",
      },
      {
        id: "preview-review",
        title: "Preview before changes land",
        body: "When Coreside proposes a change, you can review it first. Nothing permanent happens without your say.",
        target: TOUR_TARGETS.composerSend,
        kind: "spotlight",
      },
      {
        id: "help-learning",
        title: "Help & learning",
        body: "Restart this tour anytime from Settings → Help & learning. You can also open advanced modules when you are ready.",
        target: TOUR_TARGETS.helpLearning,
        kind: "completion",
        nextLabel: "Finish",
      },
    ],
  },
  {
    id: "coreside-projects",
    version: 1,
    title: "Projects",
    description: "Organize chats and context by topic.",
    firstRun: false,
    estimatedMinutes: 2,
    steps: [
      {
        id: "projects-sidebar",
        title: "Projects in the sidebar",
        body: "Group related chats under a project so context stays together.",
        target: TOUR_TARGETS.sidebarProjects,
        kind: "spotlight",
      },
    ],
  },
  {
    id: "coreside-permissions",
    version: 1,
    title: "Permissions",
    description: "How apps request access and what you approve.",
    firstRun: false,
    estimatedMinutes: 2,
    steps: [
      {
        id: "permissions-overview",
        title: "You stay in control",
        body: "Generated apps cannot reach secrets, the filesystem, or unprotected commands. Approvals stay visible.",
        kind: "dialog",
      },
    ],
  },
  {
    id: "coreside-versions",
    version: 1,
    title: "Versions",
    description: "Undo and history for app changes.",
    firstRun: false,
    estimatedMinutes: 2,
    steps: [
      {
        id: "versions-overview",
        title: "Changes are versioned",
        body: "App updates keep history so you can undo when something goes wrong.",
        kind: "dialog",
      },
    ],
  },
  {
    id: "coreside-shortcuts",
    version: 1,
    title: "Shortcuts",
    description: "Keyboard shortcuts for power users.",
    firstRun: false,
    developerOnly: true,
    estimatedMinutes: 1,
    steps: [
      {
        id: "shortcuts-palette",
        title: "Command palette",
        body: "Press ⌘K / Ctrl+K to open the command palette for navigation and tours.",
        kind: "dialog",
      },
    ],
  },
] as const;

export function getTutorial(id: string): TutorialDefinition | undefined {
  return TUTORIALS.find((t) => t.id === id);
}

export function getContextualTip(id: string): ContextualTipDefinition | undefined {
  return CONTEXTUAL_TIPS.find((t) => t.id === id);
}

export function listConsumerTutorials(developerMode: boolean): TutorialDefinition[] {
  return TUTORIALS.filter((t) => developerMode || !t.developerOnly);
}
