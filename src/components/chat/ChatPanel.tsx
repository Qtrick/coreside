import { MessageList } from "./MessageList";
import { Composer } from "./Composer";
import { ConversationQueue } from "./ConversationQueue";
import { ConversationHistory } from "./ConversationHistory";
import { ProjectIndicator } from "./ProjectIndicator";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { useAppStore } from "@/stores/app-store";

export function ChatPanel() {
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const conversations = useAppStore((s) => s.conversations);
  const projects = useAppStore((s) => s.projects);
  const createConversation = useAppStore((s) => s.createConversation);
  const navigateToProject = useAppStore((s) => s.navigateToProject);
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);

  const active = conversations.find((c) => c.id === activeConversationId);
  const activeProject = active?.projectId
    ? projects.find((p) => p.id === active.projectId)
    : null;

  return (
    <section className="chat-panel" aria-label="Conversation">
      <header className="panel-header">
        <div>
          <h1>{active?.title ?? "Conversation"}</h1>
          <div className="panel-header-meta">
            {activeProject ? (
              <ProjectIndicator
                project={activeProject}
                onClick={() => void navigateToProject(activeProject.id)}
              />
            ) : null}
            <p className="panel-subtitle" style={{ margin: 0 }}>
              {active
                ? "Chat with the Coreside agent"
                : "Select a chat or start a new one"}
            </p>
          </div>
        </div>
        <div className="panel-header-actions">
          {activeConversationId ? (
            <ConversationHistory conversationId={activeConversationId} />
          ) : null}
          {!active ? (
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void createConversation()}
            >
              New chat
            </button>
          ) : null}
        </div>
      </header>
      {activeConversationId ? (
        <>
          <ConversationQueue conversationId={activeConversationId} />
          <MessageList />
          <Composer />
        </>
      ) : (
        <div className="empty-state">
          <h3>{EMPTY_STATES.noConversation.title}</h3>
          <p>{EMPTY_STATES.noConversation.body}</p>
          <div className="button-row empty-state-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void createConversation()}
            >
              {EMPTY_STATES.noConversation.primaryCta}
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
      )}
    </section>
  );
}
