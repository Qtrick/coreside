import { useId } from "react";
import { ToolRenderer } from "@/components/tool-renderer/ToolRenderer";
import type { ToolDefinition, ToolState } from "@/types/tool";

export type RecoveredPreviewSurfaceView = {
  surfaceId: string;
  toolId: string | null;
  applicationId: string | null;
  tool: ToolDefinition;
  state: ToolState;
  revision: number;
  sequence: number;
};

export type RecoveredPreviewView = {
  turnId: string;
  conversationId: string;
  attemptId: string;
  fidelity: "full" | "partial";
  surfaces: RecoveredPreviewSurfaceView[];
  warnings: string[];
};

/**
 * Mounts never-applied (and existing) recovered surfaces in a dedicated
 * read-only host. Does not require a durable ToolCanvas / inline record.
 *
 * Privileged callbacks are omitted; ToolRenderer preview mode fails closed.
 */
export function RecoveredPreviewHost({
  preview,
  onClose,
}: {
  preview: RecoveredPreviewView;
  onClose: () => void;
}) {
  const titleId = useId();
  const partial = preview.fidelity === "partial";

  return (
    <section
      className="recovered-preview-host"
      role="region"
      aria-labelledby={titleId}
      aria-describedby={`${titleId}-desc`}
      data-testid="recovered-preview-host"
      data-fidelity={preview.fidelity}
      data-turn-id={preview.turnId}
    >
      <header className="recovered-preview-host-header">
        <div className="recovered-preview-host-copy">
          <span className="recovered-preview-badge" role="status" aria-atomic="true">
            Speculative preview
          </span>
          <h2 id={titleId}>Saved application preview</h2>
          <p id={`${titleId}-desc`}>
            {partial
              ? "Partially restored — some staged operations could not be reconstructed. This is inspect-only and cannot be applied."
              : "Read-only view of your last saved progressive preview. Nothing has been applied to your application."}
          </p>
          {partial && preview.warnings.length > 0 ? (
            <p className="recovered-preview-warnings" role="status">
              {preview.warnings.slice(0, 3).join("; ")}
            </p>
          ) : null}
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={onClose}
          aria-label="Close recovered preview"
        >
          Close Preview
        </button>
      </header>
      <div className="recovered-preview-host-surfaces">
        {preview.surfaces.map((surface) => (
          <article
            key={surface.surfaceId}
            className="recovered-preview-surface"
            data-testid="recovered-preview-surface"
            data-surface-id={surface.surfaceId}
          >
            <div className="recovered-preview-surface-meta">
              <strong>{surface.tool.name || "Application"}</strong>
              <span className="muted">
                {surface.toolId ?? surface.surfaceId}
              </span>
            </div>
            <div className="recovered-preview-surface-body">
              <ToolRenderer
                tool={surface.tool}
                state={surface.state}
                mode="preview"
                applicationId={surface.applicationId}
                surfaceId={surface.surfaceId}
                conversationId={preview.conversationId}
                onStateChange={() => undefined}
                onPersistState={async () => undefined}
                onSubmitToAgent={undefined}
                onInteractiveDispatch={undefined}
                onPendingApproval={undefined}
              />
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
