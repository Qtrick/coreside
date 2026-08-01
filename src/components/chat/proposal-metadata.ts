/** Extract proposal fields from assistant message metadata. */
export function kernelProposalFromMetadata(
  metadata: Record<string, unknown> | null | undefined,
): {
  proposalId: string;
  summary: string;
  impactSummary: string;
  risk: string;
  operations: unknown[];
  status: string;
} | null {
  if (!metadata || typeof metadata !== "object") return null;
  const rv = metadata.runtimeV2;
  if (!rv || typeof rv !== "object") return null;
  const obj = rv as Record<string, unknown>;
  const proposalId = typeof obj.proposalId === "string" ? obj.proposalId : "";
  if (!proposalId) return null;
  const operations = Array.isArray(obj.operations) ? obj.operations : [];
  if (operations.length === 0) return null;
  const status =
    typeof obj.status === "string"
      ? obj.status
      : typeof metadata.kernelProposalStatus === "string"
        ? metadata.kernelProposalStatus
        : "pending";
  return {
    proposalId,
    summary:
      typeof obj.summary === "string" ? obj.summary : "Proposed application change",
    impactSummary:
      typeof obj.impactSummary === "string" ? obj.impactSummary : "",
    risk: typeof obj.risk === "string" ? obj.risk : "strong",
    operations,
    status,
  };
}
