import { z } from "zod";

/** Public attachment contract — opaque id + display fields only (no storage keys). */
export const StagedAttachmentSchema = z.object({
  id: z.string(),
  name: z.string(),
  mimeType: z.string(),
  byteSize: z.number(),
});

export type StagedAttachment = z.infer<typeof StagedAttachmentSchema>;
