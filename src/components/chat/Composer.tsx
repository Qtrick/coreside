import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
} from "react";
import {
  KeyRound,
  Paperclip,
  SendHorizontal,
  Square,
  X,
} from "lucide-react";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri";
import {
  applyMentionInsertion,
  filterToolsByMentionQuery,
  getActiveMentionQuery,
  mergeResolvedMentions,
  resolveMentionsInText,
  type ToolMention,
} from "@/lib/mentions";
import type { StagedAttachment } from "@/types/attachments";
import { MentionMenu } from "./MentionMenu";
import { ModelPicker } from "./ModelPicker";
import { ToolChangePreview } from "./ToolChangePreview";

const MAX_ATTACHMENTS = 5;
const MAX_FILE_BYTES = 12 * 1024 * 1024;
const ACCEPT =
  "image/*,text/*,.md,.txt,.json,.csv,.pdf,application/pdf,application/json";

type PendingAttachment = {
  localId: string;
  file: File;
  previewUrl?: string;
};

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Failed to read file"));
    reader.onload = () => {
      const result = String(reader.result ?? "");
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.readAsDataURL(file);
  });
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function Composer() {
  const sendMessage = useAppStore((s) => s.sendMessage);
  const cancelRequest = useAppStore((s) => s.cancelRequest);
  const openProviderSetup = useAppStore((s) => s.openProviderSetup);
  const sending = useAppStore((s) => s.sending);
  const aiStatus = useAppStore((s) => s.aiStatus);
  const sendError = useAppStore((s) => s.sendError);
  const tools = useAppStore((s) => s.tools);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const setChatDraft = useAppStore((s) => s.setChatDraft);
  const [value, setValue] = useState("");
  const [caret, setCaret] = useState(0);
  const [activeIndex, setActiveIndex] = useState(0);
  const [pendingMentions, setPendingMentions] = useState<ToolMention[]>([]);
  const [mentionSuppressed, setMentionSuppressed] = useState(false);
  const [pendingFiles, setPendingFiles] = useState<PendingAttachment[]>([]);
  const [attachError, setAttachError] = useState<string | null>(null);
  const [staging, setStaging] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const fileInputId = useId();

  const needsSetup =
    aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured";
  const busy = sending || staging;
  const canSend =
    !busy && (Boolean(value.trim()) || pendingFiles.length > 0 || needsSetup);

  const mentionQuery = useMemo(
    () => getActiveMentionQuery(value, caret),
    [value, caret],
  );

  const mentionMatches = useMemo(() => {
    if (!mentionQuery) return [];
    return filterToolsByMentionQuery(tools, mentionQuery.query).slice(0, 8);
  }, [mentionQuery, tools]);

  const mentionOpen =
    Boolean(mentionQuery) && !needsSetup && !mentionSuppressed;

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 140)}px`;
  }, [value]);

  useEffect(() => {
    const draft = activeConversationId
      ? useAppStore.getState().chatViewState[activeConversationId]?.draft ?? ""
      : "";
    setValue(draft);
    setPendingMentions([]);
    setMentionSuppressed(false);
    setPendingFiles((prev) => {
      for (const item of prev) {
        if (item.previewUrl) URL.revokeObjectURL(item.previewUrl);
      }
      return [];
    });
    setAttachError(null);
  }, [activeConversationId]);

  useEffect(() => {
    setActiveIndex(0);
    setMentionSuppressed(false);
  }, [mentionQuery?.start, mentionQuery?.query]);

  useEffect(() => {
    return () => {
      for (const item of pendingFiles) {
        if (item.previewUrl) URL.revokeObjectURL(item.previewUrl);
      }
    };
  }, [pendingFiles]);

  const syncCaret = () => {
    const el = textareaRef.current;
    if (!el) return;
    setCaret(el.selectionStart ?? 0);
  };

  const insertMention = (tool: { id: string; name: string }) => {
    if (!mentionQuery) return;
    const result = applyMentionInsertion(value, mentionQuery, tool);
    setValue(result.text);
    setPendingMentions((prev) => {
      const next = prev.filter(
        (m) => !(m.start >= mentionQuery.start && m.start < mentionQuery.end),
      );
      return [...next, result.mention];
    });
    requestAnimationFrame(() => {
      const el = textareaRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(result.caret, result.caret);
      setCaret(result.caret);
    });
  };

  const addFiles = (list: FileList | null) => {
    if (!list || list.length === 0) return;
    setAttachError(null);
    const next: PendingAttachment[] = [];
    for (const file of Array.from(list)) {
      if (pendingFiles.length + next.length >= MAX_ATTACHMENTS) {
        setAttachError(`You can attach up to ${MAX_ATTACHMENTS} files.`);
        break;
      }
      if (file.size > MAX_FILE_BYTES) {
        setAttachError(`“${file.name}” exceeds the 12 MB limit.`);
        continue;
      }
      next.push({
        localId: crypto.randomUUID(),
        file,
        previewUrl: file.type.startsWith("image/")
          ? URL.createObjectURL(file)
          : undefined,
      });
    }
    if (next.length > 0) {
      setPendingFiles((prev) => [...prev, ...next]);
    }
    if (fileInputRef.current) fileInputRef.current.value = "";
  };

  const removeFile = (localId: string) => {
    setPendingFiles((prev) => {
      const target = prev.find((p) => p.localId === localId);
      if (target?.previewUrl) URL.revokeObjectURL(target.previewUrl);
      return prev.filter((p) => p.localId !== localId);
    });
  };

  const onFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    addFiles(event.target.files);
  };

  const submit = async () => {
    if (needsSetup) {
      openProviderSetup();
      return;
    }
    const content = value.trim();
    if ((!content && pendingFiles.length === 0) || busy) return;

    setStaging(true);
    setAttachError(null);
    const staged: StagedAttachment[] = [];
    try {
      for (const item of pendingFiles) {
        const dataBase64 = await fileToBase64(item.file);
        const saved = await api.stageChatAttachment({
          name: item.file.name,
          mimeType: item.file.type || "application/octet-stream",
          dataBase64,
        });
        staged.push(saved);
      }
    } catch (err) {
      setAttachError(
        err instanceof Error ? err.message : "Failed to attach files",
      );
      setStaging(false);
      return;
    }

    const resolved = resolveMentionsInText(
      content,
      tools.map((t) => ({ id: t.id, name: t.name })),
    );
    const structured = mergeResolvedMentions(resolved, pendingMentions);
    setValue("");
    if (activeConversationId) {
      setChatDraft(activeConversationId, "");
    }
    setPendingMentions([]);
    setMentionSuppressed(false);
    setPendingFiles((prev) => {
      for (const item of prev) {
        if (item.previewUrl) URL.revokeObjectURL(item.previewUrl);
      }
      return [];
    });
    setStaging(false);
    await sendMessage(content || "Shared attachments", structured, staged);
    textareaRef.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (mentionOpen && mentionMatches.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActiveIndex((i) => (i + 1) % mentionMatches.length);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setActiveIndex(
          (i) => (i - 1 + mentionMatches.length) % mentionMatches.length,
        );
        return;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        insertMention(mentionMatches[activeIndex] ?? mentionMatches[0]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setMentionSuppressed(true);
        return;
      }
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void submit();
    }
  };

  return (
    <div className="composer">
      <ToolChangePreview />
      {needsSetup ? (
        <div className="setup-banner" role="status">
          <div className="setup-banner-icon" aria-hidden>
            <KeyRound size={18} />
          </div>
          <div className="setup-banner-body">
            <strong>Connect an AI provider</strong>
            <p>
              {aiStatus?.message ??
                "Use your own API key to power Coreside. Keys stay on this device."}
            </p>
          </div>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => openProviderSetup()}
          >
            Connect provider
          </button>
        </div>
      ) : aiStatus && aiStatus.status !== "ready" ? (
        <p className="composer-hint" role="status">
          {aiStatus.message ?? "AI provider status: " + aiStatus.status}
        </p>
      ) : null}
      {sendError ? (
        <p className="composer-hint" role="alert">
          {sendError}
        </p>
      ) : null}
      {attachError ? (
        <p className="composer-hint" role="alert">
          {attachError}
        </p>
      ) : null}

      <div className="composer-box composer-box-relative">
        {mentionOpen ? (
          <MentionMenu
            tools={mentionMatches}
            query={mentionQuery?.query ?? ""}
            activeIndex={activeIndex}
            onHover={setActiveIndex}
            onSelect={insertMention}
          />
        ) : null}

        {pendingFiles.length > 0 ? (
          <ul className="composer-attachments" aria-label="Attached files">
            {pendingFiles.map((item) => (
              <li key={item.localId} className="composer-attachment-chip">
                {item.previewUrl ? (
                  <img
                    src={item.previewUrl}
                    alt=""
                    className="composer-attachment-thumb"
                  />
                ) : (
                  <span className="composer-attachment-icon" aria-hidden>
                    <Paperclip size={12} />
                  </span>
                )}
                <span className="composer-attachment-meta">
                  <span className="composer-attachment-name">{item.file.name}</span>
                  <span className="muted">{formatSize(item.file.size)}</span>
                </span>
                <button
                  type="button"
                  className="icon-btn composer-attachment-remove"
                  onClick={() => removeFile(item.localId)}
                  aria-label={`Remove ${item.file.name}`}
                  disabled={busy}
                >
                  <X size={14} />
                </button>
              </li>
            ))}
          </ul>
        ) : null}

        <label className="visually-hidden" htmlFor="composer-input">
          Message
        </label>
        <textarea
          id="composer-input"
          ref={textareaRef}
          rows={1}
          value={value}
          onChange={(e) => {
            const next = e.target.value;
            setValue(next);
            if (activeConversationId) {
              setChatDraft(activeConversationId, next);
            }
            setCaret(e.target.selectionStart ?? next.length);
          }}
          onClick={syncCaret}
          onKeyUp={syncCaret}
          onSelect={syncCaret}
          onKeyDown={onKeyDown}
          onPaste={(e) => {
            const files = e.clipboardData?.files;
            if (files && files.length > 0) {
              e.preventDefault();
              addFiles(files);
            }
          }}
          placeholder={
            needsSetup
              ? "Connect a provider to start chatting…"
              : "Message Coreside… Use @ to mention a tool"
          }
          disabled={sending}
          aria-label="Message composer"
          aria-autocomplete="list"
          aria-expanded={mentionOpen}
        />

        <div className="composer-footer">
          <div className="composer-footer-left">
            <ModelPicker />
          </div>
          <div className="composer-footer-right">
            <input
              id={fileInputId}
              ref={fileInputRef}
              type="file"
              className="visually-hidden"
              accept={ACCEPT}
              multiple
              onChange={onFileChange}
              disabled={busy || needsSetup}
            />
            <button
              type="button"
              className="composer-icon-btn"
              onClick={() => fileInputRef.current?.click()}
              disabled={busy || needsSetup}
              aria-label="Attach files"
              title="Attach files"
            >
              <Paperclip size={18} aria-hidden />
            </button>
            {sending ? (
              <button
                type="button"
                className="composer-send-btn composer-stop-btn"
                onClick={() => void cancelRequest()}
                aria-label="Cancel request"
              >
                <Square size={14} aria-hidden />
              </button>
            ) : (
              <button
                type="button"
                className="composer-send-btn"
                onClick={() => void submit()}
                disabled={!canSend}
                aria-label={needsSetup ? "Connect provider" : "Send message"}
                title={needsSetup ? "Connect provider" : "Send"}
              >
                {needsSetup ? (
                  <KeyRound size={16} aria-hidden />
                ) : (
                  <SendHorizontal size={16} aria-hidden />
                )}
              </button>
            )}
          </div>
        </div>
      </div>

      <p className="composer-hint">
        Enter to send · Shift+Enter for a new line · @ for tools · paperclip to
        attach
      </p>
    </div>
  );
}
