export type AppView =
  | { kind: "chat"; conversationId: string | null }
  | { kind: "project"; projectId: string }
  | { kind: "projects" }
  | { kind: "settings" }
  | { kind: "automations" }
  | { kind: "media" }
  | { kind: "tools" };

export type ChatViewState = {
  scrollTop?: number;
  draft?: string;
  activeToolId?: string | null;
};

export const DEFAULT_CHAT_VIEW: AppView = {
  kind: "chat",
  conversationId: null,
};

export function isOverlayView(view: AppView): boolean {
  return (
    view.kind === "settings" ||
    view.kind === "automations" ||
    view.kind === "projects" ||
    view.kind === "media" ||
    view.kind === "project"
  );
}

export function isChatView(view: AppView): boolean {
  return view.kind === "chat";
}
