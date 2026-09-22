import { useAppStore } from "@/stores/app-store";

export function ModelPicker() {
  const modelCatalog = useAppStore((s) => s.modelCatalog);
  const preferredModel = useAppStore((s) => s.preferredModel);
  const setPreferredModel = useAppStore((s) => s.setPreferredModel);
  const aiStatus = useAppStore((s) => s.aiStatus);
  const sending = useAppStore((s) => s.sending);

  const needsSetup =
    aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured";
  const showModelIdentity = Boolean(aiStatus?.disclosure?.showModelIdentity);
  const allowModelSelection = aiStatus?.disclosure?.allowModelSelection !== false;

  const staticAutoPicker = (
    <div className="model-picker">
      <span className="model-picker-select" aria-label="AI model">
        Auto
      </span>
    </div>
  );

  // Hosted / development-supplied access: keep Auto without exposing raw slugs.
  if (!showModelIdentity || !allowModelSelection) {
    return staticAutoPicker;
  }

  // Catalog may still be loading after BYOK connect or Developer Mode toggle.
  if (!modelCatalog) {
    return staticAutoPicker;
  }

  const selected = preferredModel || modelCatalog.selected || "auto";
  // Only show models that belong to the current provider's catalog.
  // Stale preferences from a previous provider are silently resolved to Auto
  // rather than appearing as a misleading "Saved preference" option.
  const options = modelCatalog.options;
  const effectiveSelected = options.some((option) => option.id === selected)
    ? selected
    : "auto";

  const selectedLabel =
    options.find((o) => o.id === effectiveSelected)?.label ??
    (effectiveSelected === "auto" ? "Auto" : effectiveSelected);

  return (
    <div className="model-picker">
      <label className="visually-hidden" htmlFor="model-picker-select">
        Model
      </label>
      <select
        id="model-picker-select"
        className="model-picker-select"
        value={effectiveSelected}
        disabled={sending || needsSetup}
        onChange={(event) => void setPreferredModel(event.target.value)}
        aria-label="Select AI model"
        title={selectedLabel}
      >
        {options.map((option) => (
          <option key={option.id} value={option.id}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}
