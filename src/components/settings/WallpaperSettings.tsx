import { useEffect, useMemo, useRef, useState } from "react";
import { Check, RotateCcw, Sliders, ExternalLink } from "lucide-react";
import type { WallpaperKind } from "@/types/agent";
import {
  buildCanvasPresetProposal,
  buildStaticColorProposal,
  CURATED_WALLPAPER_PRESETS,
  type WallpaperPresetDefinition,
  parseWallpaperJson,
  schemaWallpaperToJson,
  WALLPAPER_FILTER_PRESETS,
  type WallpaperFilterConfig,
  type SchemaWallpaperConfig,
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
  const previewWorkspaceWallpaper = useAppStore((s) => s.previewWorkspaceWallpaper);
  const revertWorkspaceWallpaper = useAppStore((s) => s.revertWorkspaceWallpaper);
  const interfaceTransparency = useAppStore((s) => s.interfaceTransparency);
  const previewInterfaceTransparency = useAppStore((s) => s.previewInterfaceTransparency);
  const commitInterfaceTransparency = useAppStore((s) => s.commitInterfaceTransparency);
  const developerMode = useAppStore((s) => s.developerMode);
  const navigateToMedia = useAppStore((s) => s.navigateToMedia);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [techDetail, setTechDetail] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selectedCategory, setSelectedCategory] = useState<
    "all" | "ambient" | "minimal" | "space" | "particles" | "rain" | "motion" | "solid" | "legacy" | "media"
  >("all");

  const [previewPreset, setPreviewPreset] = useState<WallpaperPresetDefinition | null>(null);
  const [previewOpacity, setPreviewOpacity] = useState<number>(0.85);

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

  const wallpaperActive = useMemo(() => {
    if (globalWallpaperJson?.trim()) {
      const parsed = parseWallpaperJson(globalWallpaperJson);
      if (parsed.format !== "none") return true;
    }
    return Boolean(wallpaper.kind && wallpaper.kind !== "none");
  }, [globalWallpaperJson, wallpaper]);

  // Match current active preset
  const activePreset = useMemo(() => {
    return CURATED_WALLPAPER_PRESETS.find((p) => {
      if (p.id === activeId) return true;
      if (globalWallpaperJson && p.config.color) {
        return globalWallpaperJson.includes(p.config.color);
      }
      return false;
    });
  }, [activeId, globalWallpaperJson]);

  const activeTitle = useMemo(() => {
    if (previewPreset) {
      return `${previewPreset.name} (Previewing)`;
    }
    if (solidActive) {
      return `Solid Color (${committedSolid.color})`;
    }
    if (activePreset) {
      return activePreset.name;
    }
    if (activeId && activeId !== "none") {
      return activeId;
    }
    return "None (Default)";
  }, [previewPreset, solidActive, committedSolid.color, activePreset, activeId]);

  const activeCategoryDesc = useMemo(() => {
    if (previewPreset) return `Preview · ${previewPreset.category}`;
    if (solidActive) return "Solid Color";
    if (activePreset) return activePreset.category.toUpperCase();
    if (activeId && activeId !== "none") return "Preset";
    return "No active wallpaper layer";
  }, [previewPreset, solidActive, activePreset, activeId]);

  const filteredPresets = useMemo(() => {
    let list = CURATED_WALLPAPER_PRESETS;
    if (selectedCategory !== "all" && selectedCategory !== "media") {
      list = list.filter((p) => p.category === selectedCategory);
    }
    const q = query.trim().toLowerCase();
    if (!q) return list;
    return list.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.description.toLowerCase().includes(q) ||
        p.category.toLowerCase().includes(q),
    );
  }, [selectedCategory, query]);

  const handleSelectPreset = (preset: WallpaperPresetDefinition) => {
    setPreviewPreset(preset);
    setPreviewOpacity(preset.config.opacity ?? 0.85);
    const json = schemaWallpaperToJson(preset.config);
    previewWorkspaceWallpaper(json);
  };

  const handleOpacityChange = (val: number) => {
    setPreviewOpacity(val);
    if (!previewPreset) return;
    const updatedConfig: SchemaWallpaperConfig = {
      ...previewPreset.config,
      opacity: val,
    };
    previewWorkspaceWallpaper(schemaWallpaperToJson(updatedConfig));
  };

  const applyPreviewPreset = async () => {
    if (!previewPreset) return;
    setBusy(true);
    setError(null);
    setTechDetail(null);
    try {
      const configToApply: SchemaWallpaperConfig = {
        ...previewPreset.config,
        opacity: previewOpacity,
      };
      await applyWorkspaceWallpaper(schemaWallpaperToJson(configToApply));
      setPreviewPreset(null);
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

  const revertPreview = () => {
    revertWorkspaceWallpaper();
    setPreviewPreset(null);
    setError(null);
  };

  const applyPresetDirect = async (presetId: WallpaperKind | "none") => {
    setBusy(true);
    setError(null);
    setTechDetail(null);
    setPreviewPreset(null);
    try {
      if (presetId === "none") {
        await applyWorkspaceWallpaper("");
        return;
      }
      const proposal = buildCanvasPresetProposal(presetId);
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
      setPreviewPreset(null);
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
      setPreviewPreset(null);
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
      if (gen !== transparencyCommitGenRef.current) return;
      const { message, technical } = consumerErrorMessage(
        err,
        "Coreside could not save interface transparency. The previous value was restored.",
      );
      setError(message);
      setTechDetail(technical);
    }
  };

  const previewStyle = useMemo(() => {
    if (previewPreset) {
      const c = previewPreset.previewColors;
      return {
        background: `linear-gradient(135deg, ${c[0]} 0%, ${c[1]} 60%, ${c[2] ?? c[1]} 100%)`,
      };
    }
    if (activePreset) {
      const c = activePreset.previewColors;
      return {
        background: `linear-gradient(135deg, ${c[0]} 0%, ${c[1]} 60%, ${c[2] ?? c[1]} 100%)`,
      };
    }
    if (solidActive) {
      return { background: committedSolid.color };
    }
    return { background: "var(--surface-secondary, #252538)" };
  }, [previewPreset, activePreset, solidActive, committedSolid.color]);

  return (
    <section className="settings-subsection wallpaper-settings" aria-labelledby="wallpaper-heading">
      <h4 className="settings-subheading" id="wallpaper-heading">
        Personalization & Wallpaper
      </h4>
      <p>
        Elevate Coreside with calm, restrained ambient backgrounds. Previews apply
        instantly behind the interface without committing until confirmed.
      </p>

      {/* Large Live Preview Card with Real Coreside UI Shell Mockup */}
      <div className="wallpaper-hero-card" style={previewStyle}>
        <div
          className="wallpaper-mini-shell"
          aria-hidden
          style={{ opacity: Math.max(0.85, 1 - interfaceTransparency * 0.4) }}
        >
          {/* Mini Window Chrome */}
          <div className="mini-shell-titlebar">
            <div className="mini-traffic-lights">
              <span className="dot dot-close" />
              <span className="dot dot-min" />
              <span className="dot dot-max" />
            </div>
            <span className="mini-shell-title">Task Manager — Coreside</span>
            <div className="mini-shell-status">Live Preview</div>
          </div>
          {/* Mini Window Body */}
          <div className="mini-shell-body">
            {/* Sidebar */}
            <div className="mini-shell-sidebar">
              <div className="mini-nav-item">Home</div>
              <div className="mini-nav-item active">
                Tasks <span className="mini-badge">4</span>
              </div>
              <div className="mini-nav-item">Analytics</div>
              <div className="mini-nav-item">Settings</div>
            </div>
            {/* Main Area */}
            <div className="mini-shell-main">
              <div className="mini-toolbar">
                <span className="mini-section-title">Sprint Tasks</span>
                <div className="mini-toolbar-actions">
                  <span className="mini-chip">Filter: All ▾</span>
                  <span className="mini-btn-primary">+ New</span>
                </div>
              </div>
              <div className="mini-content-grid">
                <div className="mini-stat-card">
                  <span className="mini-stat-label">Active Tasks</span>
                  <span className="mini-stat-value">4 remaining</span>
                </div>
                <div className="mini-stat-card">
                  <span className="mini-stat-label">Velocity</span>
                  <span className="mini-stat-value">5.2 pts/day</span>
                </div>
              </div>
              <div className="mini-table">
                <div className="mini-row header">
                  <span>Task</span>
                  <span>Priority</span>
                  <span>Status</span>
                </div>
                <div className="mini-row selected">
                  <span>Action authority & validation</span>
                  <span className="mini-tag high">High</span>
                  <span className="mini-tag done">Done</span>
                </div>
                <div className="mini-row">
                  <span>Progressive dependency stream</span>
                  <span className="mini-tag med">Med</span>
                  <span className="mini-tag in-prog">Active</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="wallpaper-hero-overlay">
          <div className="wallpaper-hero-info">
            <span className="badge">{activeCategoryDesc}</span>
            <h3 className="wallpaper-hero-title">{activeTitle}</h3>
            {previewPreset ? (
              <p className="wallpaper-hero-desc">{previewPreset.description}</p>
            ) : activePreset ? (
              <p className="wallpaper-hero-desc">{activePreset.description}</p>
            ) : null}
          </div>

          <div className="wallpaper-hero-actions">
            {previewPreset ? (
              <>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={busy}
                  onClick={() => void applyPreviewPreset()}
                >
                  <Check size={14} aria-hidden /> Apply Wallpaper
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={busy}
                  onClick={revertPreview}
                >
                  <RotateCcw size={14} aria-hidden /> Revert
                </button>
              </>
            ) : wallpaperActive ? (
              <button
                type="button"
                className="btn btn-ghost"
                disabled={busy}
                onClick={() => void applyPresetDirect("none")}
              >
                Reset to None
              </button>
            ) : null}
          </div>
        </div>
      </div>

      {/* Fine-Tuning Slider if Previewing */}
      {previewPreset && (
        <div className="wallpaper-tuning-panel">
          <div className="tuning-header">
            <Sliders size={14} aria-hidden />
            <strong>Adjust Preset Intensity</strong>
          </div>
          <div className="tuning-control">
            <label htmlFor="wallpaper-opacity-slider">Opacity ({Math.round(previewOpacity * 100)}%)</label>
            <input
              id="wallpaper-opacity-slider"
              type="range"
              min={0.1}
              max={1.0}
              step={0.05}
              value={previewOpacity}
              onChange={(e) => handleOpacityChange(parseFloat(e.target.value))}
            />
          </div>
        </div>
      )}

      {/* Category Tabs */}
      <div className="wallpaper-category-tabs" role="tablist" aria-label="Wallpaper categories">
        {(["all", "ambient", "minimal", "space", "particles", "rain", "motion", "solid", "legacy", "media"] as const).map((cat) => (
          <button
            key={cat}
            type="button"
            role="tab"
            className={`wallpaper-category-tab${selectedCategory === cat ? " active" : ""}`}
            aria-selected={selectedCategory === cat}
            onClick={() => setSelectedCategory(cat)}
          >
            {cat === "all"
              ? "All Presets"
              : cat === "ambient"
                ? "Ambient"
                : cat === "minimal"
                  ? "Minimal"
                  : cat === "space"
                    ? "Space"
                    : cat === "particles"
                      ? "Particles"
                      : cat === "rain"
                        ? "Rain"
                        : cat === "motion"
                          ? "Motion"
                          : cat === "solid"
                            ? "Solid"
                            : cat === "legacy"
                              ? "Legacy"
                              : "Media Library"}
          </button>
        ))}
      </div>

      {/* Preset Cards Grid */}
      {selectedCategory !== "solid" && selectedCategory !== "media" ? (
        <>
          <label className="wallpaper-preset-search">
            <span className="sr-only">Search wallpaper presets</span>
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search curated wallpapers…"
              disabled={busy}
            />
          </label>

          <div className="wallpaper-curated-grid" role="group" aria-label="Curated presets">
            {filteredPresets.map((preset) => {
              const isCurrent = previewPreset?.id === preset.id || activeId === preset.id;
              const swatchStyle = {
                background: `linear-gradient(135deg, ${preset.previewColors[0]} 0%, ${preset.previewColors[1]} 60%, ${preset.previewColors[2] ?? preset.previewColors[1]} 100%)`,
              };
              return (
                <button
                  key={preset.id}
                  type="button"
                  className={`wallpaper-card-item${isCurrent ? " is-active" : ""}`}
                  aria-pressed={isCurrent}
                  disabled={busy}
                  onClick={() => handleSelectPreset(preset)}
                >
                  <div className="wallpaper-card-swatch" style={swatchStyle} aria-hidden>
                    {isCurrent && (
                      <div className="swatch-check">
                        <Check size={14} />
                      </div>
                    )}
                  </div>
                  <div className="wallpaper-card-content">
                    <div className="wallpaper-card-top">
                      <span className="wallpaper-card-name">{preset.name}</span>
                      <span className="wallpaper-cat-tag">{preset.category}</span>
                    </div>
                    <span className="wallpaper-card-desc">{preset.description}</span>
                  </div>
                </button>
              );
            })}
          </div>

          {filteredPresets.length === 0 ? (
            <p className="muted">No presets match “{query.trim()}”.</p>
          ) : null}
        </>
      ) : null}

      {/* Solid Color Tab */}
      {selectedCategory === "all" || selectedCategory === "solid" ? (
        <div className="wallpaper-solid-color" aria-labelledby="wallpaper-solid-heading">
          <h4 className="settings-subheading" id="wallpaper-solid-heading">
            Solid Color
          </h4>
          <p>
            Choose a precise matte workspace tone. Draft changes preview locally below;
            Apply saves.
          </p>
          <div className="wallpaper-solid-row">
            <label className="wallpaper-solid-swatch">
              <span className="sr-only">Solid color picker</span>
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
          <div className="wallpaper-filter-presets" role="group" aria-label="Wallpaper filter presets">
            {WALLPAPER_FILTER_PRESETS.map((p) => (
              <button
                key={p.id}
                type="button"
                className="btn btn-secondary btn-sm"
                aria-pressed={draftFilter === p.id}
                disabled={busy}
                onClick={() => setDraftFilter(p.id)}
              >
                {p.label}
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
              Apply Color
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

      {/* Media Library Tab */}
      {selectedCategory === "all" || selectedCategory === "media" ? (
        <div style={{ marginTop: "var(--space-4)" }}>
          <h4 className="settings-subheading">Media Library Wallpapers</h4>
          <p className="muted" style={{ marginBottom: "var(--space-2)" }}>
            Select local imported images or looping videos from your personal media library.
          </p>
          <div className="button-row">
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busy}
              onClick={() => navigateToMedia()}
            >
              <ExternalLink size={14} aria-hidden /> Open Media Library
            </button>
          </div>
        </div>
      ) : null}

      {/* Interface Transparency Slider */}
      <div className="interface-transparency-control" aria-labelledby="interface-transparency-heading">
        <h4 className="settings-subheading" id="interface-transparency-heading">
          Interface Transparency
        </h4>
        <p>
          Controls how much of the background shows through Coreside’s panels.
          Readability protection automatically ensures text remains legible.
          {!wallpaperActive ? " (Has no visual effect while None is active.)" : null}
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
            onChange={(e) => previewInterfaceTransparency(Number(e.target.value))}
            onPointerUp={(e) => void commitTransparency(Number(e.currentTarget.value))}
            onKeyUp={(e) => void commitTransparency(Number(e.currentTarget.value))}
            onBlur={(e) => void commitTransparency(Number(e.currentTarget.value))}
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
        <div className="transparency-presets" role="group" aria-label="Transparency presets">
          {INTERFACE_TRANSPARENCY_PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              className="btn btn-secondary btn-sm"
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
          {developerMode && techDetail ? <span className="muted"> — {techDetail}</span> : null}
        </p>
      ) : null}
    </section>
  );
}
