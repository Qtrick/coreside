import { useEffect, useState } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { ConflictBanner } from "@/components/chat/ConflictBanner";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import { BootstrapRecoveryScreen } from "@/components/recovery/BootstrapRecoveryScreen";
import { api, isWebPreview, listenAppearanceChanged } from "@/lib/tauri";
import { applyAppearanceCssVars, useAppStore } from "@/stores/app-store";

function WebPreviewBanner() {
  if (!isWebPreview()) return null;
  return (
    <div
      role="status"
      style={{
        flexShrink: 0,
        padding: "0.35rem 0.75rem",
        fontSize: "0.75rem",
        textAlign: "center",
        background: "var(--surface-muted, #ececec)",
        color: "var(--text-secondary, #737373)",
        borderBottom: "1px solid var(--border, #e4e4e4)",
      }}
    >
      Web preview — in-memory mocks only; data is not persisted
    </div>
  );
}

function parseToolRoute(hash: string): string | null {
  const match = hash.match(/^#\/tool\/([^/?#]+)/);
  return match?.[1] ? decodeURIComponent(match[1]) : null;
}

function applyThemeToDocument(theme: "light" | "dark") {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

function ToolWindowView({ toolId }: { toolId: string }) {
  const bootstrapped = useAppStore((s) => s.bootstrapped);
  const bootstrapStatus = useAppStore((s) => s.bootstrapStatus);
  const activeTool = useAppStore((s) => s.activeTool);
  const toolState = useAppStore((s) => s.toolState);
  const loadToolWindow = useAppStore((s) => s.loadToolWindow);
  const updateToolState = useAppStore((s) => s.updateToolState);
  const bootError = useAppStore((s) => s.bootError);
  const [applicationId, setApplicationId] = useState<string | null>(null);

  useEffect(() => {
    void loadToolWindow(toolId);
  }, [loadToolWindow, toolId]);

  // Crash reporting must use a real kernel application id, never the tool id.
  useEffect(() => {
    let cancelled = false;
    void api
      .kernelGetManifest(toolId)
      .then((record) => {
        if (!cancelled) setApplicationId(record.applicationId);
      })
      .catch(() => {
        if (!cancelled) setApplicationId(null);
      });
    return () => {
      cancelled = true;
    };
  }, [toolId]);

  if (bootstrapStatus?.status === "recoveryRequired") {
    return <BootstrapRecoveryScreen status={bootstrapStatus} />;
  }

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
        applicationId={applicationId}
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
  const bootstrapStatus = useAppStore((s) => s.bootstrapStatus);
  const theme = useAppStore((s) => s.theme);
  const resolvedTheme = useAppStore((s) => s.resolvedTheme);
  const appearance = useAppStore((s) => s.appearance);
  const applyResolvedTheme = useAppStore((s) => s.applyResolvedTheme);
  const applyPersistedDockIcon = useAppStore((s) => s.applyPersistedDockIcon);
  const reconcileCommittedAppearance = useAppStore(
    (s) => s.reconcileCommittedAppearance,
  );
  const toolRoute =
    typeof window !== "undefined" ? parseToolRoute(window.location.hash) : null;

  useEffect(() => {
    if (!toolRoute) {
      void bootstrap();
    }
  }, [bootstrap, toolRoute]);

  // Cross-window appearance apply: refetch settings and reconcile store.
  // Generation counter rejects out-of-order getSettings responses so a slower
  // older fetch cannot regress a newer committed appearance.
  useEffect(() => {
    if (!bootstrapped) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let appearanceFetchGen = 0;
    void listenAppearanceChanged(() => {
      const gen = ++appearanceFetchGen;
      void api
        .getSettings()
        .then((settings) => {
          if (!cancelled && gen === appearanceFetchGen) {
            reconcileCommittedAppearance(settings);
          }
        })
        .catch(() => undefined);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    }).catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [bootstrapped, reconcileCommittedAppearance]);

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

  // Dock icon ownership is process-global and Rust-owned. Only the main window
  // re-applies the persisted preference once after bootstrap. Follow macOS clears
  // the temporary AppKit override; manual mode installs a validated PNG.
  useEffect(() => {
    if (!bootstrapped) return;
    if (toolRoute) return;
    void applyPersistedDockIcon();
  }, [bootstrapped, toolRoute, applyPersistedDockIcon]);

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

  if (bootstrapStatus?.status === "recoveryRequired") {
    return <BootstrapRecoveryScreen status={bootstrapStatus} />;
  }

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        minHeight: "100%",
      }}
    >
      <WebPreviewBanner />
      <div style={{ flex: 1, minHeight: 0 }}>
        <AppShell />
      </div>
    </div>
  );
}
