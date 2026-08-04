import { AlertTriangle, Lock, Pencil, ShieldAlert } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import { api } from "@/lib/tauri";
import type { ApprovalRequest, RememberDuration, RememberScope } from "@/types/application-kernel";

function riskPresentation(risk: string, critical: boolean) {
  if (critical) {
    return {
      label: "Critical action",
      icon: ShieldAlert,
      className: "approval-risk-critical",
    };
  }
  switch (risk) {
    case "destructive":
      return {
        label: "Destructive — may remove data",
        icon: AlertTriangle,
        className: "approval-risk-destructive",
      };
    case "write":
      return {
        label: "Write — changes local data",
        icon: Pencil,
        className: "approval-risk-write",
      };
    default:
      return {
        label: "Read — views local data",
        icon: Lock,
        className: "approval-risk-read",
      };
  }
}

/** Session and standing remember options share the same eligibility rules. */
function canRemember(risk: string, critical: boolean): boolean {
  return !critical && risk !== "destructive";
}

type ApprovalCardProps = {
  approval: ApprovalRequest;
  applicationName?: string | null;
  onResolved?: () => void;
};

export function ApprovalCard({
  approval,
  applicationName,
  onResolved,
}: ApprovalCardProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLElement>(null);

  const risk = riskPresentation(approval.risk, approval.critical);
  const RiskIcon = risk.icon;
  const expired = approval.status === "expired";
  const consumed = approval.status === "consumed";
  const decided = approval.status === "approved" || approval.status === "denied";
  const inactive = expired || consumed || decided || approval.status !== "pending";

  // Focus the dialog container (not Approve once) so window/session focus
  // or an accidental Enter cannot auto-approve a pending request.
  // useLayoutEffect runs before ModalPortal's rAF autofocus check, so the
  // portal sees an in-dialog activeElement and skips focusing Approve once.
  useLayoutEffect(() => {
    if (inactive) return;
    dialogRef.current?.focus({ preventScroll: true });
  }, [inactive, approval.id]);

  const decide = async (
    approve: boolean,
    rememberScope?: RememberScope,
    rememberDuration?: RememberDuration,
  ) => {
    setBusy(true);
    setError(null);
    try {
      await api.kernelDecideApproval(
        approval.id,
        approve,
        rememberScope ?? null,
        rememberDuration ?? null,
      );
      onResolved?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not decide approval");
    } finally {
      setBusy(false);
    }
  };

  return (
    <article
      ref={dialogRef}
      className="approval-card settings-section"
      role="dialog"
      tabIndex={-1}
      aria-labelledby={`approval-${approval.id}-title`}
      aria-describedby={`approval-${approval.id}-desc`}
    >
      <header className="approval-card-header">
        <div>
          <p className="muted approval-card-eyebrow">Permission request</p>
          <h3 id={`approval-${approval.id}-title`}>{approval.actionTitle}</h3>
          {applicationName ? (
            <p className="muted approval-card-app">
              Application: <strong>{applicationName}</strong>
            </p>
          ) : approval.applicationId ? (
            <p className="muted approval-card-app">
              Application: <code>{approval.applicationId}</code>
            </p>
          ) : null}
        </div>
        <div className={`approval-risk ${risk.className}`}>
          <RiskIcon size={16} aria-hidden />
          <span>{risk.label}</span>
        </div>
      </header>

      <div id={`approval-${approval.id}-desc`}>
        {approval.explanation ? (
          <p className="approval-card-reason">{approval.explanation}</p>
        ) : (
          <p className="approval-card-reason muted">
            This application wants to run a protected action.
          </p>
        )}

        {approval.inputPreview ? (
          <div className="approval-card-preview">
            <p className="settings-subheading">Input preview</p>
            <pre>{approval.inputPreview}</pre>
          </div>
        ) : null}

        <p className="muted approval-card-meta">
          Venue: {approval.venue} · Presence: {approval.presence}
          {approval.surfaceId ? ` · Surface ${approval.surfaceId}` : ""}
        </p>

        {inactive ? (
          <p className="provider-error" role="status">
            {expired
              ? "This approval request has expired."
              : consumed
                ? "This approval was already used."
                : decided
                  ? `This request was ${approval.status}.`
                  : `This request is ${approval.status}.`}
          </p>
        ) : null}

        {error ? (
          <p className="provider-error" role="alert">
            {error}
          </p>
        ) : null}
      </div>

      {!inactive ? (
        <div className="approval-card-actions button-row">
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() => void decide(true)}
          >
            Approve once
          </button>
          {canRemember(approval.risk, approval.critical) ? (
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busy}
              onClick={() => void decide(true, "action", "session")}
            >
              Allow for this session
            </button>
          ) : null}
          {canRemember(approval.risk, approval.critical) ? (
            <button
              type="button"
              className="btn btn-secondary"
              disabled={busy}
              onClick={() => void decide(true, "application_action", "standing")}
            >
              Always allow for this application
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-danger"
            disabled={busy}
            onClick={() => void decide(false)}
          >
            Deny
          </button>
        </div>
      ) : null}
    </article>
  );
}
