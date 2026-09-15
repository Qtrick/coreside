import { useEffect, useMemo, useRef, useState } from "react";
import type { WallpaperKind } from "@/types/agent";
import {
  buildCanvasPresetProposal,
  buildStaticColorProposal,
  CANVAS_PRESETS,
  parseWallpaperJson,
  schemaWallpaperToJson,
  WALLPAPER_FILTER_PRESETS,
  type WallpaperFilterConfig,
} from "@/types/wallpaper";
import { normalizeCanonicalHex } from "@/lib/wallpaper-hex";
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

const DEFAULT_SOLID = "#141714";

function readCommittedSolid(globalWallpaperJson: string | null | undefined): {
  color: string;
  filterPreset: NonNullable<WallpaperFilterConfig["preset"]>;
} {
  if (!globalWallpaperJson?.trim()) {
    return { color: DEFAULT_SOLID, filterPreset: "none" };
  }
  const parsed = parseWallpaperJson(globalWallpaperJson);
  if (parsed.format === "schema" && parsed.config.type === "static-color") {
    const color = normalizeCanonicalHex(parsed.config.color ?? "") ?? DEFAULT_SOLID;
    const preset = parsed.config.filter?.preset ?? "none";
    return {
      color,
      filterPreset: preset,
    };
  }
  return { color: DEFAULT_SOLID, filterPreset: "none" };
}

export function WallpaperSettings() {
  const wallpaper = useAppStore((s) => s.wallpaper);
  const globalWallpaperJson = useAppStore((s) => s.globalWallpaperJson);
  const applyWorkspaceWallpaper = useAppStore((s) => s.applyWorkspaceWallpaper);
  const interfaceTransparency = useAppStore((s) => s.interfaceTransparency);
  const previewInterfaceTransparency = useAppStore(
    (s) => s.previewInterfaceTransparency,
  );
  const commitInterfaceTransparency = useAppStore(
    (s) => s.commitInterfaceTransparency,
  );
  const developerMode = useAppStore((s) => s.developerMode);
  const navigateToMedia = useAppStore((s) => s.navigateToMedia);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [techDetail, setTechDetail] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  /** Guards pointerup+blur (and keyup+blur) double-commit of the same gesture. */
  const transparencyCommitGenRef = useRef(0);

  const committedSolid = useMemo(
    () => readCommittedSolid(globalWallpaperJson),
    [globalWallpaperJson],
  );
  const [draftHex, setDraftHex] = useState(committedSolid.color);
  const [draftFilter, setDraftFilter] = useState(committedSolid.filterPreset);

  useEffect(() => {
    setDraftHex(committedSolid.color);
    setDraftFilter(committedSolid.filterPreset);
  }, [committedSolid.color, committedSolid.filterPreset]);

  const draftCanonical = normalizeCanonicalHex(draftHex);
  const draftDirty =
    (draftCanonical ?? draftHex.toLowerCase()) !== committedSolid.color ||
    draftFilter !== committedSolid.filterPreset;
  const solidActive = useMemo(() => {
    if (!globalWallpaperJson?.trim()) return false;
    const parsed = parseWallpaperJson(globalWallpaperJson);
    return parsed.format === "schema" && parsed.config.type === "static-color";
  }, [globalWallpaperJson]);

  const activeId = activeCanvasPresetId({
    globalWallpaperJson,
    globalWallpaper: wallpaper,
  });
  // Transparency applies whenever any wallpaper layer is active — not only canvas presets.
  // (activeCanvasPresetId maps static-color → "none" for preset selection UX.)
  const wallpaperActive = useMemo(() => {
    if (globalWallpaperJson?.trim()) {
      const parsed = parseWallpaperJson(globalWallpaperJson);
      if (parsed.format !== "none") return true;
    }
    return Boolean(wallpaper.kind && wallpaper.kind !== "none");
  }, [globalWallpaperJson, wallpaper]);

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
        "Coreside could not apply that wallpaper.",
      );
      setError(message);
      setTechDetail(technical);
    } finally {
      setBusy(false);
    }
  };

  const applySolidDraft = async () => {
    if (!draftCanonical) {
      setError("Enter a valid #RRGGBB color.");
      setTechDetail(null);
      return;
    }
    setBusy(true);
    setError(null);
    setTechDetail(null);
    try {
      const filter: WallpaperFilterConfig | null =
        draftFilter === "none" ? null : { preset: draftFilter };
      const proposal = buildStaticColorProposal(draftCanonical, filter);
      if (!proposal) {
        setError("Enter a valid #RRGGBB color.");
        return;
      }
      await applyWorkspaceWallpaper(schemaWallpaperToJson(proposal));
    } catch (err) {
      const { message, technical } = consumerErrorMessage(
        err,
        "Coreside could not apply that solid color.",
      );
      setError(message);
      setTechDetail(technical);
    } finally {
      setBusy(false);
    }
  };

  const cancelSolidDraft = () => {
    setDraftHex(committedSolid.color);
    setDraftFilter(committedSolid.filterPreset);
    setError(null);
    setTechDetail(null);
  };

  const resetSolid = async () => {
    setDraftHex(DEFAULT_SOLID);
    setDraftFilter("none");
    if (!solidActive) return;
    setBusy(true);
    setError(null);
    setTechDetail(null);
    try {
      await applyWorkspaceWallpaper("");
    } catch (err) {
      const { message, technical } = consumerErrorMessage(
        err,
        "Coreside could not reset the solid color.",
      );
      setError(message);
      setTechDetail(technical);
    } finally {
      setBusy(false);
    }
  };

  const commitTransparency = async (value: number) => {
    const gen = ++transparencyCommitGenRef.current;
    setError(null);
    setTechDetail(null);
    try {
      await commitInterfaceTransparency(value);
    } catch (err) {
      // Superseded gesture — do not overwrite a newer commit's error/UI state.
      if (gen !== transparencyCommitGenRef.current) return;
      const { message, technical } = consumerErrorMessage(
        err,
        "Coreside could not save interface transparency. The previous value was restored.",
      );
      setError(message);
      setTechDetail(technical);
    }
  };

  const [selectedCategory, setSelectedCategory] = useState<"all" | "live" | "solid" | "media">("all");

  const activeTitle = useMemo(() => {
    if (solidActive) {
      return `Solid Color (${committedSolid.color})`;
    }
    if (activeId && activeId !== "none") {
      const p = CANVAS_PRESETS.find((cp) => cp.id === activeId);
      return p ? p.label : activeId;
    }
    return "None (Default Surface)";
  }, [solidActive, committedSolid.color, activeId]);

  const activeCategoryDesc = useMemo(() => {
    if (solidActive) return "Solid Color";
    if (activeId && activeId !== "none") return LIVE_PRESET_IDS.has(activeId) ? "Live Ambient Canvas" : "Preset";
    return "No active wallpaper layer";
  }, [solidActive, activeId]);

  const displayedPresets = useMemo(() => {
    if (selectedCategory === "live") {
      return filteredPresets.filter((p) => LIVE_PRESET_IDS.has(p.id));
    }
    return filteredPresets;
  }, [filteredPresets, selectedCategory]);

  return (
    <section className="settings-subsection wallpaper-settings" aria-labelledby="wallpaper-heading">
      <h4 className="settings-subheading" id="wallpaper-heading">
        Wallpapers
      </h4>
      <p>
        Choose a workspace background preset or import media from the library.
        Project chats can override this with their own wallpaper. Preview applies
        immediately; durable save follows.
      </p>

      {/* Active Wallpaper Banner */}
      <div className="wallpaper-active-banner">
        <div className="wallpaper-active-banner-info">
          <div
            className={`wallpaper-active-preview ${
              solidActive ? "" : `wallpaper-thumb-${activeId}`
            }`}
            style={solidActive ? { background: committedSolid.color } : undefined}
            aria-hidden
          />
          <div>
            <div style={{ fontWeight: 600, fontSize: "0.95rem" }}>{activeTitle}</div>
            <div className="muted" style={{ fontSize: "0.8rem" }}>{activeCategoryDesc}</div>
          </div>
        </div>
        {wallpaperActive ? (
          <button
            type="button"
            className="btn btn-ghost"
            style={{ fontSize: "0.82rem" }}
            disabled={busy}
            onClick={() => void applyPreset("none")}
          >
            Reset to None
          </button>
        ) : null}
      </div>

      {/* Category Tabs */}
      <div className="wallpaper-category-tabs" role="tablist" aria-label="Wallpaper categories">
        <button
          type="button"
          role="tab"
          className="wallpaper-category-tab"
          aria-selected={selectedCategory === "all"}
          onClick={() => setSelectedCategory("all")}
        >
          All Presets
        </button>
        <button
          type="button"
          role="tab"
          className="wallpaper-category-tab"
          aria-selected={selectedCategory === "live"}
          onClick={() => setSelectedCategory("live")}
        >
          Live Ambient
        </button>
        <button
          type="button"
          role="tab"
          className="wallpaper-category-tab"
          aria-selected={selectedCategory === "solid"}
          onClick={() => setSelectedCategory("solid")}
        >
          Solid Color
        </button>
        <button
          type="button"
          role="tab"
          className="wallpaper-category-tab"
          aria-selected={selectedCategory === "media"}
          onClick={() => setSelectedCategory("media")}
        >
          Media Library
        </button>
      </div>

      {selectedCategory !== "solid" && selectedCategory !== "media" ? (
        <>
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
            {displayedPresets.map((preset) => {
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
                  <div
                    className={`wallpaper-preset-thumbnail wallpaper-thumb-${preset.id}`}
                    aria-hidden
                  />
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

          {displayedPresets.length === 0 ? (
            <p className="muted">No presets match “{query.trim()}”.</p>
          ) : null}
        </>
      ) : null}

      {(selectedCategory === "all" || selectedCategory === "solid") ? (
        <div className="wallpaper-solid-color" aria-labelledby="wallpaper-solid-heading">
          <h4 className="settings-subheading" id="wallpaper-solid-heading">
            Solid color
          </h4>
          <p>
            Pick a workspace solid color. Draft changes preview locally below;
            Apply saves. Filters affect only the wallpaper layer.
          </p>
          <div className="wallpaper-solid-row">
            <label className="wallpaper-solid-swatch">
              <span className="sr-only">Solid color</span>
              <input
                type="color"
                value={draftCanonical ?? committedSolid.color}
                disabled={busy}
                onChange={(e) => setDraftHex(e.target.value)}
              />
            </label>
            <label className="wallpaper-solid-hex">
              <span className="sr-only">Hex color</span>
              <input
                type="text"
                spellCheck={false}
                autoComplete="off"
                placeholder="#RRGGBB"
                value={draftHex}
                disabled={busy}
                aria-invalid={draftHex.trim().length > 0 && !draftCanonical}
                onChange={(e) => setDraftHex(e.target.value)}
              />
            </label>
            <div
              className="wallpaper-solid-preview"
              style={{ background: draftCanonical ?? "transparent" }}
              aria-hidden
              title="Draft preview"
            />
          </div>
          <div
            className="wallpaper-filter-presets"
            role="group"
            aria-label="Wallpaper filter presets"
          >
            {WALLPAPER_FILTER_PRESETS.map((preset) => (
              <button
                key={preset.id}
                type="button"
                className="btn btn-secondary"
                aria-pressed={draftFilter === preset.id}
                disabled={busy}
                onClick={() => setDraftFilter(preset.id)}
              >
                {preset.label}
              </button>
            ))}
          </div>
          <div className="button-row">
            <button
              type="button"
              className="btn btn-primary"
              disabled={busy || !draftDirty || !draftCanonical}
              onClick={() => void applySolidDraft()}
            >
              Apply
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busy || !draftDirty}
              onClick={cancelSolidDraft}
            >
              Cancel
            </button>
            <button
              type="button"
              className="btn btn-ghost"
              disabled={busy}
              onClick={() => void resetSolid()}
            >
              Reset
            </button>
          </div>
        </div>
      ) : null}

      {(selectedCategory === "all" || selectedCategory === "media") ? (
        <div style={{ marginTop: "var(--space-4)" }}>
          <h4 className="settings-subheading">Media Library</h4>
          <p className="muted" style={{ marginBottom: "var(--space-2)" }}>
            Use custom local images or videos from your library as workspace or project backgrounds.
          </p>
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
        </div>
      ) : null}

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
              previewInterfaceTransparency(Number(e.target.value))
            }
            onPointerUp={(e) =>
              void commitTransparency(Number(e.currentTarget.value))
            }
            onKeyUp={(e) =>
              void commitTransparency(Number(e.currentTarget.value))
            }
            onBlur={(e) =>
              void commitTransparency(Number(e.currentTarget.value))
            }
          />
        </label>
        <div className="button-row">
          <output aria-live="polite">{interfaceTransparency}%</output>
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy || interfaceTransparency === 20}
            onClick={() => void commitTransparency(20)}
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
              onClick={() => void commitTransparency(preset.value)}
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
