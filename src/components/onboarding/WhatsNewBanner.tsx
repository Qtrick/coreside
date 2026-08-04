import { canShowWhatsNewBanner } from "@/lib/onboarding/contextual";
import {
  WHATS_NEW_ITEMS,
  WHATS_NEW_VERSION_LABEL,
} from "@/lib/onboarding/whats-new";
import {
  ESSENTIALS_TUTORIAL_ID,
  WHATS_NEW_ID,
} from "@/lib/onboarding/tutorials";
import { useOnboardingStore } from "@/stores/onboarding-store";

/** Restrained one-time upgrade banner — not a full welcome dialog. */
export function WhatsNewBanner() {
  const dismissWhatsNew = useOnboardingStore((s) => s.dismissWhatsNew);
  const visible = useOnboardingStore((s) =>
    canShowWhatsNewBanner({
      phase: s.phase,
      onboardingDisabled: s.onboardingDisabled,
      hasMeaningfulActivity: s.hasMeaningfulActivity,
      progressById: s.progressById,
      essentialsId: ESSENTIALS_TUTORIAL_ID,
      whatsNewId: WHATS_NEW_ID,
    }),
  );

  if (!visible) return null;

  return (
    <div
      className="whats-new-banner"
      role="region"
      aria-label={`What's new in ${WHATS_NEW_VERSION_LABEL}`}
    >
      <div className="whats-new-banner-body">
        <strong>What&apos;s new in {WHATS_NEW_VERSION_LABEL}</strong>
        <p>
          {WHATS_NEW_ITEMS.map((item) => item.title).join(" · ")}. Open Help
          &amp; learning for details.
        </p>
      </div>
      <button
        type="button"
        className="btn btn-ghost btn-compact"
        onClick={() => void dismissWhatsNew()}
      >
        Dismiss
      </button>
    </div>
  );
}
