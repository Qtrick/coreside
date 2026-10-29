import { useMemo } from "react";
import {
  ArrowRight,
  FileText,
  LayoutGrid,
  MessageSquare,
  Sparkles,
  Zap,
} from "lucide-react";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { Composer } from "@/components/chat/Composer";
import { useAppStore } from "@/stores/app-store";

let sessionGreeting: string | null = null;

function getContextualGreeting(): string {
  if (sessionGreeting) return sessionGreeting;
  const hour = new Date().getHours();
  if (hour >= 5 && hour < 12) {
    sessionGreeting = "Good morning.";
  } else if (hour >= 12 && hour < 17) {
    sessionGreeting = "Good afternoon.";
  } else {
    sessionGreeting = "Good evening.";
  }
  return sessionGreeting;
}

interface WorkspaceDashboardProps {
  conversationId?: string | null;
  onActionSelect?: (prompt: string) => void;
  autoFocus?: boolean;
}

export function WorkspaceDashboard({
  conversationId,
  onActionSelect,
  autoFocus = true,
}: WorkspaceDashboardProps) {
  // Stable greeting computed once per mount
  const greeting = useMemo(() => getContextualGreeting(), []);
  const resolvedTheme = useAppStore((s) => s.resolvedTheme);
  const createConversation = useAppStore((s) => s.createConversation);
  const navigateToAutomations = useAppStore((s) => s.navigateToAutomations);

  const prefillComposer = (text: string) => {
    if (onActionSelect) {
      onActionSelect(text);
    }
    window.dispatchEvent(
      new CustomEvent("coreside:prefill-composer", {
        detail: { text, conversationId: conversationId ?? null },
      }),
    );
  };

  const handleStartNewChat = () => {
    void createConversation();
  };

  const handleSummarizeDocument = () => {
    prefillComposer("Please summarize the following document: ");
  };

  const handleCreateApp = () => {
    prefillComposer("Build a personal interactive application for ");
  };

  const handleSetupAutomation = () => {
    navigateToAutomations();
  };

  const suggestions = [
    {
      icon: <Sparkles size={16} className="suggestion-icon" aria-hidden />,
      text: "Help me plan my day",
    },
    {
      icon: <MessageSquare size={16} className="suggestion-icon" aria-hidden />,
      text: "Research a topic",
    },
    {
      icon: <FileText size={16} className="suggestion-icon" aria-hidden />,
      text: "Analyze this file",
    },
    {
      icon: <LayoutGrid size={16} className="suggestion-icon" aria-hidden />,
      text: "Turn an idea into an app",
    },
  ];

  return (
    <div
      className="workspace-dashboard"
      role="region"
      aria-label="Workspace Dashboard"
      data-testid="workspace-dashboard"
    >
      <div className="dashboard-content-scroll">
        <div className="dashboard-hero">
          {/* Centered Coreside flower mark in rounded container */}
          <div className="dashboard-mark-container">
            <CoresideLogo appearance={resolvedTheme} size={36} />
          </div>

          <h1 className="dashboard-title">{greeting}</h1>
          <p className="dashboard-subtitle">What would you like to work on today?</p>
        </div>

        {/* 4 Primary Quick Action Cards in horizontal row */}
        <div className="dashboard-quick-actions" role="region" aria-label="Quick actions">
          <button
            type="button"
            className="quick-action-card"
            onClick={handleStartNewChat}
          >
            <div className="quick-action-icon">
              <MessageSquare size={20} />
            </div>
            <div className="quick-action-title">Start a new conversation</div>
          </button>

          <button
            type="button"
            className="quick-action-card"
            onClick={handleSummarizeDocument}
          >
            <div className="quick-action-icon">
              <FileText size={20} />
            </div>
            <div className="quick-action-title">Summarize a document</div>
          </button>

          <button
            type="button"
            className="quick-action-card"
            onClick={handleCreateApp}
          >
            <div className="quick-action-icon">
              <LayoutGrid size={20} />
            </div>
            <div className="quick-action-title">Create an application</div>
          </button>

          <button
            type="button"
            className="quick-action-card"
            onClick={handleSetupAutomation}
          >
            <div className="quick-action-icon">
              <Zap size={20} />
            </div>
            <div className="quick-action-title">Set up an automation</div>
          </button>
        </div>

        {/* Suggested Prompts Stacked Card */}
        <div
          className="dashboard-suggestions-card"
          role="region"
          aria-label="Suggested prompts"
        >
          {suggestions.map((item) => (
            <button
              key={item.text}
              type="button"
              className="suggestion-row"
              onClick={() => prefillComposer(item.text)}
            >
              <div className="suggestion-left">
                {item.icon}
                <span className="suggestion-text">{item.text}</span>
              </div>
              <ArrowRight size={15} className="suggestion-arrow" aria-hidden />
            </button>
          ))}
        </div>
      </div>

      {/* Actual Chat Composer at bottom */}
      <div className="dashboard-composer-host">
        <Composer conversationId={conversationId} autoFocus={autoFocus} />
      </div>
    </div>
  );
}
