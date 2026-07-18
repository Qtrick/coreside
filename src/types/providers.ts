import { z } from "zod";

export const ProviderConnectionSchema = z.object({
  id: z.string(),
  provider: z.string(),
  label: z.string(),
  baseUrl: z.string().nullable().optional(),
  modelDefault: z.string().nullable().optional(),
  isActive: z.boolean(),
  hasKey: z.boolean(),
  lastStatus: z.string().nullable().optional(),
  lastTestedAt: z.string().nullable().optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type ProviderConnection = z.infer<typeof ProviderConnectionSchema>;

export const ProviderHintSchema = z.object({
  id: z.string(),
  label: z.string(),
  keyPlaceholder: z.string(),
  defaultModel: z.string(),
  docsUrl: z.string().nullable().optional(),
  supportsBaseUrl: z.boolean(),
});

export type ProviderHint = z.infer<typeof ProviderHintSchema>;

export const UpsertProviderConnectionInputSchema = z.object({
  id: z.string().optional().nullable(),
  provider: z.string(),
  label: z.string(),
  apiKey: z.string().optional().nullable(),
  baseUrl: z.string().optional().nullable(),
  modelDefault: z.string().optional().nullable(),
  setActive: z.boolean().optional().nullable(),
});

export type UpsertProviderConnectionInput = z.infer<
  typeof UpsertProviderConnectionInputSchema
>;

export type ProviderSetupMode = "add" | "edit" | "replace-key";
