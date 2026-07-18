import { MessageList } from "./MessageList";
import { Composer } from "./Composer";
import { ProjectIndicator } from "./ProjectIndicator";
import { useAppStore } from "@/stores/app-store";

export function ChatPanel() {
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const conversations = useAppStore((s) => s.conversations);
  const projects = useAppStore((s) => s.projects);
  const createConversation = useAppStore((s) => s.createConversation);
  const navigateToProject = useAppStore((s) => s.navigateToProject);

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
        {!active ? (
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void createConversation()}
          >
            New chat
          </button>
        ) : null}
      </header>
      {activeConversationId ? (
        <>
          <MessageList />
          <Composer />
        </>
      ) : (
        <div className="empty-state">
          <h3>No conversation selected</h3>
          <p>Start a new chat to begin building with Coreside.</p>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void createConversation()}
          >
            New chat
          </button>
        </div>
      )}
    </section>
  );
}
