import { Sidebar } from "@/components/sidebar/Sidebar";
import { ChatPanel } from "@/components/chat/ChatPanel";
import { ToolCanvas } from "@/components/tool-canvas/ToolCanvas";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { useAppStore } from "@/stores/app-store";

export function AppShell() {
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const activeToolId = useAppStore((s) => s.activeToolId);
  const settingsOpen = useAppStore((s) => s.settingsOpen);

  const classes = [
    "app-shell",
    sidebarCollapsed ? "sidebar-collapsed" : "",
    !activeToolId || settingsOpen ? "no-tool" : "",
    settingsOpen ? "settings-mode" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes}>
      <Sidebar />
      {settingsOpen ? (
        <SettingsPanel />
      ) : (
        <>
          <ChatPanel />
          {activeToolId ? <ToolCanvas /> : null}
        </>
      )}
    </div>
  );
}
