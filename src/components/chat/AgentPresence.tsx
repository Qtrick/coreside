import type { AgentPresence as Presence } from "@/lib/agent-activity";

/**
 * Compact agent activity indicator (OpenAI/Grok-style dots).
 * Driven only by deriveAgentPresence — never a decorative loop while idle.
 */
export function AgentPresence({ presence }: { presence: Presence }) {
  if (presence.phase === "idle" || !presence.label) return null;

  return (
    <div
      className={`agent-presence tone-${presence.tone} phase-${presence.phase}`}
      role="status"
      aria-live="polite"
      aria-atomic="true"
      aria-label={presence.label}
    >
      <span className="agent-presence-dots" aria-hidden>
        <span />
        <span />
        <span />
      </span>
      <span className="agent-presence-label">{presence.label}</span>
    </div>
  );
}
