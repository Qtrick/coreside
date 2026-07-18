import { z } from "zod";

export const SafeSearchLevelSchema = z.enum(["strict", "standard", "off"]);
export type SafeSearchLevel = z.infer<typeof SafeSearchLevelSchema>;

export const ResourceProfileSchema = z.enum(["eco", "balanced", "performance"]);
export type ResourceProfile = z.infer<typeof ResourceProfileSchema>;

export const SearchConnectionSchema = z.object({
  provider: z.string(),
  hasKey: z.boolean(),
  source: z.string(),
  engineReady: z.boolean(),
  engineReason: z.string().optional().nullable(),
  exaConfigured: z.boolean().optional().default(false),
  exaSource: z.string().optional().default("none"),
});
export type SearchConnection = z.infer<typeof SearchConnectionSchema>;

export const SearchUsageProfileSchema = z.enum(["saver", "balanced", "thorough"]);
export type SearchUsageProfile = z.infer<typeof SearchUsageProfileSchema>;

export const ExaConnectionSchema = z.object({
  configured: z.boolean(),
  source: z.string(),
  account: z.string(),
});
export type ExaConnection = z.infer<typeof ExaConnectionSchema>;

export const ExaUsageSummarySchema = z.object({
  monthKey: z.string(),
  requestCount: z.number(),
  resultCount: z.number().optional().default(0),
  actualCostTotal: z.number(),
  estimatedCostTotal: z.number().optional().default(0),
  cacheHits: z.number().optional().default(0),
});
export type ExaUsageSummary = z.infer<typeof ExaUsageSummarySchema>;

export const ExaBudgetStatusSchema = z.object({
  monthKey: z.string(),
  spentUsd: z.number(),
  budgetUsd: z.number().nullable().optional(),
  remainingUsd: z.number().nullable().optional(),
  percentUsed: z.number().nullable().optional(),
  threshold: z.string(), // ok | soft | critical | hard
  note: z.string().optional().default(""),
});
export type ExaBudgetStatus = z.infer<typeof ExaBudgetStatusSchema>;

export const SearchProfileViewSchema = z.object({
  profile: z.string(),
  searchType: z.string(),
  numResults: z.number(),
  maxCrawlPages: z.number(),
  maxRefinements: z.number(),
});
export type SearchProfileView = z.infer<typeof SearchProfileViewSchema>;

export const InstallationStateSchema = z.enum([
  "ready",
  "needsSetup",
  "error",
]);
export type InstallationState = z.infer<typeof InstallationStateSchema>;

export const CrawlerStatusSchema = z.object({
  installation: InstallationStateSchema,
  running: z.boolean(),
  pythonPath: z.string().optional().nullable(),
  reason: z.string().optional().nullable(),
  resourceProfile: z.string(),
  sidecarVersion: z.string().optional().nullable(),
  crawl4aiVersion: z.string().optional().nullable(),
});
export type CrawlerStatus = z.infer<typeof CrawlerStatusSchema>;

export const CrawlerInstallationSchema = z.object({
  state: InstallationStateSchema,
  pythonPath: z.string().optional().nullable(),
  serviceRoot: z.string(),
  crawl4aiImportOk: z.boolean(),
  reason: z.string().optional().nullable(),
});
export type CrawlerInstallation = z.infer<typeof CrawlerInstallationSchema>;

export const CacheStatsSchema = z.object({
  root: z.string().optional().nullable(),
  sizeBytes: z.number(),
  quotaBytes: z.number(),
  usageRatio: z.number(),
  overQuota: z.boolean(),
});
export type CacheStats = z.infer<typeof CacheStatsSchema>;

export const SourceCitationSchema = z.object({
  id: z.string(),
  title: z.string(),
  url: z.string(),
  displayDomain: z.string().optional().nullable(),
  snippet: z.string().optional().nullable(),
});
export type SourceCitation = z.infer<typeof SourceCitationSchema>;

export const WebSearchResultSchema = z.object({
  id: z.string(),
  title: z.string(),
  url: z.string(),
  displayDomain: z.string().optional().nullable(),
  snippet: z.string().optional().nullable(),
  age: z.string().optional().nullable(),
  rank: z.number(),
});
export type WebSearchResult = z.infer<typeof WebSearchResultSchema>;

export const ImageSearchResultSchema = z.object({
  id: z.string(),
  title: z.string(),
  pageUrl: z.string(),
  imageUrl: z.string(),
  thumbnailUrl: z.string().optional().nullable(),
  width: z.number().optional().nullable(),
  height: z.number().optional().nullable(),
  source: z.string().optional().nullable(),
  rank: z.number(),
});
export type ImageSearchResult = z.infer<typeof ImageSearchResultSchema>;

export const VideoSearchResultSchema = z.object({
  id: z.string(),
  title: z.string(),
  url: z.string(),
  thumbnailUrl: z.string().optional().nullable(),
  duration: z.string().optional().nullable(),
  creator: z.string().optional().nullable(),
  age: z.string().optional().nullable(),
  rank: z.number(),
});
export type VideoSearchResult = z.infer<typeof VideoSearchResultSchema>;

export const WebSearchResponseSchema = z.object({
  query: z.string(),
  provider: z.string(),
  results: z.array(WebSearchResultSchema),
  sessionId: z.string().optional().nullable(),
  notice: z.string().optional().nullable(),
});
export type WebSearchResponse = z.infer<typeof WebSearchResponseSchema>;

export const ImageSearchResponseSchema = z.object({
  query: z.string(),
  provider: z.string(),
  results: z.array(ImageSearchResultSchema),
  sessionId: z.string().optional().nullable(),
  notice: z.string().optional().nullable(),
});
export type ImageSearchResponse = z.infer<typeof ImageSearchResponseSchema>;

export const VideoSearchResponseSchema = z.object({
  query: z.string(),
  provider: z.string(),
  results: z.array(VideoSearchResultSchema),
  sessionId: z.string().optional().nullable(),
  notice: z.string().optional().nullable(),
});
export type VideoSearchResponse = z.infer<typeof VideoSearchResponseSchema>;

export const MessageSearchResultsSchema = z.object({
  web: WebSearchResponseSchema.optional(),
  images: ImageSearchResponseSchema.optional(),
  videos: VideoSearchResponseSchema.optional(),
  pendingMediaImports: z
    .array(
      z.object({
        url: z.string(),
        title: z.string().optional().nullable(),
        category: z.string().optional().nullable(),
        sourcePageUrl: z.string().optional().nullable(),
        creator: z.string().optional().nullable(),
        license: z.string().optional().nullable(),
      }),
    )
    .optional(),
});
export type MessageSearchResults = z.infer<typeof MessageSearchResultsSchema>;

export const SearchSessionSummarySchema = z.object({
  id: z.string(),
  conversationId: z.string().optional().nullable(),
  projectId: z.string().optional().nullable(),
  searchType: z.string(),
  query: z.string(),
  provider: z.string(),
  createdAt: z.string(),
  resultCount: z.number(),
});
export type SearchSessionSummary = z.infer<typeof SearchSessionSummarySchema>;

export const StoredSearchResultSchema = z.object({
  id: z.string(),
  sessionId: z.string(),
  resultType: z.string(),
  title: z.string().optional().nullable(),
  url: z.string().optional().nullable(),
  displayDomain: z.string().optional().nullable(),
  snippet: z.string().optional().nullable(),
  thumbnailUrl: z.string().optional().nullable(),
  metadataJson: z.string().optional().nullable(),
  createdAt: z.string(),
});
export type StoredSearchResult = z.infer<typeof StoredSearchResultSchema>;

export const SearchSessionDetailSchema = z.object({
  session: SearchSessionSummarySchema,
  results: z.array(StoredSearchResultSchema),
});
export type SearchSessionDetail = z.infer<typeof SearchSessionDetailSchema>;
