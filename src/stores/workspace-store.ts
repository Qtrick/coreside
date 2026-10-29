import { create } from "zustand";
import {
  createDefaultWorkspaceState,
  createDefaultWorkspaceTab,
  MAX_TABS_PER_PANE,
  WorkspaceStateSchema,
  type PaneLayoutKind,
  type WorkspacePane,
  type WorkspaceState,
  type WorkspaceTab,
} from "@/types/workspace";

const STORAGE_KEY = "coreside:workspace:session";

interface WorkspaceStore extends WorkspaceState {
  setLayout: (kind: PaneLayoutKind) => void;
  setFocusedPane: (paneId: string) => void;
  openTab: (
    tabData: Omit<WorkspaceTab, "id"> & { id?: string },
    targetPaneId?: string,
  ) => string;
  closeTab: (tabId: string, targetPaneId?: string) => void;
  activateTab: (tabId: string, targetPaneId?: string) => void;
  reorderTabs: (paneId: string, fromIndex: number, toIndex: number) => void;
  setSplitRatios: (ratios: number[]) => void;
  ensurePane: (paneId: string) => WorkspacePane;
  hydrateFromStorage: () => void;
  reconcileWithAvailableResources: (params: {
    conversationIds: Set<string>;
    toolIds: Set<string>;
    projectIds: Set<string>;
  }) => void;
}

function persistState(state: WorkspaceState) {
  try {
    if (typeof localStorage !== "undefined") {
      const data = {
        version: state.version,
        layoutKind: state.layoutKind,
        panes: state.panes,
        focusedPaneId: state.focusedPaneId,
        tabs: state.tabs,
        splitRatios: state.splitRatios,
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    }
  } catch {
    // Storage quota or sandboxed failure — fail safe
  }
}

function loadInitialState(): WorkspaceState {
  try {
    if (typeof localStorage !== "undefined") {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsedJson = JSON.parse(raw);
        const result = WorkspaceStateSchema.safeParse(parsedJson);
        if (result.success && result.data.panes.length > 0) {
          const validated = result.data as WorkspaceState;
          // Post-validation sanitization: ensure each pane has valid tabs and focusedPaneId exists
          const nextTabs = { ...validated.tabs };
          const validPanes = validated.panes.map((p) => {
            const validTabIds = p.tabIds.filter((tid) => Boolean(nextTabs[tid]));
            if (validTabIds.length === 0) {
              const fallback = createDefaultWorkspaceTab();
              fallback.id = `tab-${Date.now()}-${p.id}`;
              nextTabs[fallback.id] = fallback;
              validTabIds.push(fallback.id);
            }
            const activeValid =
              p.activeTabId && validTabIds.includes(p.activeTabId)
                ? p.activeTabId
                : validTabIds[0];
            return {
              ...p,
              tabIds: validTabIds,
              activeTabId: activeValid,
            };
          });
          const focusedValid = validPanes.some((p) => p.id === validated.focusedPaneId)
            ? validated.focusedPaneId
            : validPanes[0].id;
          return {
            ...validated,
            tabs: nextTabs,
            panes: validPanes,
            focusedPaneId: focusedValid,
          };
        }
      }
    }
  } catch {
    // Malformed JSON — recover cleanly
  }
  return createDefaultWorkspaceState();
}

export const useWorkspaceStore = create<WorkspaceStore>((set, get) => {
  const initial = loadInitialState();

  return {
    ...initial,

    setLayout: (kind) => {
      set((state) => {
        const panes = [...state.panes];
        let requiredPanes = 1;
        if (kind === "split-vertical" || kind === "split-horizontal") {
          requiredPanes = 2;
        } else if (kind === "split-three") {
          requiredPanes = 3;
        } else if (kind === "split-grid") {
          requiredPanes = 4;
        }

        // Ensure required panes exist
        for (let i = 1; i <= requiredPanes; i++) {
          const pid = `pane-${i}`;
          if (!panes.some((p) => p.id === pid)) {
            const newTabId = `tab-${Date.now()}-${i}`;
            const newTab = createDefaultWorkspaceTab();
            newTab.id = newTabId;
            panes.push({
              id: pid,
              tabIds: [newTabId],
              activeTabId: newTabId,
            });
            state.tabs[newTabId] = newTab;
          }
        }

        const next: WorkspaceState = {
          ...state,
          layoutKind: kind,
          panes,
        };
        persistState(next);
        return next;
      });
    },

    setFocusedPane: (paneId) => {
      set((state) => {
        if (state.focusedPaneId === paneId) return state;
        const next = { ...state, focusedPaneId: paneId };
        persistState(next);
        return next;
      });
    },

    openTab: (tabData, targetPaneId) => {
      const state = get();
      const paneId = targetPaneId || state.focusedPaneId || state.panes[0]?.id || "pane-1";
      const paneIndex = state.panes.findIndex((p) => p.id === paneId);
      const pane = paneIndex >= 0 ? state.panes[paneIndex] : null;

      // Check if matching tab already exists in this pane
      if (pane) {
        const targetConvId = tabData.conversationId ?? null;
        const targetToolId = tabData.toolId ?? null;
        const targetProjectId = tabData.projectId ?? null;

        if (targetConvId || targetToolId || targetProjectId) {
          for (const tid of pane.tabIds) {
            const existing = state.tabs[tid];
            if (
              existing &&
              existing.kind === tabData.kind &&
              (existing.conversationId ?? null) === targetConvId &&
              (existing.toolId ?? null) === targetToolId &&
              (existing.projectId ?? null) === targetProjectId
            ) {
              // Activate existing tab
              get().activateTab(tid, paneId);
              return tid;
            }
          }
        }
      }

      const id = tabData.id || `tab-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
      const newTab: WorkspaceTab = {
        id,
        kind: tabData.kind,
        title: tabData.title,
        conversationId: tabData.conversationId ?? null,
        toolId: tabData.toolId ?? null,
        projectId: tabData.projectId ?? null,
        view: tabData.view,
        closable: tabData.closable !== false,
      };

      set((curr) => {
        const nextTabs = { ...curr.tabs, [id]: newTab };
        const nextPanes = [...curr.panes];
        const pIndex = nextPanes.findIndex((p) => p.id === paneId);

        if (pIndex >= 0) {
          const currentPane = nextPanes[pIndex];
          // If the only tab in this pane is an empty default "New chat", replace it
          const firstTab = curr.tabs[currentPane.tabIds[0]];
          const shouldReplace =
            currentPane.tabIds.length === 1 &&
            firstTab &&
            firstTab.kind === "chat" &&
            !firstTab.conversationId &&
            Boolean(newTab.conversationId || newTab.toolId || newTab.projectId);

          let nextTabIds = shouldReplace ? [id] : [...currentPane.tabIds, id];
          if (!shouldReplace && nextTabIds.length > MAX_TABS_PER_PANE) {
            const prunableIdx = nextTabIds.findIndex(
              (tid) =>
                tid !== id &&
                tid !== currentPane.activeTabId &&
                curr.tabs[tid]?.closable !== false,
            );
            if (prunableIdx >= 0) {
              const prunedId = nextTabIds[prunableIdx];
              nextTabIds = nextTabIds.filter((t) => t !== prunedId);
              delete nextTabs[prunedId];
            }
          }
          nextPanes[pIndex] = {
            ...currentPane,
            tabIds: nextTabIds,
            activeTabId: id,
          };
          if (shouldReplace && firstTab) {
            delete nextTabs[firstTab.id];
          }
        } else {
          nextPanes.push({
            id: paneId,
            tabIds: [id],
            activeTabId: id,
          });
        }

        const next: WorkspaceState = {
          ...curr,
          tabs: nextTabs,
          panes: nextPanes,
          focusedPaneId: paneId,
        };
        persistState(next);
        return next;
      });

      return id;
    },

    closeTab: (tabId, targetPaneId) => {
      set((state) => {
        const paneId = targetPaneId || state.focusedPaneId;
        const pIndex = state.panes.findIndex((p) => p.id === paneId);
        if (pIndex < 0) return state;

        const currentPane = state.panes[pIndex];
        const tabIndex = currentPane.tabIds.indexOf(tabId);
        if (tabIndex < 0) return state;

        const nextTabIds = currentPane.tabIds.filter((t) => t !== tabId);
        const nextTabs = { ...state.tabs };
        delete nextTabs[tabId];

        let nextActiveTabId = currentPane.activeTabId;
        if (currentPane.activeTabId === tabId) {
          if (nextTabIds.length > 0) {
            const nextIdx = Math.min(tabIndex, nextTabIds.length - 1);
            nextActiveTabId = nextTabIds[nextIdx];
          } else {
            // No tabs left in this pane: create a fresh "New chat" tab
            const fallbackTab = createDefaultWorkspaceTab();
            const fallbackId = `tab-${Date.now()}`;
            fallbackTab.id = fallbackId;
            nextTabs[fallbackId] = fallbackTab;
            nextTabIds.push(fallbackId);
            nextActiveTabId = fallbackId;
          }
        }

        const nextPanes = [...state.panes];
        nextPanes[pIndex] = {
          ...currentPane,
          tabIds: nextTabIds,
          activeTabId: nextActiveTabId,
        };

        const next: WorkspaceState = {
          ...state,
          tabs: nextTabs,
          panes: nextPanes,
        };
        persistState(next);
        return next;
      });
    },

    activateTab: (tabId, targetPaneId) => {
      set((state) => {
        const paneId = targetPaneId || state.focusedPaneId;
        const pIndex = state.panes.findIndex((p) => p.id === paneId);
        if (pIndex < 0) return state;

        const currentPane = state.panes[pIndex];
        if (currentPane.activeTabId === tabId) return state;

        const nextPanes = [...state.panes];
        nextPanes[pIndex] = {
          ...currentPane,
          activeTabId: tabId,
        };

        const next: WorkspaceState = {
          ...state,
          panes: nextPanes,
          focusedPaneId: paneId,
        };
        persistState(next);
        return next;
      });
    },

    reorderTabs: (paneId, fromIndex, toIndex) => {
      set((state) => {
        const pIndex = state.panes.findIndex((p) => p.id === paneId);
        if (pIndex < 0) return state;

        const currentPane = state.panes[pIndex];
        if (
          fromIndex < 0 ||
          fromIndex >= currentPane.tabIds.length ||
          toIndex < 0 ||
          toIndex >= currentPane.tabIds.length ||
          fromIndex === toIndex
        ) {
          return state;
        }

        const nextTabIds = [...currentPane.tabIds];
        const [moved] = nextTabIds.splice(fromIndex, 1);
        nextTabIds.splice(toIndex, 0, moved);

        const nextPanes = [...state.panes];
        nextPanes[pIndex] = {
          ...currentPane,
          tabIds: nextTabIds,
        };

        const next: WorkspaceState = {
          ...state,
          panes: nextPanes,
        };
        persistState(next);
        return next;
      });
    },

    setSplitRatios: (ratios) => {
      set((state) => {
        const next = { ...state, splitRatios: ratios };
        persistState(next);
        return next;
      });
    },

    ensurePane: (paneId) => {
      const state = get();
      const existing = state.panes.find((p) => p.id === paneId);
      if (existing) return existing;

      const fallbackTab = createDefaultWorkspaceTab();
      const tabId = `tab-${Date.now()}-${paneId}`;
      fallbackTab.id = tabId;

      const newPane: WorkspacePane = {
        id: paneId,
        tabIds: [tabId],
        activeTabId: tabId,
      };

      set((curr) => {
        const next: WorkspaceState = {
          ...curr,
          panes: [...curr.panes, newPane],
          tabs: { ...curr.tabs, [tabId]: fallbackTab },
        };
        persistState(next);
        return next;
      });

      return newPane;
    },

    hydrateFromStorage: () => {
      const restored = loadInitialState();
      set(restored);
    },

    reconcileWithAvailableResources: ({ conversationIds, toolIds, projectIds }) => {
      set((state) => {
        let changed = false;
        const nextTabs = { ...state.tabs };

        for (const [tid, tab] of Object.entries(nextTabs)) {
          if (tab.kind === "chat" && tab.conversationId) {
            if (!conversationIds.has(tab.conversationId)) {
              // Conversation was deleted — convert to fresh new chat tab
              nextTabs[tid] = {
                ...tab,
                title: "New chat",
                conversationId: null,
              };
              changed = true;
            }
          } else if (tab.kind === "app" && tab.toolId) {
            if (!toolIds.has(tab.toolId)) {
              // Tool was removed
              nextTabs[tid] = {
                ...tab,
                title: "App unavailable",
                toolId: null,
              };
              changed = true;
            }
          } else if (tab.kind === "project" && tab.projectId) {
            if (!projectIds.has(tab.projectId)) {
              nextTabs[tid] = {
                ...tab,
                title: "Project unavailable",
                projectId: null,
              };
              changed = true;
            }
          }
        }

        if (!changed) return state;
        const next = { ...state, tabs: nextTabs };
        persistState(next);
        return next;
      });
    },
  };
});

