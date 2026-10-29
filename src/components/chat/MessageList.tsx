import { useEffect, useRef, useState } from "react";
import { MessageSquare } from "lucide-react";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri";
import type { ChatMessage } from "@/types/messages";
import { shouldShowActionLog } from "@/lib/action-log";
import { deriveAgentPresence } from "@/lib/agent-activity";
import { parseAutomationChange } from "@/lib/automation-proposal";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { kernelProposalFromMetadata } from "@/components/chat/proposal-metadata";
import { ActionTimelineCard } from "@/components/action-log/ActionLog";
import { AgentPresence } from "./AgentPresence";
import { AnimatedRevealText } from "./AnimatedRevealText";
import { MessageBubble } from "./MessageBubble";

function messageHasConfirmableProposal(
  metadata: Record<string, unknown> | null | undefined,
): boolean {
  if (kernelProposalFromMetadata(metadata)) return true;
  return Boolean(parseAutomationChange(metadata?.automationChange));
}

interface MessageListProps {
  conversationId?: string | null;
}

export function MessageList({ conversationId: propConversationId }: MessageListProps = {}) {
  const globalConversationId = useAppStore((s) => s.activeConversationId);
  const activeConversationId =
    propConversationId !== undefined ? propConversationId : globalConversationId;
  const storeMessages = useAppStore((s) => s.messages);
  const isBackgroundPane = Boolean(
    propConversationId && propConversationId !== globalConversationId,
  );
  const [backgroundMessages, setBackgroundMessages] = useState<ChatMessage[] | null>(null);

  useEffect(() => {
    if (isBackgroundPane && propConversationId) {
      let cancelled = false;
      void api.getMessages(propConversationId).then(
        (msgs) => {
          if (!cancelled) setBackgroundMessages(msgs);
        },
        () => {
          if (!cancelled) setBackgroundMessages([]);
        },
      );
      return () => {
        cancelled = true;
      };
    } else {
      setBackgroundMessages(null);
    }
  }, [isBackgroundPane, propConversationId]);

  const messages = isBackgroundPane && backgroundMessages !== null
    ? backgroundMessages
    : storeMessages;

  const sending = useAppStore((s) => s.sending);
  const sendingConversationId = useAppStore((s) => s.sendingConversationId);
  const agentActions = useAppStore((s) => s.agentActions);
  const actionLogMode = useAppStore((s) => s.actionLogMode);
  const streamingText = useAppStore((s) => s.streamingText);
  const messagesLoading = useAppStore((s) => s.messagesLoading);
  const messagesError = useAppStore((s) => s.messagesError);
  const sendError = useAppStore((s) => s.sendError);
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);
  const activeTurnId = useAppStore((s) =>
    activeConversationId
      ? (s.activeTurnIdByConversation[activeConversationId] ?? null)
      : null,
  );
  const streamLastSequence = useAppStore((s) => {
    if (!activeConversationId) return null;
    const turnId = s.activeTurnIdByConversation[activeConversationId];
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
  /** Keep the live row mounted until letter reveal finishes after the turn ends. */
  const [holdStreamText, setHoldStreamText] = useState("");
  const [holdingReveal, setHoldingReveal] = useState(false);
  const prevMessageCount = useRef(messages.length);
  const liveHere =
    sending &&
    Boolean(activeConversationId) &&
    sendingConversationId === activeConversationId;

  useEffect(() => {
    if (streamingText && liveHere) {
      setHoldStreamText(streamingText);
      setHoldingReveal(true);
    }
  }, [streamingText, liveHere]);

  useEffect(() => {
    if (liveHere) setHoldingReveal(true);
  }, [liveHere]);

  useEffect(() => {
    setHoldingReveal(false);
    setHoldStreamText("");
  }, [activeConversationId]);

  // Proposal turns must show the durable assistant bubble (Apply + confirm cards).
  // Letter-reveal hold must not keep hiding that bubble after the stream ends.
  useEffect(() => {
    if (liveHere || !holdingReveal) return;
    const last = messages[messages.length - 1];
    if (
      last?.role === "assistant" &&
      messageHasConfirmableProposal(
        last.metadata as Record<string, unknown> | null | undefined,
      )
    ) {
      setHoldingReveal(false);
      setHoldStreamText("");
    }
  }, [holdingReveal, liveHere, messages]);

  const showLiveRow = liveHere || holdingReveal;
  const revealText = streamingText || holdStreamText;

  const finishRevealHold = () => {
    if (liveHere) return;
    setHoldingReveal(false);
    setHoldStreamText("");
  };

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
            <div className="empty-icon" aria-hidden>
              <MessageSquare size={24} />
            </div>
            <h3>{EMPTY_STATES.emptyChat.title}</h3>
            <p>{EMPTY_STATES.emptyChat.body}</p>
            <div className="button-row empty-state-actions">
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => {
                  document.getElementById("composer-input")?.focus();
                }}
              >
                {EMPTY_STATES.emptyChat.primaryCta}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => openHelpAndLearning(navigateToSettings)}
              >
                {EMPTY_STATES.helpLink.label}
              </button>
            </div>
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
        </div>
      </div>
    );
  }

  const visibleActions = agentActions.filter((entry) => {
    const lower = entry.label.toLowerCase();
    return !(
      lower.includes("working model")
      || lower.includes("model availability")
      || lower.includes("trying another")
      || lower.includes("unavailable")
      || lower.includes("selecting a")
    );
  });
  const showLiveActionLog = shouldShowActionLog(actionLogMode, visibleActions);
  // Plain derive (not useMemo): this path sits after early returns; hooks must not.
  const presence = deriveAgentPresence({
    actions: visibleActions,
    streaming: Boolean(streamingText),
    sending: liveHere,
    error: liveHere ? sendError : null,
  });

  return (
    <div className="message-list" ref={listRef} aria-live="polite">
      {showNewUpdates ? (
        <button type="button" className="new-updates-chip" onClick={jumpToLatest}>
          New updates
        </button>
      ) : null}
      {messages
        .filter((message, index, list) => {
          if (
            activeConversationId &&
            message.conversationId !== activeConversationId
          ) {
            return false;
          }
          const isSilent = Boolean(
            message.metadata &&
            typeof message.metadata === "object" &&
            (message.metadata as Record<string, unknown>).silent === true,
          );
          // Silent app interactions (e.g. game moves) remain persisted and auditable,
          // but hide ordinary chat bubbles unless Action Log mode is always on.
          if (isSilent && actionLogMode !== "always") return false;
          // Avoid duplicating the trailing assistant message while letter-reveal finishes.
          // Keep proposal messages visible — they own Apply + confirm cards.
          if (
            holdingReveal &&
            !liveHere &&
            message.role === "assistant" &&
            index === list.length - 1
          ) {
            if (
              messageHasConfirmableProposal(
                message.metadata as Record<string, unknown> | null | undefined,
              )
            ) {
              return true;
            }
            return false;
          }
          return true;
        })
        .map((message) => (
          <MessageBubble key={message.id} message={message} />
        ))}
      {showLiveRow ? (
        <div className="message-row assistant agent-live" aria-label="Agent is responding">
          {liveHere ? <AgentPresence presence={presence} /> : null}
          {liveHere && showLiveActionLog && visibleActions.length > 0 ? (
            <ActionTimelineCard events={visibleActions} live />
          ) : null}
          {revealText ? (
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
                <AnimatedRevealText text={revealText} onCaughtUp={finishRevealHold} />
                {liveHere ? <span className="stream-caret" aria-hidden /> : null}
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
