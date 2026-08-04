import { storeSettingsCategory } from "@/lib/settings-categories";

/** Consumer empty-state copy — purpose + next action. Prefer "Apps" over "Tools". */
export const EMPTY_STATES = {
  noConversation: {
    title: "No conversation selected",
    body: "Start a new chat to begin building with Coreside.",
    primaryCta: "New chat",
  },
  emptyChat: {
    title: "Start a conversation",
    body: "Ask a question, or describe a goal — Coreside can explain ideas and build personal apps for you.",
    primaryCta: "Describe a goal",
  },
  chatLoadError: {
    title: "Conversation failed to load",
    primaryCta: "Open Help & learning",
  },
  noProjects: {
    title: "No projects yet",
    body: "Create a project to group related chats with shared context.",
    primaryCta: "Create project",
  },
  noAppOpen: {
    title: "No app open",
    body: "Ask Coreside to build an app, or select one from Apps in the sidebar.",
    primaryCta: "Build an app",
  },
  noMedia: {
    title: "No media yet",
    body: "Import an image or video to use as a wallpaper or project asset.",
    primaryCta: "Import media",
  },
  noAutomations: {
    title: "No automations yet",
    body: "Create a recurring routine in chat — Coreside can schedule safe changes while the app is open.",
    primaryCta: "Create automation",
  },
  sidebarNoChats: {
    hint: "No chats yet — use New chat",
  },
  sidebarNoApps: {
    hint: "No apps yet — ask in chat to build one",
  },
  helpLink: {
    label: "Open Help & learning",
  },
} as const;

/** Navigate to Settings → Help & learning via the existing category event. */
export function openHelpAndLearning(navigateToSettings: () => void): void {
  // Store + dispatch before navigation so a freshly mounted SettingsPanel
  // initializer and an already-open panel both land on Help & learning.
  storeSettingsCategory("help-learning");
  navigateToSettings();
  // Re-assert after navigate in case a remount raced the first store.
  storeSettingsCategory("help-learning");
}
