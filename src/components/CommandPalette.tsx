import { useEffect, useMemo, useState } from "react";
import { api } from "@/lib/tauri";
import type { UnifiedSearchHit } from "@/types/application-kernel";
import { useAppStore } from "@/stores/app-store";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { storeSettingsCategory } from "@/lib/settings-categories";

/**
 * Consumer command palette — protected actions + local discovery.
 * Generated apps cannot replace these commands.
 */
export function CommandPalette({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<UnifiedSearchHit[]>([]);
  const navigateToChat = useAppStore((s) => s.navigateToChat);
  const createConversation = useAppStore((s) => s.createConversation);
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);
  const navigateToAutomations = useAppStore((s) => s.navigateToAutomations);
  const navigateToMedia = useAppStore((s) => s.navigateToMedia);
  const navigateToProjects = useAppStore((s) => s.navigateToProjects);

  const commands = useMemo(
    () => [
      {
        id: "new-chat",
        title: "New Chat",
        run: () => void createConversation(),
      },
      {
        id: "settings",
        title: "Open Settings",
        run: () => navigateToSettings(),
      },
      {
        id: "help-learning",
        title: "Open Help & learning",
        run: () => {
          storeSettingsCategory("help-learning");
          navigateToSettings();
        },
      },
      {
        id: "start-tour",
        title: "Start Coreside tour",
        run: () => {
          void useOnboardingStore.getState().startEssentials(false);
        },
      },
      {
        id: "recovery",
        title: "Open Recovery (in Settings)",
        run: () => navigateToSettings(),
      },
      {
        id: "projects",
        title: "Open Projects",
        run: () => navigateToProjects(),
      },
      {
        id: "media",
        title: "Open Media Library",
        run: () => navigateToMedia(),
      },
      {
        id: "automations",
        title: "Open Automations",
        run: () => navigateToAutomations(),
      },
    ],
    [
      createConversation,
      navigateToSettings,
      navigateToAutomations,
      navigateToMedia,
      navigateToProjects,
    ],
  );

  useEffect(() => {
    if (!open) return;
    const q = query.trim();
    if (!q) {
      setHits([]);
      return;
    }
    let cancelled = false;
    void api
      .kernelUnifiedSearch(q, 12)
      .then((rows) => {
        if (!cancelled) setHits(rows);
      })
      .catch(() => {
        if (!cancelled) setHits([]);
      });
    return () => {
      cancelled = true;
    };
  }, [open, query]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const qLower = query.trim().toLowerCase();
  const filteredCommands = commands.filter(
    (c) => !qLower || c.title.toLowerCase().includes(qLower),
  );

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.35)",
        display: "flex",
        alignItems: "flex-start",
        justifyContent: "center",
        paddingTop: "12vh",
        zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        style={{
          width: "min(520px, 92vw)",
          background: "var(--core-modal-overlay, var(--surface))",
          borderRadius: 12,
          border: "1px solid var(--border)",
          padding: "0.75rem",
          backdropFilter: "blur(16px)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <input
          autoFocus
          aria-label="Search commands and applications"
          placeholder="Search chats, apps, applications, commands…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{
            width: "100%",
            padding: "0.65rem 0.75rem",
            borderRadius: 8,
            border: "1px solid var(--border)",
            background: "var(--core-control-overlay, var(--surface))",
            color: "var(--fg)",
          }}
        />
        <ul
          style={{
            listStyle: "none",
            margin: "0.5rem 0 0",
            padding: 0,
            maxHeight: 320,
            overflow: "auto",
          }}
        >
          {filteredCommands.map((c) => (
            <li key={c.id}>
              <button
                type="button"
                className="btn btn-secondary"
                style={{
                  width: "100%",
                  justifyContent: "flex-start",
                  marginBottom: 4,
                }}
                onClick={() => {
                  c.run();
                  onClose();
                }}
              >
                {c.title}
              </button>
            </li>
          ))}
          {hits.map((h) => (
            <li key={`${h.resourceType}-${h.id}`}>
              <button
                type="button"
                className="btn btn-secondary"
                style={{
                  width: "100%",
                  justifyContent: "flex-start",
                  marginBottom: 4,
                }}
                onClick={() => {
                  if (h.resourceType === "chat") void navigateToChat(h.id);
                  else navigateToSettings();
                  onClose();
                }}
              >
                <span className="muted" style={{ marginRight: 8 }}>
                  {h.resourceType}
                </span>
                {h.title}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
