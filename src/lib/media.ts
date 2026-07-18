import { convertFileSrc } from "@tauri-apps/api/core";
import { api } from "@/lib/tauri";
import type { MediaAsset } from "@/types/media";

const srcCache = new Map<string, string>();
const thumbCache = new Map<string, string | null>();

function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

/** Placeholder gradient for web preview / vitest when media files are unavailable. */
export function mockMediaPreviewUrl(asset: MediaAsset): string {
  const label = encodeURIComponent(asset.title.slice(0, 24));
  return `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200">
      <defs>
        <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="#2f8f63"/>
          <stop offset="100%" stop-color="#1a2a3a"/>
        </linearGradient>
      </defs>
      <rect width="100%" height="100%" fill="url(#g)"/>
      <text x="50%" y="50%" fill="#eef2ec" font-size="14" text-anchor="middle" font-family="system-ui">${label}</text>
    </svg>`,
  )}`;
}

export async function resolveMediaAssetUrl(assetId: string): Promise<string> {
  const cached = srcCache.get(assetId);
  if (cached) return cached;

  if (!isTauriRuntime()) {
    const asset = await api.getMediaAsset(assetId);
    const url = mockMediaPreviewUrl(asset);
    srcCache.set(assetId, url);
    return url;
  }

  const src = await api.getMediaAssetSrc(assetId);
  const url = convertFileSrc(src.absolutePath);
  srcCache.set(assetId, url);
  return url;
}

/** Prefer generated JPEG thumb for grid cards; fall back to full asset. */
export async function resolveMediaThumbUrl(asset: MediaAsset): Promise<string> {
  if (thumbCache.has(asset.id)) {
    const cached = thumbCache.get(asset.id);
    if (cached) return cached;
    return resolveMediaAssetUrl(asset.id);
  }

  if (!isTauriRuntime()) {
    const url = mockMediaPreviewUrl(asset);
    thumbCache.set(asset.id, url);
    return url;
  }

  if (!asset.thumbnailFilename) {
    thumbCache.set(asset.id, null);
    return resolveMediaAssetUrl(asset.id);
  }

  try {
    const thumb = await api.getMediaAssetThumbSrc(asset.id);
    if (thumb) {
      const url = convertFileSrc(thumb.absolutePath);
      thumbCache.set(asset.id, url);
      return url;
    }
  } catch {
    // Fall through to full asset.
  }

  thumbCache.set(asset.id, null);
  return resolveMediaAssetUrl(asset.id);
}

export function clearMediaSrcCache(assetId?: string): void {
  if (assetId) {
    srcCache.delete(assetId);
    thumbCache.delete(assetId);
  } else {
    srcCache.clear();
    thumbCache.clear();
  }
}
