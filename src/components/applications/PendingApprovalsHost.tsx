import { useCallback, useEffect, useState } from "react";
import { OverlayPortal } from "@/components/ui/OverlayPortal";
import { ApprovalCard } from "@/components/applications/ApprovalCard";
import { api, listenApprovalsChanged } from "@/lib/tauri";
import type { ApprovalRequest, ManifestRecord } from "@/types/application-kernel";

export function PendingApprovalsHost() {
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);
  const [appNames, setAppNames] = useState<Record<string, string>>({});

  const reload = useCallback(async () => {
    try {
      const [pending, manifests] = await Promise.all([
        api.kernelListPendingApprovals(),
        api.kernelListManifests().catch(() => [] as ManifestRecord[]),
      ]);
      setApprovals(pending);
      const names: Record<string, string> = {};
      for (const record of manifests) {
        names[record.applicationId] = record.manifest.name;
      }
      setAppNames(names);
    } catch {
      setApprovals([]);
    }
  }, []);

  useEffect(() => {
    void reload();
    const onFocus = () => void reload();
    const onPending = () => void reload();
    // The trusted core emits whenever approvals, grants, or away-parked
    // requests change, so no timer is needed. Focus stays as a cheap catch-up
    // for anything that changed while this window was not listening.
    let stop: (() => void) | null = null;
    let disposed = false;
    void listenApprovalsChanged(() => void reload()).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    window.addEventListener("focus", onFocus);
    window.addEventListener("coreside:pending-approval", onPending);
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("coreside:pending-approval", onPending);
    };
  }, [reload]);

  if (approvals.length === 0) return null;

  // Non-blocking host: approvals must not mark #root inert (Journey 7 /
  // multi-window approval race needs the rest of the app clickable).
  return (
    <OverlayPortal>
      <div className="approval-host" aria-live="polite">
        {approvals.map((approval) => (
          <ApprovalCard
            key={approval.id}
            approval={approval}
            applicationName={
              approval.applicationId
                ? appNames[approval.applicationId] ?? null
                : null
            }
            onResolved={() => void reload()}
          />
        ))}
      </div>
    </OverlayPortal>
  );
}
