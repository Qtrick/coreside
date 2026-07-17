import { MessageList } from "./MessageList";
import { Composer } from "./Composer";
import { useAppStore } from "@/stores/app-store";

export function ChatPanel() {
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const conversations = useAppStore((s) => s.conversations);
  const createConversation = useAppStore((s) => s.createConversation);

  const active = conversations.find((c) => c.id === activeConversationId);

  return (
    <section className="chat-panel" aria-label="Conversation">
      <header className="panel-header">
        <div>
          <h1>{active?.title ?? "Conversation"}</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            {active
              ? "Chat with the Coreside agent"
              : "Select a chat or start a new one"}
          </p>
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
