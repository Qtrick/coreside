import { useCallback, useEffect, useMemo, useState } from "react";
import { ImagePlus, RefreshCw, Search } from "lucide-react";
import { api } from "@/lib/tauri";
import { clearMediaSrcCache } from "@/lib/media";
import { MediaAssetCard } from "@/components/media/MediaAssetCard";
import { MediaDetails } from "@/components/media/MediaDetails";
import { MediaImportDialog } from "@/components/media/MediaImportDialog";
import {
  WallpaperProposalDialog,
  type WallpaperProposalResult,
} from "@/components/wallpaper/WallpaperProposalDialog";
import { useAppStore } from "@/stores/app-store";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import {
  matchesMediaFilter,
  type ImportMediaInput,
  type MediaAsset,
  type MediaAssetUsage,
  type MediaCategoryFilter,
} from "@/types/media";

type MediaLibraryProps = {
  projectId?: string | null;
  projectName?: string | null;
  onBack?: () => void;
};

const FILTERS: Array<{ id: MediaCategoryFilter; label: string }> = [
  { id: "all", label: "All" },
  { id: "image", label: "Images" },
  { id: "video", label: "Video" },
  { id: "animated", label: "Animated" },
];

export function MediaLibrary({
  projectId = null,
  projectName = null,
  onBack,
}: MediaLibraryProps) {
  const applyWorkspaceWallpaper = useAppStore((s) => s.applyWorkspaceWallpaper);
  const applyProjectWallpaper = useAppStore((s) => s.applyProjectWallpaper);
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);

  const [assets, setAssets] = useState<MediaAsset[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<MediaCategoryFilter>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [proposal, setProposal] = useState<{
    asset: MediaAsset;
    target: "workspace" | "project";
  } | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await api.listMediaAssets(projectId, 200);
      setAssets(rows);
      setSelectedId((current) =>
        current && rows.some((a) => a.id === current) ? current : rows[0]?.id ?? null,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load media");
      setAssets([]);
      setSelectedId(null);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return assets.filter((asset) => {
      if (!matchesMediaFilter(asset, filter)) return false;
      if (!q) return true;
      return (
        asset.title.toLowerCase().includes(q) ||
        asset.category.toLowerCase().includes(q) ||
        (asset.creator?.toLowerCase().includes(q) ?? false)
      );
    });
  }, [assets, filter, query]);

  const selected =
    filtered.find((a) => a.id === selectedId) ??
    assets.find((a) => a.id === selectedId) ??
    null;

  const importAsset = async (input: ImportMediaInput) => {
    setBusy(true);
    setImportError(null);
    try {
      const asset = await api.importMediaAsset(input);
      clearMediaSrcCache();
      setImportOpen(false);
      await refresh();
      setSelectedId(asset.id);
    } catch (err) {
      setImportError(err instanceof Error ? err.message : "Import failed");
    } finally {
      setBusy(false);
    }
  };

  const deleteAsset = async (asset: MediaAsset, usages: MediaAssetUsage[]) => {
    const usageNote =
      usages.length > 0
        ? `\n\nUsed as wallpaper in: ${usages.map((u) => u.projectName).join(", ")}.`
        : "";
    const ok = window.confirm(
      `Delete “${asset.title}”? This removes the local file.${usageNote}`,
    );
    if (!ok) return;

    setBusy(true);
    setError(null);
    try {
      await api.deleteMediaAsset(asset.id);
      clearMediaSrcCache(asset.id);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Delete failed");
    } finally {
      setBusy(false);
    }
  };

  const applyProposal = async (result: WallpaperProposalResult) => {
    setBusy(true);
    setError(null);
    try {
      if (result.target === "project" && result.projectId) {
        await applyProjectWallpaper(result.projectId, result.wallpaperJson);
      } else {
        await applyWorkspaceWallpaper(result.wallpaperJson);
      }
      if (result.config.assetId) {
        await api.touchMediaAsset(result.config.assetId);
      }
      setProposal(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not apply wallpaper");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="media-library-page" aria-label="Media library">
      <header className="panel-header media-library-header">
        <div>
          {onBack ? (
            <button type="button" className="text-btn" onClick={onBack}>
              ← Back
            </button>
          ) : null}
          <h1>Media library</h1>
          <p className="panel-subtitle">
            {projectName
              ? `Assets for ${projectName} and shared workspace media.`
              : "Import and manage local images, video, and animated assets."}
          </p>
        </div>
        <div className="media-library-header-actions">
          <button
            type="button"
            className="btn btn-secondary"
            disabled={loading}
            onClick={() => void refresh()}
          >
            <RefreshCw size={16} aria-hidden />
            Refresh
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => {
              setImportError(null);
              setImportOpen(true);
            }}
          >
            <ImagePlus size={16} aria-hidden />
            Import
          </button>
        </div>
      </header>

      <div className="media-library-toolbar">
        <label className="media-library-search">
          <Search size={16} aria-hidden />
          <input
            type="search"
            value={query}
            placeholder="Search title or creator…"
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <div className="media-library-filters" role="group" aria-label="Category">
          {FILTERS.map((item) => (
            <button
              key={item.id}
              type="button"
              className="btn btn-secondary"
              aria-pressed={filter === item.id}
              onClick={() => setFilter(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>

      {error ? (
        <p className="form-error media-library-error" role="alert">
          {error}
        </p>
      ) : null}

      <div className="media-library-body">
        <div className="media-library-grid-wrap">
          {loading ? (
            <p className="muted">Loading media…</p>
          ) : filtered.length === 0 ? (
            <div className="empty-state">
              <h3>{EMPTY_STATES.noMedia.title}</h3>
              <p>{EMPTY_STATES.noMedia.body}</p>
              <div className="button-row empty-state-actions">
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => setImportOpen(true)}
                >
                  {EMPTY_STATES.noMedia.primaryCta}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => openHelpAndLearning(navigateToSettings)}
                >
                  {EMPTY_STATES.helpLink.label}
                </button>
              </div>
            </div>
          ) : (
            <div className="media-library-grid">
              {filtered.map((asset) => (
                <MediaAssetCard
                  key={asset.id}
                  asset={asset}
                  selected={selected?.id === asset.id}
                  onSelect={(row) => setSelectedId(row.id)}
                />
              ))}
            </div>
          )}
        </div>

        {selected ? (
          <MediaDetails
            asset={selected}
            busy={busy}
            projectLabel={projectName}
            onDelete={deleteAsset}
            onUseWorkspaceWallpaper={(asset) =>
              setProposal({ asset, target: "workspace" })
            }
            onUseProjectWallpaper={
              projectId
                ? (asset) => setProposal({ asset, target: "project" })
                : undefined
            }
          />
        ) : null}
      </div>

      <MediaImportDialog
        open={importOpen}
        busy={busy}
        error={importError}
        projectId={projectId}
        onClose={() => !busy && setImportOpen(false)}
        onImport={importAsset}
      />

      <WallpaperProposalDialog
        open={proposal !== null}
        asset={proposal?.asset ?? null}
        target={proposal?.target ?? "workspace"}
        projectId={projectId}
        projectName={projectName}
        busy={busy}
        onClose={() => !busy && setProposal(null)}
        onApply={applyProposal}
      />
    </section>
  );
}
