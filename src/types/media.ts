import { z } from "zod";

export const MediaCategorySchema = z.enum([
  "image",
  "video",
  "animated",
  "all",
]);
export type MediaCategoryFilter = z.infer<typeof MediaCategorySchema>;

export const MediaAssetSchema = z.object({
  id: z.string(),
  projectId: z.string().optional().nullable(),
  category: z.string(),
  title: z.string(),
  localFilename: z.string(),
  mimeType: z.string(),
  byteSize: z.number(),
  width: z.number().optional().nullable(),
  height: z.number().optional().nullable(),
  durationMs: z.number().optional().nullable(),
  contentHash: z.string(),
  sourceUrl: z.string().optional().nullable(),
  sourcePageUrl: z.string().optional().nullable(),
  creator: z.string().optional().nullable(),
  license: z.string().optional().nullable(),
  attribution: z.string().optional().nullable(),
  validationStatus: z.string(),
  createdAt: z.string(),
  lastUsedAt: z.string().optional().nullable(),
  thumbnailFilename: z.string().optional().nullable(),
});

export type MediaAsset = z.infer<typeof MediaAssetSchema>;

export const ImportMediaInputSchema = z.object({
  url: z.string(),
  title: z.string().optional().nullable(),
  projectId: z.string().optional().nullable(),
  sourcePageUrl: z.string().optional().nullable(),
  creator: z.string().optional().nullable(),
  license: z.string().optional().nullable(),
  attribution: z.string().optional().nullable(),
  category: z.string().optional().nullable(),
});

export type ImportMediaInput = z.infer<typeof ImportMediaInputSchema>;

export const MediaAssetSrcSchema = z.object({
  absolutePath: z.string(),
  mimeType: z.string(),
});

export type MediaAssetSrc = z.infer<typeof MediaAssetSrcSchema>;

export const MediaAssetUsageSchema = z.object({
  projectId: z.string(),
  projectName: z.string(),
});

export type MediaAssetUsage = z.infer<typeof MediaAssetUsageSchema>;

export function formatByteSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function mediaCategoryLabel(category: string): string {
  switch (category) {
    case "image":
      return "Image";
    case "video":
      return "Video";
    case "animated":
    case "animated-image":
      return "Animated";
    default:
      return category;
  }
}

export function matchesMediaFilter(
  asset: MediaAsset,
  filter: MediaCategoryFilter,
): boolean {
  if (filter === "all") return true;
  if (filter === "animated") {
    return (
      asset.category === "animated" || asset.category === "animated-image"
    );
  }
  return asset.category === filter;
}
