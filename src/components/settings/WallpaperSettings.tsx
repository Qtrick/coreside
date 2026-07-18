import { useMemo, useState } from "react";
import type { WallpaperKind } from "@/types/agent";
import { DEFAULT_WALLPAPER } from "@/types/agent";
import {
  buildCanvasPresetProposal,
  legacyWallpaperToJson,
  schemaWallpaperToJson,
} from "@/types/wallpaper";
import { CANVAS_PRESETS } from "@/types/wallpaper";
import { activeCanvasPresetId } from "@/lib/wallpaper";
import { useAppStore } from "@/stores/app-store";

const LIVE_PRESET_IDS = new Set<WallpaperKind>([
  "matrix",
  "aurora",
  "particles",
  "rain",
  "pulse",
]);

export function WallpaperSettings() {
  const wallpaper = useAppStore((s) => s.wallpaper);
  const globalWallpaperJson = useAppStore((s) => s.globalWallpaperJson);
  const applyWorkspaceWallpaper = useAppStore((s) => s.applyWorkspaceWallpaper);
  const navigateToMedia = useAppStore((s) => s.navigateToMedia);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const activeId = activeCanvasPresetId({
    globalWallpaperJson,
    globalWallpaper: wallpaper,
  });

  const filteredPresets = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return CANVAS_PRESETS;
    return CANVAS_PRESETS.filter((preset) => {
      const live = LIVE_PRESET_IDS.has(preset.id) ? "live" : "";
      return (
        preset.label.toLowerCase().includes(q) ||
        preset.description.toLowerCase().includes(q) ||
        live.includes(q)
      );
    });
  }, [query]);

  const applyPreset = async (preset: WallpaperKind) => {
    setBusy(true);
    setError(null);
    try {
      if (preset === "none") {
        await applyWorkspaceWallpaper(legacyWallpaperToJson(DEFAULT_WALLPAPER));
        return;
      }
      const proposal = buildCanvasPresetProposal(preset);
      if ("kind" in proposal) {
        await applyWorkspaceWallpaper(legacyWallpaperToJson(proposal));
      } else {
        await applyWorkspaceWallpaper(schemaWallpaperToJson(proposal));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not update wallpaper");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="settings-subsection wallpaper-settings" aria-labelledby="wallpaper-heading">
      <h4 className="settings-subheading" id="wallpaper-heading">
        Wallpapers
      </h4>
      <p>
        Choose a workspace background preset or import media from the library.
        Project chats can override this with their own wallpaper. Clicking a
        preset applies it immediately (Apply-only — no separate preview).
      </p>

      <label className="wallpaper-preset-search">
        <span className="sr-only">Search wallpaper presets</span>
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search presets…"
          disabled={busy}
        />
      </label>

      <div className="wallpaper-preset-grid" role="group" aria-label="Wallpaper presets">
        {filteredPresets.map((preset) => {
          const isLive = LIVE_PRESET_IDS.has(preset.id);
          return (
            <button
              key={preset.id}
              type="button"
              className="wallpaper-preset-card"
              aria-pressed={activeId === preset.id}
              disabled={busy}
              onClick={() => void applyPreset(preset.id)}
            >
              <span className="wallpaper-preset-card-top">
                <span className="wallpaper-preset-label">{preset.label}</span>
                {isLive ? (
                  <span className="wallpaper-live-badge" aria-label="Live wallpaper">
                    Live
                  </span>
                ) : null}
              </span>
              <span className="muted">{preset.description}</span>
            </button>
          );
        })}
      </div>

      {filteredPresets.length === 0 ? (
        <p className="muted">No presets match “{query.trim()}”.</p>
      ) : null}

      <div className="button-row">
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() => navigateToMedia()}
        >
          Open media library
        </button>
      </div>

      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
