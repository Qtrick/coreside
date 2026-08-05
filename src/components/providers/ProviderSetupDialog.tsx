import {
  useEffect,
  useId,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { Eye, EyeOff, X } from "lucide-react";
import { api } from "@/lib/tauri";
import type { ProviderHint } from "@/types/providers";
import { useAppStore } from "@/stores/app-store";
import { ModalPortal } from "@/components/ui/ModalPortal";

type Step = "pick" | "form" | "success";

type FormState = {
  label: string;
  apiKey: string;
  model: string;
  baseUrl: string;
};

const emptyForm = (): FormState => ({
  label: "",
  apiKey: "",
  model: "",
  baseUrl: "",
});

function defaultLabel(hint: ProviderHint): string {
  return hint.label;
}

function clearSensitiveForm(
  setForm: (value: FormState) => void,
  setShowKey: (value: boolean) => void,
) {
  setForm(emptyForm());
  setShowKey(false);
}

export function ProviderSetupDialog() {
  const open = useAppStore((s) => s.providerSetupOpen);
  const editing = useAppStore((s) => s.providerSetupEditing);
  const close = useAppStore((s) => s.closeProviderSetup);
  const refreshAiStatus = useAppStore((s) => s.refreshAiStatus);
  const titleId = useId();
  const descId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const keyInputRef = useRef<HTMLInputElement>(null);
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  const sessionKeyRef = useRef<string | null>(null);

  const [hints, setHints] = useState<ProviderHint[]>([]);
  const [step, setStep] = useState<Step>("pick");
  const [selected, setSelected] = useState<ProviderHint | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [showKey, setShowKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusLabel, setStatusLabel] = useState<string | null>(null);
  const [editId, setEditId] = useState<string | null>(null);

  const dismiss = () => {
    if (busy) return;
    clearSensitiveForm(setForm, setShowKey);
    close();
  };

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void api.providerKeyHints().then((rows) => {
      if (!cancelled) setHints(rows);
    });
    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (!open) {
      sessionKeyRef.current = null;
      clearSensitiveForm(setForm, setShowKey);
      setStep("pick");
      setSelected(null);
      setBusy(false);
      setError(null);
      setStatusLabel(null);
      setEditId(null);
      const previouslyFocused = previouslyFocusedRef.current;
      previouslyFocusedRef.current = null;
      if (previouslyFocused) {
        window.setTimeout(() => previouslyFocused.focus?.(), 0);
      }
      return;
    }

    if (!previouslyFocusedRef.current) {
      previouslyFocusedRef.current =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
    }

    const sessionKey = editing ? `edit:${editing.id}` : "add";
    if (sessionKeyRef.current === sessionKey) {
      // Hints arrived after open — refresh provider metadata only, keep typed key.
      if (editing && hints.length > 0) {
        const hint = hints.find((h) => h.id === editing.provider);
        if (hint) setSelected(hint);
      }
      return;
    }
    sessionKeyRef.current = sessionKey;

    if (editing) {
      const hint =
        hints.find((h) => h.id === editing.provider) ??
        ({
          id: editing.provider,
          label: editing.label,
          local: false,
          keyPlaceholder: "Enter API key",
          defaultModel: editing.modelDefault ?? "",
          docsUrl: null,
          supportsBaseUrl: editing.provider === "compatible",
          requiresApiKey: true,
          experimental: false,
        } satisfies ProviderHint);
      setSelected(hint);
      setEditId(editing.id);
      setForm({
        label: editing.label,
        apiKey: "",
        model: editing.modelDefault ?? hint.defaultModel,
        baseUrl: editing.baseUrl ?? "",
      });
      setShowKey(false);
      setBusy(false);
      setError(null);
      setStatusLabel(null);
      setStep("form");
    } else {
      setStep("pick");
      setSelected(null);
      clearSensitiveForm(setForm, setShowKey);
      setEditId(null);
      setBusy(false);
      setError(null);
      setStatusLabel(null);
    }
  }, [open, editing, hints]);

  useEffect(() => {
    if (!open) return;
    const t = window.setTimeout(() => {
      if (step === "form") {
        keyInputRef.current?.focus();
        return;
      }
      const root = dialogRef.current;
      if (!root) return;
      const focusable = root.querySelector<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      focusable?.focus();
    }, 40);
    return () => window.clearTimeout(t);
  }, [open, step, selected?.id]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        clearSensitiveForm(setForm, setShowKey);
        close();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, busy, close]);

  if (!open) return null;

  const pickProvider = (hint: ProviderHint) => {
    setSelected(hint);
    setForm({
      label: defaultLabel(hint),
      apiKey: "",
      model: hint.defaultModel,
      baseUrl: hint.defaultBaseUrl ?? "",
    });
    setShowKey(false);
    setError(null);
    setStatusLabel(null);
    setStep("form");
  };

  const onSubmit = async (event: FormEvent) => {
    event.preventDefault();
    if (!selected || busy) return;
    const apiKey = form.apiKey.trim();
    const needsKey = selected.requiresApiKey !== false && !selected.local;
    if (!editId && needsKey && !apiKey) {
      setError("Enter an API key to continue.");
      return;
    }
    if (selected.supportsBaseUrl && !form.baseUrl.trim() && !selected.defaultBaseUrl) {
      setError("Enter a base URL for this endpoint.");
      return;
    }
    if (!form.label.trim()) {
      setError("Enter a name for this connection.");
      return;
    }

    setBusy(true);
    setError(null);
    setStatusLabel(
      needsKey && apiKey
        ? "Testing connection…"
        : selected.local
          ? "Checking local server…"
          : "Saving…",
    );
    try {
      await api.upsertProviderConnection({
        id: editId,
        provider: selected.id,
        label: form.label.trim(),
        apiKey: apiKey || null,
        modelDefault: form.model.trim() || selected.defaultModel || null,
        baseUrl: selected.supportsBaseUrl
          ? form.baseUrl.trim() || selected.defaultBaseUrl || null
          : null,
        setActive: true,
      });
      // Wipe key from React state immediately after a successful save.
      clearSensitiveForm(setForm, setShowKey);
      setStatusLabel(null);
      setStep("success");
      await refreshAiStatus();
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Could not save provider connection.";
      setError(message);
      setStatusLabel(null);
    } finally {
      setBusy(false);
    }
  };

  const onDialogKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Tab" || !dialogRef.current) return;
    const focusable = Array.from(
      dialogRef.current.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((el) => !el.hasAttribute("disabled") && el.tabIndex !== -1);
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && active === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return (
    <ModalPortal>
      <div className="provider-modal-root" role="presentation">
      <button
        type="button"
        className="provider-modal-backdrop"
        aria-label="Dismiss"
        tabIndex={-1}
        disabled={busy}
        onClick={() => dismiss()}
      />
      <div
        ref={dialogRef}
        className="provider-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
        tabIndex={-1}
        onKeyDown={onDialogKeyDown}
      >
        <header className="provider-modal-header">
          <div>
            <h2 id={titleId}>
              {step === "success"
                ? "Connected"
                : editId
                  ? "Manage provider"
                  : "Connect an AI provider"}
            </h2>
            <p id={descId}>
              {step === "success"
                ? "You can start chatting right away — no restart needed."
                : "Connect a hosted provider with your own key, or Local AI without a fake secret. Keys stay in the OS keychain."}
            </p>
          </div>
          <button
            type="button"
            className="btn-icon"
            aria-label="Close"
            disabled={busy}
            onClick={() => dismiss()}
          >
            <X size={18} />
          </button>
        </header>

        {step === "pick" ? (
          <div className="provider-picker">
            <p className="provider-picker-label">Choose Hosted AI or Local AI.</p>
            <ul className="provider-picker-list">
              {hints.map((hint) => (
                <li key={hint.id}>
                  <button
                    type="button"
                    className="provider-picker-item"
                    onClick={() => pickProvider(hint)}
                  >
                    <strong>{hint.label}</strong>
                    <span className="muted">
                      {hint.local
                        ? "Local AI · no API key"
                        : hint.defaultModel || "Hosted"}
                      {hint.experimental ? " · experimental" : ""}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {step === "form" && selected ? (
          <form className="provider-key-form" onSubmit={(e) => void onSubmit(e)}>
            <button
              type="button"
              className="provider-back"
              disabled={busy || Boolean(editId)}
              onClick={() => {
                clearSensitiveForm(setForm, setShowKey);
                setStep("pick");
                setSelected(null);
                setError(null);
              }}
            >
              ← {selected.label}
            </button>

            <label className="field">
              <span>Connection name</span>
              <input
                value={form.label}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, label: e.target.value }))
                }
                autoComplete="off"
                disabled={busy}
              />
            </label>

            {selected.requiresApiKey !== false && !selected.local ? (
              <label className="field">
                <span>API key</span>
                <div className="provider-key-row">
                  <input
                    ref={keyInputRef}
                    type={showKey ? "text" : "password"}
                    value={form.apiKey}
                    onChange={(e) =>
                      setForm((prev) => ({ ...prev, apiKey: e.target.value }))
                    }
                    placeholder={
                      editId && !form.apiKey
                        ? "Leave blank to keep the saved key"
                        : selected.keyPlaceholder
                    }
                    autoComplete="off"
                    autoCorrect="off"
                    spellCheck={false}
                    disabled={busy}
                  />
                  <button
                    type="button"
                    className="btn btn-secondary"
                    aria-label={showKey ? "Hide API key" : "Show API key"}
                    onClick={() => setShowKey((v) => !v)}
                    disabled={busy}
                  >
                    {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </label>
            ) : (
              <p className="muted">
                Local AI does not need an API key. Coreside talks to the server on
                this device.
              </p>
            )}

            <label className="field">
              <span>Model</span>
              <input
                value={form.model}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, model: e.target.value }))
                }
                placeholder={selected.defaultModel || "Model id from your server"}
                autoComplete="off"
                disabled={busy}
              />
            </label>

            {selected.supportsBaseUrl ? (
              <label className="field">
                <span>Base URL</span>
                <input
                  value={form.baseUrl}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, baseUrl: e.target.value }))
                  }
                  placeholder={
                    selected.defaultBaseUrl || "https://api.example.com/v1"
                  }
                  autoComplete="off"
                  disabled={busy}
                />
              </label>
            ) : null}

            {selected.docsUrl ? (
              <p className="muted provider-docs">
                <a href={selected.docsUrl} target="_blank" rel="noreferrer">
                  {selected.local
                    ? `${selected.label} documentation`
                    : `Where to get a ${selected.label} key`}
                </a>
              </p>
            ) : null}

            {statusLabel ? (
              <p className="muted" role="status">
                {statusLabel}
              </p>
            ) : null}
            {error ? (
              <p className="provider-error" role="alert">
                {error}
              </p>
            ) : null}

            <div className="button-row provider-form-actions">
              <button
                type="button"
                className="btn btn-secondary"
                disabled={busy}
                onClick={() => dismiss()}
              >
                Cancel
              </button>
              <button type="submit" className="btn btn-primary" disabled={busy}>
                {busy ? "Working…" : "Save and test"}
              </button>
            </div>
          </form>
        ) : null}

        {step === "success" ? (
          <div className="provider-success">
            <p role="status">Provider connected and ready.</p>
            <div className="button-row">
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => dismiss()}
              >
                Start chatting
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </div>
    </ModalPortal>
  );
}
