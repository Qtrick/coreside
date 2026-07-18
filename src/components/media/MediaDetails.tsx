import { useEffect, useState } from "react";
import { ExternalLink, Trash2, Wallpaper } from "lucide-react";
import { api } from "@/lib/tauri";
import { MediaPreview } from "@/components/media/MediaPreview";
import { formatByteSize, mediaCategoryLabel } from "@/types/media";
import type { MediaAsset, MediaAssetUsage } from "@/types/media";

type MediaDetailsProps = {
  asset: MediaAsset;
  busy?: boolean;
  onDelete: (asset: MediaAsset, usages: MediaAssetUsage[]) => Promise<void>;
  onUseWorkspaceWallpaper: (asset: MediaAsset) => void;
  onUseProjectWallpaper?: (asset: MediaAsset) => void;
  projectLabel?: string | null;
};

export function MediaDetails({
  asset,
  busy,
  onDelete,
  onUseWorkspaceWallpaper,
  onUseProjectWallpaper,
  projectLabel,
}: MediaDetailsProps) {
  const [usages, setUsages] = useState<MediaAssetUsage[]>([]);
  const [usageLoading, setUsageLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setUsageLoading(true);
    void api
      .mediaAssetUsage(asset.id)
      .then((rows) => {
        if (!cancelled) setUsages(rows);
      })
      .catch(() => {
        if (!cancelled) setUsages([]);
      })
      .finally(() => {
        if (!cancelled) setUsageLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [asset.id]);

  const dims =
    asset.width && asset.height ? `${asset.width}×${asset.height}` : null;
  const duration =
    asset.durationMs && asset.durationMs > 0
      ? `${Math.round(asset.durationMs / 1000)}s`
      : null;

  return (
    <aside className="media-details" aria-label="Asset details">
      <div className="media-details-preview">
        <MediaPreview
          asset={asset}
          autoPlay={asset.category === "video"}
          objectFit="contain"
        />
      </div>

      <div className="media-details-body">
        <h2>{asset.title}</h2>
        <p className="media-details-summary">
          {mediaCategoryLabel(asset.category)} · {formatByteSize(asset.byteSize)}
          {dims ? ` · ${dims}` : ""}
          {duration ? ` · ${duration}` : ""}
          {asset.thumbnailFilename ? " · thumb" : ""}
        </p>

        <dl className="media-details-list">
          {asset.creator ? (
            <>
              <dt>Creator</dt>
              <dd>{asset.creator}</dd>
            </>
          ) : null}
          {asset.license ? (
            <>
              <dt>License</dt>
              <dd>{asset.license}</dd>
            </>
          ) : null}
          {asset.attribution ? (
            <>
              <dt>Attribution</dt>
              <dd>{asset.attribution}</dd>
            </>
          ) : null}
          {asset.sourcePageUrl ? (
            <>
              <dt>Source</dt>
              <dd>
                <a href={asset.sourcePageUrl} target="_blank" rel="noreferrer">
                  {asset.sourcePageUrl}
                  <ExternalLink size={12} aria-hidden />
                </a>
              </dd>
            </>
          ) : null}
          <dt>Imported</dt>
          <dd>{new Date(asset.createdAt).toLocaleString()}</dd>
        </dl>

        {!usageLoading && usages.length > 0 ? (
          <p className="media-details-warning" role="status">
            Used as wallpaper in {usages.map((u) => u.projectName).join(", ")}.
          </p>
        ) : null}

        <div className="media-details-actions">
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() => onUseWorkspaceWallpaper(asset)}
          >
            <Wallpaper size={16} aria-hidden />
            Workspace background
          </button>
          {onUseProjectWallpaper ? (
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busy}
              onClick={() => onUseProjectWallpaper(asset)}
            >
              <Wallpaper size={16} aria-hidden />
              {projectLabel ? `${projectLabel} wallpaper` : "Project wallpaper"}
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-danger"
            disabled={busy}
            onClick={() => void onDelete(asset, usages)}
          >
            <Trash2 size={16} aria-hidden />
            Delete
          </button>
        </div>
      </div>
    </aside>
  );
}
