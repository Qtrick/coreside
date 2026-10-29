import { beforeEach, describe, expect, it } from "vitest";
import { useWorkspaceStore } from "./workspace-store";
import { useAppStore } from "./app-store";
import {
  createDefaultWorkspaceState,
  createDefaultWorkspaceTab,
  DEFAULT_WORKSPACE_PANE_ID,
  DEFAULT_WORKSPACE_TAB_ID,
} from "@/types/workspace";

const STORAGE_KEY = "coreside:workspace:session";

describe("useWorkspaceStore", () => {
  beforeEach(() => {
    localStorage.clear();
    useWorkspaceStore.setState(createDefaultWorkspaceState());
  });

  describe("Initial State and Defaults", () => {
    it("initializes with a single pane, default tab, and focused pane-1", () => {
      const state = useWorkspaceStore.getState();
      expect(state.layoutKind).toBe("single");
      expect(state.panes).toHaveLength(1);
      expect(state.panes[0].id).toBe(DEFAULT_WORKSPACE_PANE_ID);
      expect(state.panes[0].tabIds).toEqual([DEFAULT_WORKSPACE_TAB_ID]);
      expect(state.panes[0].activeTabId).toBe(DEFAULT_WORKSPACE_TAB_ID);
      expect(state.focusedPaneId).toBe(DEFAULT_WORKSPACE_PANE_ID);
      expect(state.tabs[DEFAULT_WORKSPACE_TAB_ID]).toBeDefined();
      expect(state.tabs[DEFAULT_WORKSPACE_TAB_ID].closable).toBe(false);
      expect(state.tabs[DEFAULT_WORKSPACE_TAB_ID].title).toBe("New chat");
      expect(state.splitRatios).toEqual([0.5, 0.5]);
    });

    it("creates standalone default tab and default state with expected fields", () => {
      const tab = createDefaultWorkspaceTab();
      expect(tab.id).toBe(DEFAULT_WORKSPACE_TAB_ID);
      expect(tab.kind).toBe("chat");
      expect(tab.conversationId).toBeNull();
      expect(tab.closable).toBe(false);

      const defaultState = createDefaultWorkspaceState();
      expect(defaultState.version).toBe(1);
      expect(defaultState.layoutKind).toBe("single");
      expect(defaultState.panes[0].id).toBe("pane-1");
    });
  });

  describe("Layout Switching & Pane Creation", () => {
    it("switches to split-vertical and creates pane-2 with a default tab", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      const state = useWorkspaceStore.getState();
      expect(state.layoutKind).toBe("split-vertical");
      expect(state.panes).toHaveLength(2);
      expect(state.panes[0].id).toBe("pane-1");
      expect(state.panes[1].id).toBe("pane-2");
      expect(state.panes[1].tabIds).toHaveLength(1);
      const pane2TabId = state.panes[1].tabIds[0];
      expect(state.tabs[pane2TabId]).toBeDefined();
      expect(state.tabs[pane2TabId].title).toBe("New chat");
    });

    it("switches to split-three and creates up to 3 panes", () => {
      useWorkspaceStore.getState().setLayout("split-three");

      const state = useWorkspaceStore.getState();
      expect(state.layoutKind).toBe("split-three");
      expect(state.panes).toHaveLength(3);
      expect(state.panes.map((p) => p.id)).toEqual(["pane-1", "pane-2", "pane-3"]);
    });

    it("switches to split-grid and creates up to 4 panes", () => {
      useWorkspaceStore.getState().setLayout("split-grid");

      const state = useWorkspaceStore.getState();
      expect(state.layoutKind).toBe("split-grid");
      expect(state.panes).toHaveLength(4);
      expect(state.panes.map((p) => p.id)).toEqual([
        "pane-1",
        "pane-2",
        "pane-3",
        "pane-4",
      ]);
    });

    it("retains created panes in memory when switching back to single layout", () => {
      useWorkspaceStore.getState().setLayout("split-grid");
      expect(useWorkspaceStore.getState().panes).toHaveLength(4);

      // Add a custom tab to pane-4
      useWorkspaceStore.getState().openTab(
        {
          kind: "chat",
          title: "Grid Tab 4",
          conversationId: "conv-grid-4",
        },
        "pane-4",
      );

      // Switch back to single layout
      useWorkspaceStore.getState().setLayout("single");
      expect(useWorkspaceStore.getState().layoutKind).toBe("single");
      expect(useWorkspaceStore.getState().panes).toHaveLength(4);

      // Verify pane-4 tab wasn't lost
      const pane4 = useWorkspaceStore.getState().panes.find((p) => p.id === "pane-4");
      expect(pane4?.tabIds.length).toBeGreaterThan(0);
    });

    it("ensurePane returns existing pane or creates new pane with fallback tab", () => {
      const existingPane = useWorkspaceStore.getState().ensurePane("pane-1");
      expect(existingPane.id).toBe("pane-1");
      expect(useWorkspaceStore.getState().panes).toHaveLength(1);

      const customPane = useWorkspaceStore.getState().ensurePane("pane-custom");
      expect(customPane.id).toBe("pane-custom");
      expect(useWorkspaceStore.getState().panes).toHaveLength(2);
      expect(customPane.tabIds).toHaveLength(1);
      expect(useWorkspaceStore.getState().tabs[customPane.tabIds[0]]).toBeDefined();
    });
  });

  describe("Focused Pane Management", () => {
    it("updates focusedPaneId and persists to storage", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");
      useWorkspaceStore.getState().setFocusedPane("pane-2");

      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-2");
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
      expect(stored.focusedPaneId).toBe("pane-2");
    });

    it("is a no-op when setting the same focusedPaneId", () => {
      const initial = useWorkspaceStore.getState();
      useWorkspaceStore.getState().setFocusedPane("pane-1");
      expect(useWorkspaceStore.getState()).toBe(initial);
    });
  });

  describe("Tab Opening & Duplicate Prevention", () => {
    it("replaces the initial empty placeholder tab when opening the first real chat", () => {
      const store = useWorkspaceStore.getState();
      expect(store.panes[0].tabIds).toEqual([DEFAULT_WORKSPACE_TAB_ID]);

      const tabId = store.openTab({
        kind: "chat",
        title: "Project Architecture Discussion",
        conversationId: "conv-arch-1",
      });

      const next = useWorkspaceStore.getState();
      expect(next.panes[0].tabIds).toEqual([tabId]);
      expect(next.panes[0].activeTabId).toBe(tabId);
      expect(next.tabs[DEFAULT_WORKSPACE_TAB_ID]).toBeUndefined(); // Replaced and cleaned up
      expect(next.tabs[tabId].title).toBe("Project Architecture Discussion");
      expect(next.tabs[tabId].conversationId).toBe("conv-arch-1");
    });

    it("appends subsequent tabs without replacing existing content", () => {
      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({
        kind: "chat",
        title: "First Chat",
        conversationId: "conv-1",
      });

      const tab2 = store.openTab({
        kind: "chat",
        title: "Second Chat",
        conversationId: "conv-2",
      });

      const next = useWorkspaceStore.getState();
      expect(next.panes[0].tabIds).toEqual([tab1, tab2]);
      expect(next.panes[0].activeTabId).toBe(tab2);
    });

    it("prevents duplicate tabs by activating the existing matching tab in the target pane", () => {
      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({
        kind: "chat",
        title: "Unique Conversation",
        conversationId: "conv-unique",
      });

      // Open a second tab so tab1 is no longer active
      store.openTab({
        kind: "chat",
        title: "Another Conversation",
        conversationId: "conv-other",
      });
      expect(useWorkspaceStore.getState().panes[0].activeTabId).not.toBe(tab1);

      // Attempt to re-open the exact same conversation in pane-1
      const returnedTabId = useWorkspaceStore.getState().openTab({
        kind: "chat",
        title: "Unique Conversation",
        conversationId: "conv-unique",
      });

      expect(returnedTabId).toBe(tab1);
      const next = useWorkspaceStore.getState();
      expect(next.panes[0].tabIds).toHaveLength(2); // No new tab was added
      expect(next.panes[0].activeTabId).toBe(tab1); // Existing tab was reactivated
    });

    it("allows the same conversation in different panes (multi-pane isolation)", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");

      const tabPane1 = useWorkspaceStore.getState().openTab(
        {
          kind: "chat",
          title: "Shared Discussion",
          conversationId: "conv-shared",
        },
        "pane-1",
      );

      const tabPane2 = useWorkspaceStore.getState().openTab(
        {
          kind: "chat",
          title: "Shared Discussion",
          conversationId: "conv-shared",
        },
        "pane-2",
      );

      expect(tabPane1).not.toBe(tabPane2);
      const state = useWorkspaceStore.getState();
      expect(state.panes[0].tabIds).toContain(tabPane1);
      expect(state.panes[1].tabIds).toContain(tabPane2);
    });

    it("preserves explicitly passed tab ID and closable flag", () => {
      const tabId = useWorkspaceStore.getState().openTab({
        id: "explicit-tab-id-99",
        kind: "app",
        title: "Custom Tool",
        toolId: "tool-custom",
        closable: false,
      });

      expect(tabId).toBe("explicit-tab-id-99");
      const tab = useWorkspaceStore.getState().tabs["explicit-tab-id-99"];
      expect(tab).toBeDefined();
      expect(tab.closable).toBe(false);
    });
  });

  describe("Tab Activation", () => {
    it("activates tab and switches focused pane", () => {
      useWorkspaceStore.getState().setLayout("split-vertical");
      const store = useWorkspaceStore.getState();

      const tab1 = store.openTab({ kind: "chat", title: "C1", conversationId: "c1" }, "pane-1");
      const tab2 = store.openTab({ kind: "chat", title: "C2", conversationId: "c2" }, "pane-1");

      expect(useWorkspaceStore.getState().panes[0].activeTabId).toBe(tab2);

      useWorkspaceStore.getState().activateTab(tab1, "pane-1");
      expect(useWorkspaceStore.getState().panes[0].activeTabId).toBe(tab1);
      expect(useWorkspaceStore.getState().focusedPaneId).toBe("pane-1");
    });

    it("is a no-op when activating already active tab", () => {
      const tabId = useWorkspaceStore.getState().openTab({
        kind: "chat",
        title: "Active Chat",
        conversationId: "c-active",
      });

      const before = useWorkspaceStore.getState();
      useWorkspaceStore.getState().activateTab(tabId, "pane-1");
      expect(useWorkspaceStore.getState()).toBe(before);
    });
  });

  describe("Closing Tabs and State Preservation", () => {
    it("removes the tab from tabIds and tabs record", () => {
      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({ kind: "chat", title: "C1", conversationId: "c1" });
      const tab2 = store.openTab({ kind: "chat", title: "C2", conversationId: "c2" });

      useWorkspaceStore.getState().closeTab(tab2, "pane-1");

      const state = useWorkspaceStore.getState();
      expect(state.panes[0].tabIds).toEqual([tab1]);
      expect(state.tabs[tab2]).toBeUndefined();
      expect(state.panes[0].activeTabId).toBe(tab1);
    });

    it("does NOT delete underlying durable conversation or app in app-store when closing a tab", () => {
      // Seed durable conversation in app-store
      useAppStore.setState({
        conversations: [
          {
            id: "conv-durable-123",
            title: "Important Long Term Research",
            createdAt: "2026-10-01T00:00:00Z",
            updatedAt: "2026-10-28T00:00:00Z",
            archived: false,
            pinned: false,
          },
        ],
      });

      const tabId = useWorkspaceStore.getState().openTab({
        kind: "chat",
        title: "Important Long Term Research",
        conversationId: "conv-durable-123",
      });

      // Close the workspace tab
      useWorkspaceStore.getState().closeTab(tabId, "pane-1");

      // Tab is gone from workspace store
      expect(useWorkspaceStore.getState().tabs[tabId]).toBeUndefined();

      // But conversation in app store is completely untouched
      const appState = useAppStore.getState();
      expect(appState.conversations.some((c) => c.id === "conv-durable-123")).toBe(true);
    });

    it("falls back to a fresh 'New chat' tab when closing the last remaining tab in a pane", () => {
      const store = useWorkspaceStore.getState();
      const singleTab = store.openTab({
        kind: "chat",
        title: "Sole Tab",
        conversationId: "conv-sole",
      });

      expect(useWorkspaceStore.getState().panes[0].tabIds).toEqual([singleTab]);

      // Close the sole tab
      useWorkspaceStore.getState().closeTab(singleTab, "pane-1");

      const state = useWorkspaceStore.getState();
      expect(state.panes[0].tabIds).toHaveLength(1);
      const fallbackTabId = state.panes[0].tabIds[0];
      expect(fallbackTabId).not.toBe(singleTab);
      expect(state.panes[0].activeTabId).toBe(fallbackTabId);
      expect(state.tabs[fallbackTabId].title).toBe("New chat");
      expect(state.tabs[fallbackTabId].conversationId).toBeNull();
    });

    it("activates the adjacent tab when the active tab is closed", () => {
      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({ kind: "chat", title: "C1", conversationId: "c1" });
      const tab2 = store.openTab({ kind: "chat", title: "C2", conversationId: "c2" });
      const tab3 = store.openTab({ kind: "chat", title: "C3", conversationId: "c3" });

      // tab3 is currently active (index 2). Close it:
      useWorkspaceStore.getState().closeTab(tab3, "pane-1");
      expect(useWorkspaceStore.getState().panes[0].activeTabId).toBe(tab2);

      // Now tab2 is active (index 1). Close tab1 (inactive tab):
      useWorkspaceStore.getState().closeTab(tab1, "pane-1");
      expect(useWorkspaceStore.getState().panes[0].activeTabId).toBe(tab2);
    });
  });

  describe("Split Ratios", () => {
    it("updates split ratios and persists to storage", () => {
      useWorkspaceStore.getState().setSplitRatios([0.35, 0.65]);

      expect(useWorkspaceStore.getState().splitRatios).toEqual([0.35, 0.65]);
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
      expect(stored.splitRatios).toEqual([0.35, 0.65]);
    });
  });

  describe("Hydration and Storage Recovery", () => {
    it("hydrates successfully from valid stored session", () => {
      const persistedSession = {
        version: 1,
        layoutKind: "split-vertical",
        panes: [
          { id: "pane-1", tabIds: ["tab-restored-1"], activeTabId: "tab-restored-1" },
          { id: "pane-2", tabIds: ["tab-restored-2"], activeTabId: "tab-restored-2" },
        ],
        focusedPaneId: "pane-2",
        tabs: {
          "tab-restored-1": {
            id: "tab-restored-1",
            kind: "chat",
            title: "Restored Chat 1",
            conversationId: "conv-r1",
          },
          "tab-restored-2": {
            id: "tab-restored-2",
            kind: "app",
            title: "Restored App 2",
            toolId: "tool-r2",
          },
        },
        splitRatios: [0.4, 0.6],
      };

      localStorage.setItem(STORAGE_KEY, JSON.stringify(persistedSession));

      useWorkspaceStore.getState().hydrateFromStorage();

      const state = useWorkspaceStore.getState();
      expect(state.layoutKind).toBe("split-vertical");
      expect(state.panes).toHaveLength(2);
      expect(state.focusedPaneId).toBe("pane-2");
      expect(state.tabs["tab-restored-1"].title).toBe("Restored Chat 1");
      expect(state.tabs["tab-restored-2"].title).toBe("Restored App 2");
      expect(state.splitRatios).toEqual([0.4, 0.6]);
    });

    it("recovers cleanly to default state when storage contains corrupted JSON", () => {
      localStorage.setItem(STORAGE_KEY, "{corrupted-json-syntax-error---");

      useWorkspaceStore.getState().hydrateFromStorage();

      const state = useWorkspaceStore.getState();
      expect(state.layoutKind).toBe("single");
      expect(state.panes).toHaveLength(1);
      expect(state.panes[0].id).toBe(DEFAULT_WORKSPACE_PANE_ID);
    });

    it("recovers cleanly to default state when storage contains empty panes array", () => {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          version: 1,
          layoutKind: "single",
          panes: [],
          tabs: {},
        }),
      );

      useWorkspaceStore.getState().hydrateFromStorage();

      const state = useWorkspaceStore.getState();
      expect(state.panes).toHaveLength(1);
      expect(state.panes[0].id).toBe(DEFAULT_WORKSPACE_PANE_ID);
    });
  });

  describe("Reconciliation with Deleted Resources", () => {
    it("converts chat tabs referencing deleted conversations into fresh new chat tabs", () => {
      const tab1 = useWorkspaceStore.getState().openTab({
        kind: "chat",
        title: "Deleted Conversation",
        conversationId: "conv-to-delete",
      });

      const tab2 = useWorkspaceStore.getState().openTab({
        kind: "chat",
        title: "Kept Conversation",
        conversationId: "conv-kept",
      });

      // Reconcile: conv-to-delete is no longer available
      useWorkspaceStore.getState().reconcileWithAvailableResources({
        conversationIds: new Set(["conv-kept"]),
        toolIds: new Set(),
        projectIds: new Set(),
      });

      const state = useWorkspaceStore.getState();
      expect(state.tabs[tab1].title).toBe("New chat");
      expect(state.tabs[tab1].conversationId).toBeNull();
      expect(state.tabs[tab2].title).toBe("Kept Conversation");
      expect(state.tabs[tab2].conversationId).toBe("conv-kept");
    });

    it("marks app tabs referencing deleted tools as App unavailable", () => {
      const tabId = useWorkspaceStore.getState().openTab({
        kind: "app",
        title: "My Custom App",
        toolId: "tool-deleted-id",
      });

      useWorkspaceStore.getState().reconcileWithAvailableResources({
        conversationIds: new Set(),
        toolIds: new Set(["other-tool"]),
        projectIds: new Set(),
      });

      const tab = useWorkspaceStore.getState().tabs[tabId];
      expect(tab.title).toBe("App unavailable");
      expect(tab.toolId).toBeNull();
    });

    it("marks project tabs referencing deleted projects as Project unavailable", () => {
      const tabId = useWorkspaceStore.getState().openTab({
        kind: "project",
        title: "My Deleted Project",
        projectId: "proj-deleted-id",
      });

      useWorkspaceStore.getState().reconcileWithAvailableResources({
        conversationIds: new Set(),
        toolIds: new Set(),
        projectIds: new Set(["active-project-id"]),
      });

      const tab = useWorkspaceStore.getState().tabs[tabId];
      expect(tab.title).toBe("Project unavailable");
      expect(tab.projectId).toBeNull();
    });

    it("does not mutate state if all resources are present", () => {
      useWorkspaceStore.getState().openTab({
        kind: "chat",
        title: "Kept Chat",
        conversationId: "conv-valid",
      });

      const before = useWorkspaceStore.getState();
      useWorkspaceStore.getState().reconcileWithAvailableResources({
        conversationIds: new Set(["conv-valid"]),
        toolIds: new Set(),
        projectIds: new Set(),
      });

      expect(useWorkspaceStore.getState()).toBe(before);
    });
  });

  describe("Tab Reordering & Max Tabs Constraint", () => {
    it("reorders tabs within a pane and persists to storage", () => {
      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({ kind: "chat", title: "Tab 1", conversationId: "c1" });
      const tab2 = store.openTab({ kind: "chat", title: "Tab 2", conversationId: "c2" });
      const tab3 = store.openTab({ kind: "chat", title: "Tab 3", conversationId: "c3" });

      expect(useWorkspaceStore.getState().panes[0].tabIds).toEqual([tab1, tab2, tab3]);

      // Move tab1 (index 0) to end (index 2)
      useWorkspaceStore.getState().reorderTabs("pane-1", 0, 2);
      expect(useWorkspaceStore.getState().panes[0].tabIds).toEqual([tab2, tab3, tab1]);

      // Check persistence
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
      expect(stored.panes[0].tabIds).toEqual([tab2, tab3, tab1]);
    });

    it("ignores reorder requests with invalid or identical indices", () => {
      const store = useWorkspaceStore.getState();
      const tab1 = store.openTab({ kind: "chat", title: "Tab 1", conversationId: "c1" });
      const tab2 = store.openTab({ kind: "chat", title: "Tab 2", conversationId: "c2" });

      const before = useWorkspaceStore.getState();
      useWorkspaceStore.getState().reorderTabs("pane-1", 0, 0); // same
      useWorkspaceStore.getState().reorderTabs("pane-1", -1, 1); // out of bounds
      useWorkspaceStore.getState().reorderTabs("pane-1", 0, 99); // out of bounds
      useWorkspaceStore.getState().reorderTabs("non-existent-pane", 0, 1);

      expect(useWorkspaceStore.getState().panes[0].tabIds).toEqual([tab1, tab2]);
      expect(useWorkspaceStore.getState()).toBe(before);
    });

    it("enforces MAX_TABS_PER_PANE by pruning oldest non-active closable tab", () => {
      const store = useWorkspaceStore.getState();
      const tabIds: string[] = [];

      // Open 20 tabs (the maximum limit)
      for (let i = 1; i <= 20; i++) {
        const tid = store.openTab({
          kind: "chat",
          title: `Tab ${i}`,
          conversationId: `c-${i}`,
          closable: true,
        });
        tabIds.push(tid);
      }

      const pane = useWorkspaceStore.getState().panes[0];
      expect(pane.tabIds).toHaveLength(20);
      expect(pane.tabIds).toContain(tabIds[0]);

      // Open 21st tab: tabIds[0] (oldest non-active closable) should be pruned
      const tab21 = store.openTab({
        kind: "chat",
        title: "Tab 21",
        conversationId: "c-21",
        closable: true,
      });

      const nextPane = useWorkspaceStore.getState().panes[0];
      expect(nextPane.tabIds).toHaveLength(20);
      expect(nextPane.tabIds).not.toContain(tabIds[0]); // pruned
      expect(nextPane.tabIds).toContain(tab21);
      expect(useWorkspaceStore.getState().tabs[tabIds[0]]).toBeUndefined();
    });

    it("post-validation sanitization recovers panes referencing missing tabs in storage", () => {
      // Simulate partial localStorage where pane references a deleted tab
      const partialStorage = {
        version: 1,
        layoutKind: "single",
        panes: [
          {
            id: "pane-1",
            tabIds: ["missing-tab-id"],
            activeTabId: "missing-tab-id",
          },
        ],
        focusedPaneId: "pane-invalid",
        tabs: {},
        splitRatios: [0.5, 0.5],
      };

      localStorage.setItem(STORAGE_KEY, JSON.stringify(partialStorage));
      useWorkspaceStore.getState().hydrateFromStorage();

      const state = useWorkspaceStore.getState();
      expect(state.panes).toHaveLength(1);
      expect(state.panes[0].tabIds).toHaveLength(1);
      const generatedTabId = state.panes[0].tabIds[0];
      expect(state.tabs[generatedTabId]).toBeDefined();
      expect(state.focusedPaneId).toBe("pane-1"); // fixed from pane-invalid
    });
  });
});
