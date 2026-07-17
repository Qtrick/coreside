import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { AlertCircle, RotateCcw } from "lucide-react";
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
  const isUser = message.role === "user";
  const isError = message.status === "error";

  return (
    <article
      className={`message-bubble ${isUser ? "user" : "assistant"}${isError ? " error" : ""}`}
      aria-label={isUser ? "Your message" : "Agent message"}
    >
      <div className="message-meta">
        <span>{isUser ? "You" : "Coreside agent"}</span>
        <time dateTime={message.createdAt}>
          {new Date(message.createdAt).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </time>
      </div>
      <div className="message-content">
        {isUser ? (
          <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>{message.content}</p>
        ) : (
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            urlTransform={safeMarkdownUrl}
            skipHtml
          >
            {message.content}
          </ReactMarkdown>
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
