import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
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

function AttachmentCard({ attachment }: { attachment: StagedAttachment }) {
  const [src, setSrc] = useState<string | null>(null);
  const isImage = attachment.mimeType.startsWith("image/");

  useEffect(() => {
    if (!isImage) return;
    let cancelled = false;
    void (async () => {
      try {
        if (!isTauriRuntime()) return;
        const path = await api.getChatAttachmentSrc(attachment.localFilename);
        if (!cancelled) setSrc(convertFileSrc(path));
      } catch {
        // Preview is best-effort.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [attachment.localFilename, isImage]);

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
}: {
  metadata: Record<string, unknown> | null | undefined;
}) {
  const attachments = parseAttachments(metadata?.attachments);
  if (attachments.length === 0) return null;

  return (
    <ul className="message-attachments" aria-label="Attachments">
      {attachments.map((attachment) => (
        <AttachmentCard key={attachment.id} attachment={attachment} />
      ))}
    </ul>
  );
}
