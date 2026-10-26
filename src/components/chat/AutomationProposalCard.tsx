import { useAppStore } from "@/stores/app-store";
import {
  describeAutomationAction,
  describeAutomationTrigger,
  type AutomationChangeProposal,
  type AutomationChangeStatus,
} from "@/lib/automation-proposal";

export function AutomationProposalCard({
  proposal,
  status,
  messageId,
  conversationId,
  placement = "inline",
}: {
  proposal: AutomationChangeProposal;
  status: AutomationChangeStatus;
  messageId: string;
  conversationId: string;
  /** Composer sticky owns Confirm when the same proposal is staged for this chat. */
  placement?: "inline" | "composer";
}) {
  const pending = useAppStore((s) => s.pendingAutomationChange);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const applying = useAppStore((s) => s.automationProposalApplying);
  const stage = useAppStore((s) => s.stagePendingAutomationChange);
  const apply = useAppStore((s) => s.applyPendingAutomationChange);
  const discard = useAppStore((s) => s.discardPendingAutomationChange);

  const resolved = status === "applied" || status === "discarded" || status === "failed";
  const stagedHere =
    pending?.messageId === messageId && pending.conversationId === conversationId;
  // Avoid two competing Confirm rows — sticky composer owns actions while staged
  // for the open conversation. Mismatched conversation / missing stage keeps inline.
  const deferActionsToComposer =
    placement === "inline" &&
    stagedHere &&
    pending.conversationId === activeConversationId &&
    status === "pending";
  const showActions = status === "pending" && !deferActionsToComposer;

  const summary =
    proposal.changeSummary?.trim() ||
    "Review this automation before Coreside schedules it.";

  const ensureStagedThen = (fn: () => Promise<void>) => {
    if (!stagedHere) {
      stage({ conversationId, messageId, proposal });
    }
    void fn();
  };

  return (
    <div
      className="tool-change-preview automation-proposal-card"
      role="region"
      aria-label="Automation proposal"
    >
      <h3>Automation</h3>
      <p className="muted" style={{ margin: 0 }}>
        {summary}
      </p>
      <ul className="muted" style={{ margin: "0.5rem 0 0", paddingLeft: "1.25rem" }}>
        <li>
          <strong>{proposal.name}</strong>
        </li>
        <li>{describeAutomationTrigger(proposal.trigger)}</li>
        <li>{describeAutomationAction(proposal.action)}</li>
        {proposal.requiresAi || proposal.action.type === "ai_prompt" ? (
          <li>Uses your connected AI provider (read-only summary)</li>
        ) : null}
        {proposal.applicationId ? (
          <li>Scoped to application {proposal.applicationId}</li>
        ) : null}
        <li>Runs only while Coreside is open</li>
      </ul>
      {resolved ? (
        <>
          <p className="muted" role="status" style={{ margin: "0.65rem 0 0" }}>
            {status === "applied"
              ? "Scheduled. Open Automations to pause, run, or review history."
              : status === "discarded"
                ? "Discarded."
                : "Could not schedule."}
          </p>
          {status === "applied" ? (
            <div className="preview-actions" style={{ marginTop: "0.5rem" }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => useAppStore.getState().navigateToAutomations()}
              >
                Open Automations
              </button>
            </div>
          ) : null}
        </>
      ) : showActions ? (
        <div className="preview-actions">
          <button
            type="button"
            className="btn btn-primary"
            disabled={applying}
            onClick={() => ensureStagedThen(apply)}
          >
            {applying ? "Scheduling…" : "Confirm"}
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            disabled={applying}
            onClick={() => ensureStagedThen(discard)}
          >
            Discard
          </button>
        </div>
      ) : null}
    </div>
  );
}

/** Sticky composer preview when a turn left a pending automation. */
export function AutomationProposalPreview() {
  const pending = useAppStore((s) => s.pendingAutomationChange);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  if (!pending || pending.conversationId !== activeConversationId) return null;
  return (
    <AutomationProposalCard
      proposal={pending.proposal}
      status="pending"
      messageId={pending.messageId}
      conversationId={pending.conversationId}
      placement="composer"
    />
  );
}
