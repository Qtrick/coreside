import { useEffect, useRef } from "react";
import { api, listenApprovalsChanged } from "@/lib/tauri";
import { readStoredSettingsCategory } from "@/lib/settings-categories";
import { CONTEXTUAL_TIP_IDS } from "@/lib/onboarding/tutorials";
import { useAppStore } from "@/stores/app-store";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { ContextualHint } from "./ContextualHint";
import { WhatsNewBanner } from "./WhatsNewBanner";
import { OverlayPortal } from "@/components/ui/OverlayPortal";

/**
 * Central first-use education watchers — keep triggers out of leaf UI.
 * Backup first-action education is intentionally skipped (no clear hook).
 */
export function ContextualEducationHost() {
  const offer = useOnboardingStore((s) => s.offerContextual);
  const bootstrapped = useOnboardingStore((s) => s.bootstrapped);
  const onboardingDisabled = useOnboardingStore((s) => s.onboardingDisabled);
  const phase = useOnboardingStore((s) => s.phase);
  const activeTool = useAppStore((s) => s.activeTool);
  const view = useAppStore((s) => s.view);
  const aiStatus = useAppStore((s) => s.aiStatus);
  const offeredRef = useRef(new Set<string>());

  const tryOffer = (tipId: string) => {
    if (!bootstrapped || onboardingDisabled) return;
    if (offeredRef.current.has(tipId)) return;
    if (offer(tipId)) offeredRef.current.add(tipId);
  };

  // First generated app
  useEffect(() => {
    if (activeTool) tryOffer(CONTEXTUAL_TIP_IDS.firstApp);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- offer identity is stable enough
  }, [activeTool?.id, bootstrapped, onboardingDisabled, phase]);

  // First project create/open
  useEffect(() => {
    if (view.kind === "project" || view.kind === "projects") {
      tryOffer(CONTEXTUAL_TIP_IDS.firstProject);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.kind, bootstrapped, onboardingDisabled, phase]);

  // First approval request
  useEffect(() => {
    if (!bootstrapped || onboardingDisabled) return;
    let stop: (() => void) | null = null;
    let disposed = false;

    const check = async () => {
      try {
        const pending = await api.kernelListPendingApprovals();
        if (!disposed && pending.length > 0) {
          tryOffer(CONTEXTUAL_TIP_IDS.firstApproval);
        }
      } catch {
        // best-effort
      }
    };

    void check();
    const onPending = () => void check();
    window.addEventListener("coreside:pending-approval", onPending);
    void listenApprovalsChanged(() => void check()).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener("coreside:pending-approval", onPending);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bootstrapped, onboardingDisabled, phase]);

  // First AI connection ready or AI connections settings opened
  useEffect(() => {
    if (aiStatus?.status === "ready") {
      tryOffer(CONTEXTUAL_TIP_IDS.firstAiConnection);
    }
    if (view.kind === "settings" && readStoredSettingsCategory() === "ai-access") {
      tryOffer(CONTEXTUAL_TIP_IDS.firstAiConnection);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    aiStatus?.status,
    aiStatus?.accessMode,
    view.kind,
    bootstrapped,
    onboardingDisabled,
    phase,
  ]);

  // First queue / history — thin custom events from leaf components
  useEffect(() => {
    if (!bootstrapped || onboardingDisabled) return;
    const onQueue = () => tryOffer(CONTEXTUAL_TIP_IDS.firstQueue);
    const onHistory = () => tryOffer(CONTEXTUAL_TIP_IDS.firstVersions);
    window.addEventListener("coreside:queue-has-items", onQueue);
    window.addEventListener("coreside:history-opened", onHistory);
    return () => {
      window.removeEventListener("coreside:queue-has-items", onQueue);
      window.removeEventListener("coreside:history-opened", onHistory);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bootstrapped, onboardingDisabled, phase]);

  return (
    <OverlayPortal>
      <WhatsNewBanner />
      <ContextualHint />
    </OverlayPortal>
  );
}
