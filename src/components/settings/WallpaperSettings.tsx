import { useMemo, useState } from "react";
import type { WallpaperKind } from "@/types/agent";
import {
  buildCanvasPresetProposal,
  schemaWallpaperToJson,
} from "@/types/wallpaper";
import { CANVAS_PRESETS } from "@/types/wallpaper";
import { activeCanvasPresetId } from "@/lib/wallpaper";
import { consumerErrorMessage } from "@/lib/consumer-errors";
import {
  INTERFACE_TRANSPARENCY_MAX,
  INTERFACE_TRANSPARENCY_MIN,
  INTERFACE_TRANSPARENCY_PRESETS,
  INTERFACE_TRANSPARENCY_STEP,
} from "@/lib/interface-transparency";
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
  const interfaceTransparency = useAppStore((s) => s.interfaceTransparency);
  const setInterfaceTransparency = useAppStore((s) => s.setInterfaceTransparency);
  const developerMode = useAppStore((s) => s.developerMode);
  const navigateToMedia = useAppStore((s) => s.navigateToMedia);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [techDetail, setTechDetail] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const activeId = activeCanvasPresetId({
    globalWallpaperJson,
    globalWallpaper: wallpaper,
  });
  const wallpaperActive = activeId !== "none";

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
    setTechDetail(null);
    try {
      if (preset === "none") {
        // Clear schema + legacy stores. Do not POST `{ kind: "none" }` as
        // wallpaperJson — Rust validate_wallpaper_config requires schemaVersion.
        await applyWorkspaceWallpaper("");
        return;
      }
      const proposal = buildCanvasPresetProposal(preset);
      if ("kind" in proposal) {
        await applyWorkspaceWallpaper("");
        return;
      }
      await applyWorkspaceWallpaper(schemaWallpaperToJson(proposal));
    } catch (err) {
      const { message, technical } = consumerErrorMessage(
        err,
        "Coreside could not apply that wallpaper. The previous appearance was restored.",
      );
      setError(message);
      setTechDetail(technical);
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

      <div
        className="interface-transparency-control"
        aria-labelledby="interface-transparency-heading"
      >
        <h4 className="settings-subheading" id="interface-transparency-heading">
          Interface transparency
        </h4>
        <p>
          Controls how much of the active wallpaper shows through Coreside’s
          panels. Readability protection may strengthen individual surfaces when
          needed.
          {!wallpaperActive
            ? " Has no visual effect while None is active."
            : null}
        </p>
        <label>
          <span className="sr-only">Interface transparency percent</span>
          <input
            type="range"
            min={INTERFACE_TRANSPARENCY_MIN}
            max={INTERFACE_TRANSPARENCY_MAX}
            step={INTERFACE_TRANSPARENCY_STEP}
            value={interfaceTransparency}
            aria-valuemin={INTERFACE_TRANSPARENCY_MIN}
            aria-valuemax={INTERFACE_TRANSPARENCY_MAX}
            aria-valuenow={interfaceTransparency}
            aria-valuetext={`${interfaceTransparency} percent`}
            disabled={busy}
            onChange={(e) =>
              void setInterfaceTransparency(Number(e.target.value))
            }
          />
        </label>
        <div className="button-row">
          <output aria-live="polite">{interfaceTransparency}%</output>
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy || interfaceTransparency === 20}
            onClick={() => void setInterfaceTransparency(20)}
          >
            Reset
          </button>
        </div>
        <div
          className="transparency-presets"
          role="group"
          aria-label="Transparency presets"
        >
          {INTERFACE_TRANSPARENCY_PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              className="btn btn-secondary"
              aria-pressed={interfaceTransparency === preset.value}
              disabled={busy}
              onClick={() => void setInterfaceTransparency(preset.value)}
            >
              {preset.label}
            </button>
          ))}
        </div>
      </div>

      {error ? (
        <p className="form-error" role="alert">
          {error}
          {developerMode && techDetail ? (
            <span className="muted"> — {techDetail}</span>
          ) : null}
        </p>
      ) : null}
    </section>
  );
}
