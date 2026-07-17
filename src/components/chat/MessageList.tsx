import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/app-store";
import { MessageBubble } from "./MessageBubble";

export function MessageList() {
  const messages = useAppStore((s) => s.messages);
  const sending = useAppStore((s) => s.sending);
  const messagesLoading = useAppStore((s) => s.messagesLoading);
  const messagesError = useAppStore((s) => s.messagesError);
  const bottomRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const onScroll = () => {
      const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
      stickToBottom.current = distance < 80;
    };
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    if (stickToBottom.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  }, [messages, sending]);

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

  if (messages.length === 0) {
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

  return (
    <div className="message-list" ref={listRef} aria-live="polite">
      {messages.map((message) => (
        <MessageBubble key={message.id} message={message} />
      ))}
      {sending ? (
        <div className="message-bubble assistant" aria-label="Agent is thinking">
          <div className="message-meta">
            <span>Coreside agent</span>
          </div>
          <div className="loading-dots" aria-hidden>
            <span />
            <span />
            <span />
          </div>
        </div>
      ) : null}
      <div ref={bottomRef} />
    </div>
  );
}
