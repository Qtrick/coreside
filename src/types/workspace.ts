import { z } from "zod";
import type { AppView } from "@/lib/navigation/types";

export const PaneLayoutKindSchema = z.enum([
  "single",
  "split-vertical",
  "split-horizontal",
  "split-three",
  "split-grid",
]);

export type PaneLayoutKind = z.infer<typeof PaneLayoutKindSchema>;

export const WorkspaceTabKindSchema = z.enum([
  "chat",
  "app",
  "project",
  "projects",
  "media",
  "automations",
  "settings",
]);

export type WorkspaceTabKind = z.infer<typeof WorkspaceTabKindSchema>;

export const WorkspaceTabSchema = z.object({
  id: z.string(),
  kind: WorkspaceTabKindSchema,
  title: z.string(),
  conversationId: z.string().nullable().optional(),
  toolId: z.string().nullable().optional(),
  projectId: z.string().nullable().optional(),
  closable: z.boolean().optional(),
});

export type WorkspaceTab = z.infer<typeof WorkspaceTabSchema> & {
  view?: AppView;
};

export const WorkspacePaneSchema = z.object({
  id: z.string(),
  tabIds: z.array(z.string()),
  activeTabId: z.string().nullable(),
});

export type WorkspacePane = z.infer<typeof WorkspacePaneSchema>;

export const WorkspaceStateSchema = z.object({
  version: z.number().default(1),
  layoutKind: PaneLayoutKindSchema,
  panes: z.array(WorkspacePaneSchema),
  focusedPaneId: z.string(),
  tabs: z.record(z.string(), WorkspaceTabSchema),
  splitRatios: z.array(z.number()).default([0.5, 0.5]),
});

export type WorkspaceState = z.infer<typeof WorkspaceStateSchema>;

export const DEFAULT_WORKSPACE_TAB_ID = "tab-default";
export const DEFAULT_WORKSPACE_PANE_ID = "pane-1";
export const MAX_TABS_PER_PANE = 20;

export function createDefaultWorkspaceTab(): WorkspaceTab {
  return {
    id: DEFAULT_WORKSPACE_TAB_ID,
    kind: "chat",
    title: "New chat",
    conversationId: null,
    closable: false,
  };
}

export function createDefaultWorkspaceState(): WorkspaceState {
  const defaultTab = createDefaultWorkspaceTab();
  return {
    version: 1,
    layoutKind: "single",
    panes: [
      {
        id: DEFAULT_WORKSPACE_PANE_ID,
        tabIds: [defaultTab.id],
        activeTabId: defaultTab.id,
      },
    ],
    focusedPaneId: DEFAULT_WORKSPACE_PANE_ID,
    tabs: {
      [defaultTab.id]: defaultTab,
    },
    splitRatios: [0.5, 0.5],
  };
}

