import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  AlertCircle,
  Check,
  Copy,
  Pencil,
  RotateCcw,
} from "lucide-react";
import { ActionLog } from "@/components/action-log/ActionLog";
import {
  ChangeProposalCard,
  kernelProposalFromMetadata,
} from "@/components/chat/ChangeProposalCard";
import { InlineSurfacesForMessage } from "@/components/chat/InlineSurface";
import { MessageAttachments } from "@/components/chat/MessageAttachments";
import {
  SearchResults,
  searchDataFromMetadata,
} from "@/components/search/SearchResults";
import type { ChatMessage } from "@/types/messages";
import { useAppStore } from "@/stores/app-store";

function safeMarkdownUrl(url: string): string {
  const trimmed = url.trim();
  if (
    trimmed.startsWith("https:") ||
    trimmed.startsWith("http:") ||
    trimmed.startsWith("mailto:") ||
    trimmed.startsWith("#")
  ) {
    return trimmed;
  }
  return "";
}

export function MessageBubble({ message }: { message: ChatMessage }) {
  const retryLastFailed = useAppStore((s) => s.retryLastFailed);
  const editAndResendMessage = useAppStore((s) => s.editAndResendMessage);
  const sending = useAppStore((s) => s.sending);
  const isUser = message.role === "user";
  const isError = message.status === "error";
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(message.content);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!editing) setDraft(message.content);
  }, [message.content, editing]);

  useEffect(() => {
    if (!copied) return;
    const t = window.setTimeout(() => setCopied(false), 1400);
    return () => window.clearTimeout(t);
  }, [copied]);

  const copyMessage = async () => {
    try {
      await navigator.clipboard.writeText(message.content);
      setCopied(true);
    } catch {
      // Fallback for restricted clipboard contexts.
      const ta = document.createElement("textarea");
      ta.value = message.content;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.left = "-9999px";
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      setCopied(true);
    }
  };

  const saveEdit = async () => {
    const next = draft.trim();
    if (!next || sending) return;
    setEditing(false);
    await editAndResendMessage(message.id, next);
  };

  const responseModel = (() => {
    if (isUser) return null;
    const meta = message.metadata;
    if (!meta || typeof meta !== "object") return null;
    const diagnostics = (meta as Record<string, unknown>).diagnostics;
    if (!diagnostics || typeof diagnostics !== "object") return null;
    const model = (diagnostics as Record<string, unknown>).model;
    return typeof model === "string" && model.trim() ? model.trim() : null;
  })();

  const searchData = !isUser
    ? searchDataFromMetadata(
        message.metadata && typeof message.metadata === "object"
          ? (message.metadata as Record<string, unknown>)
          : null,
      )
    : { citations: [], searchResults: null };

  const kernelProposal = !isUser
    ? kernelProposalFromMetadata(
        message.metadata && typeof message.metadata === "object"
          ? (message.metadata as Record<string, unknown>)
          : null,
      )
    : null;

  return (
    <article
      className={`message-bubble ${isUser ? "user" : "assistant"}${isError ? " error" : ""}`}
      aria-label={isUser ? "Your message" : "Agent message"}
    >
      <div className="message-meta">
        <span>{isUser ? "You" : "Coreside agent"}</span>
        <div className="message-meta-right">
          <time dateTime={message.createdAt}>
            {new Date(message.createdAt).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </time>
          {!editing ? (
            <div className="message-actions" role="group" aria-label="Message actions">
              <button
                type="button"
                className="message-action-btn"
                onClick={() => void copyMessage()}
                aria-label={copied ? "Copied" : "Copy message"}
                title={copied ? "Copied" : "Copy"}
              >
                {copied ? <Check size={14} aria-hidden /> : <Copy size={14} aria-hidden />}
              </button>
              {isUser && !isError ? (
                <button
                  type="button"
                  className="message-action-btn"
                  onClick={() => {
                    setDraft(message.content);
                    setEditing(true);
                  }}
                  disabled={sending}
                  aria-label="Edit message"
                  title="Edit"
                >
                  <Pencil size={14} aria-hidden />
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>

      <div className="message-content">
        {editing ? (
          <div className="message-edit">
            <textarea
              className="message-edit-input"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              rows={Math.min(10, Math.max(3, draft.split("\n").length + 1))}
              aria-label="Edit message"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  setEditing(false);
                  setDraft(message.content);
                }
                if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                  e.preventDefault();
                  void saveEdit();
                }
              }}
            />
            <div className="message-edit-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => {
                  setEditing(false);
                  setDraft(message.content);
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={!draft.trim() || sending}
                onClick={() => void saveEdit()}
              >
                Save & send
              </button>
            </div>
          </div>
        ) : isUser ? (
          <>
            <MessageAttachments
              metadata={
                (message.metadata as Record<string, unknown> | null | undefined) ??
                null
              }
            />
            <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>{message.content}</p>
          </>
        ) : (
          <>
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              urlTransform={safeMarkdownUrl}
              skipHtml
            >
              {message.content}
            </ReactMarkdown>
            {responseModel ? (
              <p className="message-model-attribution">{responseModel}</p>
            ) : null}
            <SearchResults
              citations={searchData.citations}
              searchResults={searchData.searchResults}
            />
            <InlineSurfacesForMessage
              conversationId={message.conversationId}
              messageId={message.id}
            />
            {kernelProposal ? (
              <ChangeProposalCard
                proposalId={kernelProposal.proposalId}
                summary={kernelProposal.summary}
                impactSummary={kernelProposal.impactSummary}
                risk={kernelProposal.risk}
                operations={kernelProposal.operations}
                messageId={message.id}
                conversationId={message.conversationId}
                status={kernelProposal.status}
              />
            ) : null}
            <ActionLog message={message} />
          </>
        )}
      </div>

      {isError ? (
        <div className="preview-actions">
          <span
            className="muted"
            style={{ display: "inline-flex", gap: 6, alignItems: "center" }}
          >
            <AlertCircle size={14} aria-hidden />
            {message.errorMessage ?? "Something went wrong"}
          </span>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void retryLastFailed()}
          >
            <RotateCcw size={14} aria-hidden />
            Retry
          </button>
        </div>
      ) : null}
    </article>
  );
}
