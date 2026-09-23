import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import type { ToolDefinition, ToolState } from "@/types/tool";

/**
 * Risk-based change proposal card in the chat stream.
 * Loads authoritative proposal from the database row.
 */
export function ChangeProposalCard({
  proposalId,
  summary: initialSummary,
  impactSummary: initialImpactSummary,
  risk: initialRisk,
  operations: initialOperations,
  messageId,
  conversationId,
  status: initialStatus,
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
  const [previewState, setPreviewState] = useState<ToolState>({});
  const [proposalData, setProposalData] = useState<{
    summary: string;
    impactSummary: string;
    risk: string;
    status: string;
    operations: unknown[];
    error?: string | null;
  } | null>(null);

  useEffect(() => {
    let active = true;
    if (!proposalId) return;
    api
      .kernelGetProposal(proposalId)
      .then((rec) => {
        if (!active) return;
        setProposalData({
          summary: (rec.summary as string) || initialSummary,
          impactSummary: (rec.impactSummary as string) || initialImpactSummary,
          risk: (rec.risk as string) || initialRisk,
          status: (rec.status as string) || initialStatus || "pending",
          operations: (rec.exactOperations as unknown[]) || initialOperations,
          error: (rec.error as string) || null,
        });
      })
      .catch(() => {
        // Fallback to props
      });
    return () => {
      active = false;
    };
  }, [proposalId, initialSummary, initialImpactSummary, initialRisk, initialStatus, initialOperations]);

  const currentStatus = proposalData?.status ?? initialStatus ?? "pending";
  const summary = proposalData?.summary ?? initialSummary;
  const impactSummary = proposalData?.impactSummary ?? initialImpactSummary;
  const risk = proposalData?.risk ?? initialRisk;
  const operations = proposalData?.operations ?? initialOperations;
  const proposalError = proposalData?.error ?? error;

  const previewTool = useMemo<ToolDefinition | null>(() => {
    for (const op of (operations as Array<Record<string, unknown>>)) {
      if (!op || typeof op !== "object") continue;
      const payload = op.payload as Record<string, unknown> | undefined;
      if (!payload) continue;
      if (payload.tool && typeof payload.tool === "object") {
        return payload.tool as ToolDefinition;
      }
      if (Array.isArray(payload.components)) {
        return {
          id: (payload.id as string) || "preview-tool",
          name: (payload.name as string) || summary || "Preview Tool",
          description: (payload.description as string) || "",
          layout: (payload.layout as Record<string, unknown>) || { type: "single-column" },
          components: payload.components,
        } as unknown as ToolDefinition;
      }
    }
    return null;
  }, [operations, summary]);

  if (currentStatus === "applied" || currentStatus === "discarded" || currentStatus === "rejected") {
    return (
      <aside
        className="change-proposal"
        aria-label="Proposed change resolved"
        style={{ margin: "0.75rem 0" }}
      >
        <p className="muted" style={{ margin: 0 }}>
          Change {currentStatus}
          {proposalId ? ` · ${proposalId.slice(0, 12)}…` : ""}
        </p>
      </aside>
    );
  }

  if (currentStatus === "stale" || currentStatus === "expired" || currentStatus === "failed") {
    return (
      <aside
        className="change-proposal"
        aria-label="Proposed change unresolved"
        style={{
          border: "1px solid var(--border)",
          borderRadius: 12,
          padding: "0.85rem 1rem",
          margin: "0.75rem 0",
          background: "var(--core-muted-overlay, var(--surface-muted, var(--surface)))",
        }}
      >
        <p style={{ margin: 0 }}>
          <strong>Proposal {currentStatus}</strong>{" "}
          <span className="muted">({proposalId.slice(0, 12)}…)</span>
        </p>
        <p className="muted" style={{ margin: "0.35rem 0 0" }}>
          {proposalError || `This proposal can no longer be applied (${currentStatus}).`}
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
        background: "var(--core-muted-overlay, var(--surface-muted, var(--surface)))",
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
      {previewTool ? (
        <div
          className="proposal-tool-visual-preview"
          style={{
            marginTop: "0.75rem",
            padding: "0.75rem",
            borderRadius: "var(--radius-md)",
            border: "1px dashed var(--border)",
            background: "var(--core-card-overlay)",
            maxHeight: "320px",
            overflowY: "auto",
          }}
        >
          <ToolRenderer
            tool={previewTool}
            state={previewState}
            mode="preview"
            onStateChange={(nextState) => setPreviewState(nextState)}
          />
        </div>
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
