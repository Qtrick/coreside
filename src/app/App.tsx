import { useEffect } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { ConflictBanner } from "@/components/chat/ConflictBanner";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { api } from "@/lib/tauri";
import { applyAppearanceCssVars, useAppStore } from "@/stores/app-store";

function parseToolRoute(hash: string): string | null {
  const match = hash.match(/^#\/tool\/([^/?#]+)/);
  return match?.[1] ? decodeURIComponent(match[1]) : null;
}

function applyThemeToDocument(theme: "light" | "dark") {
  document.documentElement.dataset.theme = theme;
}

function ToolWindowView({ toolId }: { toolId: string }) {
  const bootstrapped = useAppStore((s) => s.bootstrapped);
  const activeTool = useAppStore((s) => s.activeTool);
  const toolState = useAppStore((s) => s.toolState);
  const loadToolWindow = useAppStore((s) => s.loadToolWindow);
  const updateToolState = useAppStore((s) => s.updateToolState);
  const bootError = useAppStore((s) => s.bootError);

  useEffect(() => {
    void loadToolWindow(toolId);
  }, [loadToolWindow, toolId]);

  if (bootError) {
    return (
      <div className="tool-window">
        <div className="empty-state">
          <h3>Could not open tool</h3>
          <p>{bootError}</p>
        </div>
      </div>
    );
  }

  if (!bootstrapped || !activeTool) {
    return (
      <div className="tool-window">
        <div className="empty-state">
          <div className="loading-dots" aria-label="Loading tool">
            <span />
            <span />
            <span />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="tool-window">
      <ConflictBanner />
      <header style={{ marginBottom: "1rem" }}>
        <h1
          style={{
            margin: 0,
            fontFamily: "var(--font-display)",
            fontSize: "1.35rem",
          }}
        >
          {activeTool.name}
        </h1>
        <p className="muted" style={{ margin: "0.35rem 0 0" }}>
          {activeTool.description}
        </p>
      </header>
      <ToolRenderer
        tool={activeTool}
        state={toolState}
        onStateChange={(state) => {
          void updateToolState(state, true);
        }}
      />
    </div>
  );
}

export function App() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const bootstrapped = useAppStore((s) => s.bootstrapped);
  const bootError = useAppStore((s) => s.bootError);
  const theme = useAppStore((s) => s.theme);
  const resolvedTheme = useAppStore((s) => s.resolvedTheme);
  const appearance = useAppStore((s) => s.appearance);
  const applyResolvedTheme = useAppStore((s) => s.applyResolvedTheme);
  const dockIcon = useAppStore((s) => s.dockIcon);
  const toolRoute =
    typeof window !== "undefined" ? parseToolRoute(window.location.hash) : null;

  useEffect(() => {
    if (!toolRoute) {
      void bootstrap();
    }
  }, [bootstrap, toolRoute]);

  useEffect(() => {
    applyThemeToDocument(resolvedTheme);
    applyAppearanceCssVars(resolvedTheme, appearance);
  }, [resolvedTheme, appearance]);

  useEffect(() => {
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () => {
      applyResolvedTheme(media.matches ? "dark" : "light");
    };
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, [theme, applyResolvedTheme]);

  // Dock icon: `auto` follows OS; `dark` / `light` lock the tile. Independent of in-app theme.
  useEffect(() => {
    if (!bootstrapped) return;
    const apply = () => {
      const osIsDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      void api.setDockIcon(dockIcon, osIsDark).catch(() => {
        // Non-macOS / web preview: command may no-op or be unavailable.
      });
    };
    apply();
    if (dockIcon !== "auto") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [bootstrapped, dockIcon]);

  useEffect(() => {
    const onHash = () => {
      if (parseToolRoute(window.location.hash)) {
        window.location.reload();
      }
    };
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  if (toolRoute) {
    return <ToolWindowView toolId={toolRoute} />;
  }

  if (!bootstrapped) {
    return (
      <div className="empty-state" style={{ minHeight: "100%" }}>
        <div className="loading-dots" aria-label="Starting Coreside">
          <span />
          <span />
          <span />
        </div>
      </div>
    );
  }

  if (bootError) {
    return (
      <div className="empty-state" style={{ minHeight: "100%" }}>
        <h2>Coreside could not start</h2>
        <p>{bootError}</p>
      </div>
    );
  }

  return <AppShell />;
}
