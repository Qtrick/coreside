import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { KeyRound, SendHorizontal, Square } from "lucide-react";
import { useAppStore } from "@/stores/app-store";
import { ToolChangePreview } from "./ToolChangePreview";

export function Composer() {
  const sendMessage = useAppStore((s) => s.sendMessage);
  const cancelRequest = useAppStore((s) => s.cancelRequest);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const sending = useAppStore((s) => s.sending);
  const aiStatus = useAppStore((s) => s.aiStatus);
  const sendError = useAppStore((s) => s.sendError);
  const [value, setValue] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const needsSetup =
    aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured";
  const disabled = sending || needsSetup;

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const submit = async () => {
    const content = value.trim();
    if (!content || disabled) return;
    setValue("");
    await sendMessage(content);
    textareaRef.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
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
                "Add AI_API_KEY (or GEMINI_API_KEY) to the project .env file, then restart npm run dev."}
            </p>
          </div>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => setSettingsOpen(true)}
          >
            Open settings
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
      <div className="composer-box">
        <label className="visually-hidden" htmlFor="composer-input">
          Message
        </label>
        <textarea
          id="composer-input"
          ref={textareaRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={
            needsSetup ? "Configure an API key to start chatting…" : "Message Coreside…"
          }
          disabled={sending || needsSetup}
          aria-label="Message composer"
        />
        {sending ? (
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void cancelRequest()}
            aria-label="Cancel request"
          >
            <Square size={16} aria-hidden />
            Stop
          </button>
        ) : (
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void submit()}
            disabled={disabled || !value.trim()}
            aria-label="Send message"
          >
            <SendHorizontal size={16} aria-hidden />
            Send
          </button>
        )}
      </div>
      <p className="composer-hint">Enter to send · Shift+Enter for a new line</p>
      <style>{`
        .visually-hidden {
          position: absolute;
          width: 1px;
          height: 1px;
          padding: 0;
          margin: -1px;
          overflow: hidden;
          clip: rect(0, 0, 0, 0);
          white-space: nowrap;
          border: 0;
        }
      `}</style>
    </div>
  );
}
