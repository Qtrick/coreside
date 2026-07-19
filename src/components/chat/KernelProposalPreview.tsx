import { ChangeProposalCard } from "./ChangeProposalCard";
import { useAppStore } from "@/stores/app-store";

/** Sticky composer banner for the active pending Application Kernel proposal. */
export function KernelProposalPreview() {
  const pending = useAppStore((s) => s.pendingKernelProposal);

  if (!pending) return null;

  return (
    <ChangeProposalCard
      proposalId={pending.proposalId}
      summary={pending.summary}
      impactSummary={pending.impactSummary}
      risk={pending.risk}
      operations={pending.operations}
      messageId={pending.messageId}
      conversationId={pending.conversationId}
      status="pending"
    />
  );
}
