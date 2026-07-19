import { useState } from "react";
import { useAppStore } from "@/stores/app-store";

/**
 * Risk-based change proposal card in the chat stream.
 * Technical JSON is not shown unless Developer Mode is used later.
 */
export function ChangeProposalCard({
  proposalId,
  summary,
  impactSummary,
  risk,
  operations,
  messageId,
  conversationId,
  status,
}: {
  proposalId: string;
  summary: string;
  impactSummary: string;
  risk: string;
  operations: unknown[];
  messageId: string;
  conversationId: string;
  status?: string;
}) {
  const applyPending = useAppStore((s) => s.applyPendingKernelProposal);
  const discardPending = useAppStore((s) => s.discardPendingKernelProposal);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (status === "applied" || status === "discarded") {
    return (
      <aside
        className="change-proposal"
        aria-label="Proposed change resolved"
        style={{ margin: "0.75rem 0" }}
      >
        <p className="muted" style={{ margin: 0 }}>
          Change {status}
          {proposalId ? ` · ${proposalId.slice(0, 12)}…` : ""}
        </p>
      </aside>
    );
  }

  const stagePending = () => {
    useAppStore.setState({
      pendingKernelProposal: {
        conversationId,
        messageId,
        proposalId,
        summary,
        impactSummary,
        risk,
        operations,
      },
    });
  };

  const apply = async () => {
    setBusy(true);
    setError(null);
    try {
      stagePending();
      await applyPending();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not apply change");
    } finally {
      setBusy(false);
    }
  };

  const cancel = async () => {
    setBusy(true);
    setError(null);
    try {
      stagePending();
      await discardPending();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not discard change");
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside
      className="change-proposal"
      aria-label="Proposed change"
      style={{
        border: "1px solid var(--border)",
        borderRadius: 12,
        padding: "0.85rem 1rem",
        margin: "0.75rem 0",
        background: "var(--surface-muted, var(--surface))",
      }}
    >
      <p style={{ margin: 0 }}>
        <strong>Proposed change</strong>{" "}
        <span className="muted">
          ({risk} risk · {proposalId.slice(0, 12)}…)
        </span>
      </p>
      <p style={{ margin: "0.35rem 0 0" }}>{summary}</p>
      <p className="muted" style={{ margin: "0.35rem 0 0" }}>
        {impactSummary}
      </p>
      {operations.length === 0 ? (
        <p className="muted" role="status" style={{ margin: "0.35rem 0 0" }}>
          This proposal has no operations to apply.
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="muted">
          {error}
        </p>
      ) : null}
      <div className="button-row" style={{ marginTop: "0.75rem" }}>
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy || operations.length === 0}
          onClick={() => void apply()}
        >
          Apply
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() => void cancel()}
        >
          Discard
        </button>
      </div>
    </aside>
  );
}

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
