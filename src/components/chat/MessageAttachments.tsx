import { useEffect, useState } from "react";
import { FileText, Paperclip } from "lucide-react";
import { api } from "@/lib/tauri";
import { StagedAttachmentSchema, type StagedAttachment } from "@/types/attachments";

function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

function parseAttachments(raw: unknown): StagedAttachment[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) => StagedAttachmentSchema.safeParse(item))
    .filter((r) => r.success)
    .map((r) => r.data);
}

function AttachmentCard({
  attachment,
  conversationId,
}: {
  attachment: StagedAttachment;
  conversationId: string;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const isImage = attachment.mimeType.startsWith("image/");

  useEffect(() => {
    if (!isImage || !conversationId) return;
    let cancelled = false;
    void (async () => {
      try {
        if (!isTauriRuntime()) return;
        const srcResult = await api.getChatAttachmentSrc(
          attachment.id,
          conversationId,
        );
        if (!cancelled) setSrc(srcResult.url);
      } catch {
        // Preview is best-effort.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [attachment.id, conversationId, isImage]);

  return (
    <li className="message-attachment-chip">
      {src ? (
        <img src={src} alt={attachment.name} className="message-attachment-thumb" />
      ) : (
        <span className="message-attachment-icon" aria-hidden>
          {isImage ? <Paperclip size={14} /> : <FileText size={14} />}
        </span>
      )}
      <span className="message-attachment-name">{attachment.name}</span>
    </li>
  );
}

export function MessageAttachments({
  metadata,
  conversationId,
}: {
  metadata: Record<string, unknown> | null | undefined;
  conversationId: string;
}) {
  const attachments = parseAttachments(metadata?.attachments);
  if (attachments.length === 0) return null;

  return (
    <ul className="message-attachments" aria-label="Attachments">
      {attachments.map((attachment) => (
        <AttachmentCard
          key={attachment.id}
          attachment={attachment}
          conversationId={conversationId}
        />
      ))}
    </ul>
  );
}
