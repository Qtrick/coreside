import { z } from "zod";

export const StagedAttachmentSchema = z.object({
  id: z.string(),
  name: z.string(),
  mimeType: z.string(),
  byteSize: z.number(),
  localFilename: z.string(),
});

export type StagedAttachment = z.infer<typeof StagedAttachmentSchema>;
