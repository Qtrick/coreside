import { z } from "zod";

export const DeleteProjectModeSchema = z.enum(["keepChats", "deleteChats"]);
export type DeleteProjectMode = z.infer<typeof DeleteProjectModeSchema>;

export const ProjectSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional().nullable(),
  iconKey: z.string().optional().nullable(),
  instructions: z.string().optional().nullable(),
  summary: z.string().optional().nullable(),
  summaryUpdatedAt: z.string().optional().nullable(),
  archived: z.boolean().default(false),
  pinned: z.boolean().default(false),
  wallpaperJson: z.string().optional().nullable(),
  createdAt: z.string(),
  updatedAt: z.string(),
  lastOpenedAt: z.string().optional().nullable(),
});

export type Project = z.infer<typeof ProjectSchema>;

export const CreateProjectInputSchema = z.object({
  name: z.string(),
  description: z.string().optional().nullable(),
  iconKey: z.string().optional().nullable(),
  instructions: z.string().optional().nullable(),
  pinned: z.boolean().optional(),
});

export type CreateProjectInput = z.infer<typeof CreateProjectInputSchema>;

export const UpdateProjectInputSchema = z.object({
  name: z.string().optional(),
  description: z.string().optional().nullable(),
  iconKey: z.string().optional().nullable(),
  instructions: z.string().optional().nullable(),
  pinned: z.boolean().optional(),
  archived: z.boolean().optional(),
});

export type UpdateProjectInput = z.infer<typeof UpdateProjectInputSchema>;

export const ProjectContextHitSchema = z.object({
  messageId: z.string(),
  conversationId: z.string(),
  conversationTitle: z.string(),
  role: z.string(),
  snippet: z.string(),
  rank: z.number(),
});

export type ProjectContextHit = z.infer<typeof ProjectContextHitSchema>;

/** Trusted icon keys validated by the Rust backend. */
export const PROJECT_ICON_KEYS = [
  "folder",
  "book",
  "leaf",
  "chart",
  "code",
  "heart",
  "star",
  "travel",
  "fitness",
  "pen",
] as const;

export type ProjectIconKey = (typeof PROJECT_ICON_KEYS)[number];

export const PROJECT_ICON_LABELS: Record<ProjectIconKey, string> = {
  folder: "Folder",
  book: "Book",
  leaf: "Leaf",
  chart: "Chart",
  code: "Code",
  heart: "Heart",
  star: "Star",
  travel: "Travel",
  fitness: "Fitness",
  pen: "Pen",
};
