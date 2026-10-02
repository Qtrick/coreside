import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import type { ToolDefinition, ToolState } from "@/types/tool";
import {
  consumerRiskHint,
  deriveEvolutionCopy,
  proposalStatusPresentation,
  sanitizeConsumerError,
} from "@/lib/application-proposal-status";

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
  preservationSummary: initialPreservationSummary = "",
}: {
  proposalId: string;
  summary: string;
  impactSummary: string;
  risk: string;
  operations: unknown[];
  messageId: string;
  conversationId: string;
  status?: string;
  preservationSummary?: string;
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

  const currentStatus =
    busy ? "applying" : (proposalData?.status ?? initialStatus ?? "pending");
  const summary = proposalData?.summary ?? initialSummary;
  const impactSummary = proposalData?.impactSummary ?? initialImpactSummary;
  const risk = proposalData?.risk ?? initialRisk;
  const operations = proposalData?.operations ?? initialOperations;
  const proposalError = proposalData?.error ?? error;
  const statusUi = proposalStatusPresentation(currentStatus);
  const riskHint = consumerRiskHint(risk);

  const evolution = useMemo(
    () =>
      deriveEvolutionCopy(
        operations,
        impactSummary,
        summary,
        initialPreservationSummary,
      ),
    [operations, impactSummary, summary, initialPreservationSummary],
  );

  const previewTool = useMemo<ToolDefinition | null>(() => {
    for (const op of operations as Array<Record<string, unknown>>) {
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

  const statusBadgeClass =
    statusUi.tone === "success"
      ? "proposal-status-badge success"
      : statusUi.tone === "error"
        ? "proposal-status-badge error"
        : statusUi.tone === "warning"
          ? "proposal-status-badge warning"
          : statusUi.tone === "active"
            ? "proposal-status-badge active"
            : "proposal-status-badge";

  if (statusUi.resolved) {
    const friendlyError =
      statusUi.tone === "error" || statusUi.tone === "warning"
        ? sanitizeConsumerError(proposalError)
        : null;
    return (
      <aside
        className="change-proposal"
        aria-label="Application change status"
        style={{ margin: "0.75rem 0" }}
      >
        <p style={{ margin: 0 }}>
          <span className={statusBadgeClass}>{statusUi.label}</span>
        </p>
        {friendlyError ? (
          <p className="muted" style={{ margin: "0.35rem 0 0" }}>{friendlyError}</p>
        ) : statusUi.label === "Discarded" ? (
          <p className="muted" style={{ margin: "0.35rem 0 0" }}>
            No changes were applied.
          </p>
        ) : statusUi.label === "Ready" ? (
          <p className="muted" style={{ margin: "0.35rem 0 0" }}>
            Your application is up to date.
          </p>
        ) : null}
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
      setError(sanitizeConsumerError(e instanceof Error ? e.message : null));
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
      setError(sanitizeConsumerError(e instanceof Error ? e.message : null));
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
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          alignItems: "center",
          gap: "0.5rem",
          marginBottom: "0.35rem",
        }}
      >
        <span className={statusBadgeClass}>{statusUi.label}</span>
        {riskHint ? <span className="muted" style={{ fontSize: "0.8rem" }}>{riskHint}</span> : null}
      </div>

      <p style={{ margin: 0 }}>
        <strong>{evolution.headline}</strong>
      </p>

      <div style={{ marginTop: "0.65rem" }}>
        <p className="muted" style={{ margin: 0, fontSize: "0.8rem", fontWeight: 600 }}>
          What will change
        </p>
        <p style={{ margin: "0.25rem 0 0" }}>{evolution.whatWillChange}</p>
      </div>

      <div style={{ marginTop: "0.65rem" }}>
        <p className="muted" style={{ margin: 0, fontSize: "0.8rem", fontWeight: 600 }}>
          What will be preserved
        </p>
        <p style={{ margin: "0.25rem 0 0" }}>{evolution.whatWillBePreserved}</p>
      </div>

      {operations.length === 0 ? (
        <p className="muted" role="status" style={{ margin: "0.65rem 0 0" }}>
          This preview has no changes to apply yet.
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
          aria-label="Application preview"
        >
          <p className="muted" style={{ margin: "0 0 0.5rem", fontSize: "0.75rem" }}>
            Preview ready
          </p>
          <ToolRenderer
            tool={previewTool}
            state={previewState}
            mode="preview"
            onStateChange={(nextState) => setPreviewState(nextState)}
          />
        </div>
      ) : null}

      {proposalError ? (
        <p className="muted" role="alert" style={{ margin: "0.65rem 0 0", color: "var(--destructive, #dc2626)" }}>
          {sanitizeConsumerError(proposalError)}
        </p>
      ) : null}

      <div className="button-row" style={{ marginTop: "0.75rem" }}>
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy || operations.length === 0}
          onClick={() => void apply()}
          aria-busy={busy}
        >
          {busy ? "Applying…" : "Apply"}
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
