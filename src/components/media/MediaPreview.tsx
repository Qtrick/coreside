import { useEffect, useState } from "react";
import { resolveMediaAssetUrl, resolveMediaThumbUrl } from "@/lib/media";
import type { MediaAsset } from "@/types/media";

type MediaPreviewProps = {
  asset: MediaAsset;
  className?: string;
  autoPlay?: boolean;
  muted?: boolean;
  loop?: boolean;
  objectFit?: "cover" | "contain";
  /** Prefer generated thumbnail when available (grid cards). */
  preferThumbnail?: boolean;
};

export function MediaPreview({
  asset,
  className = "",
  autoPlay = false,
  muted = true,
  loop = true,
  objectFit = "cover",
  preferThumbnail = false,
}: MediaPreviewProps) {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    const loader = preferThumbnail
      ? resolveMediaThumbUrl(asset)
      : resolveMediaAssetUrl(asset.id);
    void loader
      .then((url) => {
        if (!cancelled) setSrc(url);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Preview unavailable");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [asset, preferThumbnail]);

  if (error) {
    return (
      <div className={`media-preview media-preview-error ${className}`.trim()}>
        <span>{error}</span>
      </div>
    );
  }

  if (!src) {
    return <div className={`media-preview media-preview-loading ${className}`.trim()} />;
  }

  // Thumbnail JPEGs are always images even for video assets (when generated later).
  const showVideo =
    !preferThumbnail &&
    (asset.category === "video" || asset.mimeType.startsWith("video/"));

  if (showVideo) {
    return (
      <video
        className={`media-preview media-preview-video ${className}`.trim()}
        src={src}
        muted={muted}
        loop={loop}
        playsInline
        autoPlay={autoPlay}
        preload="metadata"
        style={{ objectFit }}
      />
    );
  }

  return (
    <img
      className={`media-preview media-preview-image ${className}`.trim()}
      src={src}
      alt={asset.title}
      loading="lazy"
      style={{ objectFit }}
    />
  );
}
