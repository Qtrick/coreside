import { useAppStore } from "@/stores/app-store";

export function ModelPicker() {
  const modelCatalog = useAppStore((s) => s.modelCatalog);
  const preferredModel = useAppStore((s) => s.preferredModel);
  const setPreferredModel = useAppStore((s) => s.setPreferredModel);
  const aiStatus = useAppStore((s) => s.aiStatus);
  const sending = useAppStore((s) => s.sending);

  const needsSetup =
    aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured";

  if (!modelCatalog) return null;

  const selected = preferredModel || modelCatalog.selected || "auto";
  const options = modelCatalog.options.some((option) => option.id === selected)
    ? modelCatalog.options
    : [
        ...modelCatalog.options,
        {
          id: selected,
          label: selected,
          description: "Saved preference",
        },
      ];

  const selectedLabel =
    options.find((o) => o.id === selected)?.label ??
    (selected === "auto" ? "Auto" : selected);

  return (
    <div className="model-picker">
      <label className="visually-hidden" htmlFor="model-picker-select">
        Model
      </label>
      <select
        id="model-picker-select"
        className="model-picker-select"
        value={selected}
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
