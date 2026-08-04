import { describe, expect, it, vi } from "vitest";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { readStoredSettingsCategory } from "@/lib/settings-categories";

describe("EMPTY_STATES", () => {
  it("keeps consumer copy constants stable", () => {
    expect(EMPTY_STATES).toMatchInlineSnapshot(`
      {
        "chatLoadError": {
          "primaryCta": "Open Help & learning",
          "title": "Conversation failed to load",
        },
        "emptyChat": {
          "body": "Ask a question, or describe a goal — Coreside can explain ideas and build personal apps for you.",
          "primaryCta": "Describe a goal",
          "title": "Start a conversation",
        },
        "helpLink": {
          "label": "Open Help & learning",
        },
        "noAppOpen": {
          "body": "Ask Coreside to build an app, or select one from Apps in the sidebar.",
          "primaryCta": "Build an app",
          "title": "No app open",
        },
        "noAutomations": {
          "body": "Create a recurring routine in chat — Coreside can schedule safe changes while the app is open.",
          "primaryCta": "Create automation",
          "title": "No automations yet",
        },
        "noConversation": {
          "body": "Start a new chat to begin building with Coreside.",
          "primaryCta": "New chat",
          "title": "No conversation selected",
        },
        "noMedia": {
          "body": "Import an image or video to use as a wallpaper or project asset.",
          "primaryCta": "Import media",
          "title": "No media yet",
        },
        "noProjects": {
          "body": "Create a project to group related chats with shared context.",
          "primaryCta": "Create project",
          "title": "No projects yet",
        },
        "sidebarNoApps": {
          "hint": "No apps yet — ask in chat to build one",
        },
        "sidebarNoChats": {
          "hint": "No chats yet — use New chat",
        },
      }
    `);
  });

  it("prefers Apps over Tools in consumer copy", () => {
    const blob = JSON.stringify(EMPTY_STATES);
    expect(blob.toLowerCase()).not.toMatch(/\btools\b/);
    expect(blob).toMatch(/apps/i);
  });
});

describe("openHelpAndLearning", () => {
  it("stores help-learning before and after navigate", () => {
    const navigate = vi.fn(() => {
      expect(readStoredSettingsCategory()).toBe("help-learning");
    });
    openHelpAndLearning(navigate);
    expect(navigate).toHaveBeenCalledTimes(1);
    expect(readStoredSettingsCategory()).toBe("help-learning");
  });
});
