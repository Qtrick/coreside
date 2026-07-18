import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/app-store";
import { shouldShowActionLog } from "@/lib/action-log";
import { MessageBubble } from "./MessageBubble";

export function MessageList() {
  const messages = useAppStore((s) => s.messages);
  const sending = useAppStore((s) => s.sending);
  const agentActions = useAppStore((s) => s.agentActions);
  const actionLogMode = useAppStore((s) => s.actionLogMode);
  const streamingText = useAppStore((s) => s.streamingText);
  const messagesLoading = useAppStore((s) => s.messagesLoading);
  const messagesError = useAppStore((s) => s.messagesError);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const chatViewState = useAppStore((s) => s.chatViewState);
  const setChatScrollTop = useAppStore((s) => s.setChatScrollTop);
  const bottomRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  const restoredFor = useRef<string | null>(null);

  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const onScroll = () => {
      const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
      stickToBottom.current = distance < 80;
      if (activeConversationId) {
        setChatScrollTop(activeConversationId, el.scrollTop);
      }
    };
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, [activeConversationId, setChatScrollTop]);

  useEffect(() => {
    const el = listRef.current;
    if (!el || !activeConversationId) return;
    if (restoredFor.current === activeConversationId) return;
    const saved = chatViewState[activeConversationId]?.scrollTop;
    if (typeof saved === "number") {
      el.scrollTop = saved;
      stickToBottom.current = false;
    }
    restoredFor.current = activeConversationId;
  }, [activeConversationId, chatViewState, messagesLoading]);

  useEffect(() => {
    if (stickToBottom.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  }, [messages, sending, agentActions, streamingText]);

  if (messagesLoading) {
    return (
      <div className="message-list" aria-busy="true">
        <div className="empty-state">
          <div className="loading-dots" aria-label="Loading messages">
            <span />
            <span />
            <span />
          </div>
        </div>
      </div>
    );
  }

  if (messagesError) {
    return (
      <div className="message-list">
        <div className="empty-state">
          <h3>Conversation failed to load</h3>
          <p>{messagesError}</p>
        </div>
      </div>
    );
  }

  if (messages.length === 0 && !sending) {
    return (
      <div className="message-list">
        <div className="empty-state">
          <h3>Start a conversation</h3>
          <p>
            Ask Coreside to explain something, or create a personal tool like a
            water tracker or quiz.
          </p>
        </div>
      </div>
    );
  }

  const latestAction = agentActions[agentActions.length - 1] ?? null;
  const visibleActions = agentActions.filter((label) => {
    const lower = label.toLowerCase();
    return !(
      lower.includes("working model")
      || lower.includes("model availability")
      || lower.includes("trying another")
      || lower.includes("unavailable")
      || lower.includes("selecting a")
    );
  });
  const liveEvents = visibleActions.map((label) => ({ label }));
  const showLiveActionLog = shouldShowActionLog(actionLogMode, liveEvents);

  return (
    <div className="message-list" ref={listRef} aria-live="polite">
      {messages.map((message) => (
        <MessageBubble key={message.id} message={message} />
      ))}
      {sending ? (
        <div className="message-bubble assistant agent-live" aria-label="Agent is responding">
          <div className="message-meta">
            <span>Coreside agent</span>
          </div>
          {showLiveActionLog && visibleActions.length > 0 ? (
            <ul className="agent-action-log" aria-label="Action log">
              {visibleActions.map((label, index) => (
                <li
                  key={`${index}-${label}`}
                  className={index === visibleActions.length - 1 ? "current" : ""}
                >
                  {label}
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted agent-action-pending">
              {showLiveActionLog &&
              latestAction &&
              !latestAction.toLowerCase().includes("model") &&
              !latestAction.toLowerCase().includes("unavailable")
                ? latestAction
                : "Working…"}
            </p>
          )}
          {streamingText ? (
            <div className="message-content agent-stream-text">
              <p style={{ margin: 0, whiteSpace: "pre-wrap" }}>
                {streamingText}
                <span className="stream-caret" aria-hidden />
              </p>
            </div>
          ) : (
            <div className="loading-dots" aria-hidden>
              <span />
              <span />
              <span />
            </div>
          )}
        </div>
      ) : null}
      <div ref={bottomRef} />
    </div>
  );
}
