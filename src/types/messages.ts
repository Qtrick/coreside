import { z } from "zod";

export const MessageRoleSchema = z.enum(["user", "assistant", "system"]);
export type MessageRole = z.infer<typeof MessageRoleSchema>;

export const ChatMessageSchema = z.object({
  id: z.string(),
  conversationId: z.string(),
  role: MessageRoleSchema,
  content: z.string(),
  createdAt: z.string(),
  metadata: z.record(z.unknown()).optional().nullable(),
  status: z.enum(["ok", "error", "pending"]).optional(),
  errorMessage: z.string().optional().nullable(),
});

export type ChatMessage = z.infer<typeof ChatMessageSchema>;

export const ConversationSchema = z.object({
  id: z.string(),
  title: z.string(),
  workspaceId: z.string().optional().nullable(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export type Conversation = z.infer<typeof ConversationSchema>;
