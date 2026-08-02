import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import { useAppStore } from "@/stores/app-store";
import type { ActionLogMode } from "@/lib/action-log";
import { parseActionLogMode } from "@/lib/action-log";

const MODES: { id: ActionLogMode; label: string; hint: string }[] = [
  {
    id: "off",
    label: "Off",
    hint: "Hide Action Log",
  },
  {
    id: "always",
    label: "Always",
    hint: "Show every sanitized step",
  },
  {
    id: "intelligent",
    label: "Intelligent",
    hint: "Show when real work happens",
  },
];

export function AgentBehaviorSettings() {
  const [mode, setMode] = useState<ActionLogMode>(() =>
    useAppStore.getState().actionLogMode,
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bootstrapped = useAppStore((s) => s.bootstrapped);
  const storeMode = useAppStore((s) => s.actionLogMode);

  useEffect(() => {
    if (bootstrapped) setMode(storeMode);
  }, [bootstrapped, storeMode]);

  useEffect(() => {
    if (!bootstrapped) return;
    let cancelled = false;
    void api
      .getSettings()
      .then((settings) => {
        if (cancelled) return;
        const next = parseActionLogMode(
          settings.actionLogMode ?? (settings.actionLogEnabled ? "always" : "off"),
        );
        setMode(next);
        useAppStore.setState({
          actionLogMode: next,
          actionLogEnabled: next !== "off",
        });
      })
      .catch(() => {
        if (!cancelled) setMode("off");
      });
    return () => {
      cancelled = true;
    };
  }, [bootstrapped]);

  const onSelect = async (next: ActionLogMode) => {
    setSaving(true);
    setError(null);
    setMode(next);
    try {
      await api.setSetting("actionLogMode", next);
      useAppStore.setState({
        actionLogMode: next,
        actionLogEnabled: next !== "off",
      });
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Could not update Action Log.",
      );
      try {
        const settings = await api.getSettings();
        setMode(
          parseActionLogMode(
            settings.actionLogMode ?? (settings.actionLogEnabled ? "always" : "off"),
          ),
        );
      } catch {
        /* keep optimistic value */
      }
    } finally {
      setSaving(false);
    }
  };

  return (
    <section
      className="settings-section settings-section-compact"
      aria-labelledby="agent-behavior-heading"
    >
      <h3 id="agent-behavior-heading">Agent</h3>
      <div className="settings-toggle-row settings-toggle-row-stack">
        <div>
          <strong id="action-log-label">Action Log</strong>
          <p>
            Show a concise record of actions Coreside performs — never private
            model reasoning. Intelligent mode appears only when the agent
            searches, changes tools, or does other real work.
          </p>
        </div>
        <div
          className="theme-options"
          role="group"
          aria-labelledby="action-log-label"
        >
          {MODES.map((option) => (
            <button
              key={option.id}
              type="button"
              className="btn btn-secondary"
              aria-pressed={mode === option.id}
              title={option.hint}
              disabled={saving}
              onClick={() => void onSelect(option.id)}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>
      {error ? (
        <p className="provider-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
