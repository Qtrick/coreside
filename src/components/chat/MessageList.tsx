import { useEffect, useRef, useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { shouldShowActionLog } from "@/lib/action-log";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { MessageBubble } from "./MessageBubble";
import {
  computeLiveApplicationGenerationIndex,
  humanizeAgentActionLabel,
  LIVE_APPLICATION_GENERATION_STEPS,
} from "@/lib/application-proposal-status";

export function MessageList() {
  const messages = useAppStore((s) => s.messages);
  const sending = useAppStore((s) => s.sending);
  const sendingConversationId = useAppStore((s) => s.sendingConversationId);
  const agentActions = useAppStore((s) => s.agentActions);
  const actionLogMode = useAppStore((s) => s.actionLogMode);
  const streamingText = useAppStore((s) => s.streamingText);
  const messagesLoading = useAppStore((s) => s.messagesLoading);
  const messagesError = useAppStore((s) => s.messagesError);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);
  const activeTurnId = useAppStore((s) =>
    s.activeConversationId
      ? (s.activeTurnIdByConversation[s.activeConversationId] ?? null)
      : null,
  );
  const streamLastSequence = useAppStore((s) => {
    if (!s.activeConversationId) return null;
    const turnId = s.activeTurnIdByConversation[s.activeConversationId];
    if (!turnId) return null;
    const seq = s.turnsById[turnId]?.lastSequence;
    return typeof seq === "number" ? seq : null;
  });
  const chatViewState = useAppStore((s) => s.chatViewState);
  const setChatScrollTop = useAppStore((s) => s.setChatScrollTop);
  const bottomRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  const restoredFor = useRef<string | null>(null);
  const [showNewUpdates, setShowNewUpdates] = useState(false);
  const prevMessageCount = useRef(messages.length);
  const liveHere =
    sending &&
    Boolean(activeConversationId) &&
    sendingConversationId === activeConversationId;

  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const onScroll = () => {
      const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
      stickToBottom.current = distance < 80;
      if (stickToBottom.current) {
        setShowNewUpdates(false);
      }
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
    if (messages.length > prevMessageCount.current && !stickToBottom.current) {
      setShowNewUpdates(true);
    } else if (stickToBottom.current) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
    }
    prevMessageCount.current = messages.length;
  }, [messages, liveHere, agentActions, streamingText]);

  const jumpToLatest = () => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
    setShowNewUpdates(false);
    stickToBottom.current = true;
  };

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
          <h3>{EMPTY_STATES.chatLoadError.title}</h3>
          <p>{messagesError}</p>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => openHelpAndLearning(navigateToSettings)}
          >
            {EMPTY_STATES.chatLoadError.primaryCta}
          </button>
        </div>
      </div>
    );
  }

const STARTER_PROMPTS = [
  {
    title: "Task Manager",
    desc: "Persistent task board with priorities, statuses, and filtering",
    prompt:
      "Build me a task manager with persistent tasks, priority, status, create, edit, delete, search, and filtering.",
  },
  {
    title: "Habit Tracker",
    desc: "Daily habit tracking with streaks, metrics, and progress logs",
    prompt:
      "Build a habit tracker to log daily habits, completion streaks, notes, and progress overview.",
  },
  {
    title: "Research Organizer",
    desc: "Structured research topics with key findings, tags, and summary",
    prompt:
      "Build a research organizer with search, categorized findings, tags, and summary view.",
  },
  {
    title: "Study Dashboard",
    desc: "Subject cards, flashcard statistics, deadlines, and study timer",
    prompt:
      "Build a study dashboard with subject cards, flashcard review stats, upcoming deadlines, and study timer.",
  },
  {
    title: "Quiz App",
    desc: "Interactive question banks, multi-choice scoring, and review mode",
    prompt:
      "Build a quiz app with question banks, interactive choices, score calculation, and review mode.",
  },
  {
    title: "Finance Tracker",
    desc: "Income and expense records with category breakdown and budget summary",
    prompt:
      "Build a finance tracker for income, expenses, category breakdown, and monthly budget summary.",
  },
  {
    title: "Media Organizer",
    desc: "Catalog books, games, movies, personal ratings, and wishlist",
    prompt:
      "Build a media organizer to catalog books, movies, games, ratings, and wishlist.",
  },
  {
    title: "Personal Dashboard",
    desc: "Daily agenda, quick metrics, high-priority actions, and notes",
    prompt:
      "Build a personal dashboard with daily agenda, quick actions, metric cards, and notes.",
  },
];

  if (messages.length === 0 && !liveHere) {
    return (
      <div className="message-list">
        <div className="chat-starter-container">
          <div className="empty-state" style={{ padding: 0 }}>
            <h3>{EMPTY_STATES.emptyChat.title}</h3>
            <p>{EMPTY_STATES.emptyChat.body}</p>
          </div>

          <h4 className="chat-starter-heading">Quick Starters</h4>
          <div className="chat-starter-grid" role="group" aria-label="Suggested starter applications">
            {STARTER_PROMPTS.map((starter) => (
              <button
                key={starter.title}
                type="button"
                className="chat-starter-card"
                onClick={() => {
                  window.dispatchEvent(
                    new CustomEvent("coreside:prefill-composer", {
                      detail: { text: starter.prompt },
                    }),
                  );
                }}
              >
                <span className="chat-starter-title">{starter.title}</span>
                <span className="chat-starter-desc">{starter.desc}</span>
              </button>
            ))}
          </div>

          <div className="button-row empty-state-actions" style={{ justifyContent: "center", marginTop: "var(--space-2)" }}>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => openHelpAndLearning(navigateToSettings)}
            >
              {EMPTY_STATES.helpLink.label}
            </button>
          </div>
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
  const lifecycleIndex = computeLiveApplicationGenerationIndex(
    visibleActions,
    Boolean(streamingText),
  );

  return (
    <div className="message-list" ref={listRef} aria-live="polite">
      {showNewUpdates ? (
        <button type="button" className="new-updates-chip" onClick={jumpToLatest}>
          New updates
        </button>
      ) : null}
      {messages
        .filter((message) => {
          const isSilent = Boolean(
            message.metadata &&
              typeof message.metadata === "object" &&
              (message.metadata as Record<string, unknown>).silent === true,
          );
          // Silent app interactions (e.g. game moves) remain persisted and auditable,
          // but hide ordinary chat bubbles unless Action Log mode is always on.
          return !isSilent || actionLogMode === "always";
        })
        .map((message) => (
          <MessageBubble key={message.id} message={message} />
        ))}
      {liveHere ? (
        <div className="message-bubble assistant agent-live" aria-label="Agent is responding">
          <div className="message-meta">
            <span>Coreside agent</span>
          </div>

          {/* Human-Readable Generation Lifecycle Indicator */}
          <div className="generation-lifecycle" aria-label="Application progress">
            {LIVE_APPLICATION_GENERATION_STEPS.map((label, index) => (
              <span key={label} style={{ display: "contents" }}>
                {index > 0 ? <span className="lifecycle-arrow">→</span> : null}
                <div
                  className={`lifecycle-step ${
                    lifecycleIndex === index
                      ? "active"
                      : lifecycleIndex > index
                        ? "complete"
                        : ""
                  }`}
                >
                  <span className="step-dot" /> {label}
                </div>
              </span>
            ))}
          </div>

          {showLiveActionLog && visibleActions.length > 0 ? (
            <ul className="agent-action-log" aria-label="Action log">
              {visibleActions.map((label, index) => (
                <li
                  key={`${index}-${label}`}
                  className={index === visibleActions.length - 1 ? "current" : ""}
                >
                  {humanizeAgentActionLabel(label)}
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted agent-action-pending">
              {showLiveActionLog &&
              latestAction &&
              !latestAction.toLowerCase().includes("model") &&
              !latestAction.toLowerCase().includes("unavailable")
                ? humanizeAgentActionLabel(latestAction)
                : "Working…"}
            </p>
          )}
          {streamingText ? (
            <div
              className="message-content agent-stream-text"
              role="status"
              aria-live="polite"
              aria-relevant="additions text"
              aria-label="Assistant response streaming"
              data-testid="assistant-stream-text"
              data-turn-id={activeTurnId ?? undefined}
              data-last-sequence={
                streamLastSequence != null ? String(streamLastSequence) : undefined
              }
              data-stream-sequence={
                streamLastSequence != null ? String(streamLastSequence) : undefined
              }
            >
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
