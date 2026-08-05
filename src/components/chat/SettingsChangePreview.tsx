import { useAppStore } from "@/stores/app-store";

export function SettingsChangePreview() {
  const pending = useAppStore((s) => s.pendingSettingsChange);
  const applyPendingSettingsChange = useAppStore((s) => s.applyPendingSettingsChange);
  const discardPendingSettingsChange = useAppStore((s) => s.discardPendingSettingsChange);

  if (!pending) return null;

  const { settingsChange } = pending;
  const summary =
    settingsChange.changeSummary?.trim() ||
    "Review this appearance change before applying it to your workspace.";

  const details: string[] = [];
  if (settingsChange.theme) details.push(`Theme: ${settingsChange.theme}`);
  if (settingsChange.wallpaper) {
    const kind =
      "type" in settingsChange.wallpaper
        ? String((settingsChange.wallpaper as { type?: string }).type ?? "wallpaper")
        : String((settingsChange.wallpaper as { kind?: string }).kind ?? "wallpaper");
    details.push(`Wallpaper: ${kind}`);
  }
  if (settingsChange.background) details.push("Background colors");
  if (settingsChange.accentPrimary) details.push("Accent colors");

  return (
    <div className="tool-change-preview" role="region" aria-label="Appearance change preview">
      <h3>Appearance change</h3>
      <p className="muted" style={{ margin: 0 }}>
        {summary}
      </p>
      {details.length > 0 ? (
        <ul className="muted" style={{ margin: "0.5rem 0 0", paddingLeft: "1.25rem" }}>
          {details.map((d) => (
            <li key={d}>{d}</li>
          ))}
        </ul>
      ) : null}
      <div className="preview-actions">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void applyPendingSettingsChange()}
        >
          Apply
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => void discardPendingSettingsChange()}
        >
          Discard
        </button>
      </div>
    </div>
  );
}
