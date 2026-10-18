import { useCallback, useEffect, useId, useRef, useState } from "react";
import { MoreHorizontal, X } from "lucide-react";
import { ModalPortal } from "@/components/ui/ModalPortal";
import { api, listenThemePresetsChanged } from "@/lib/tauri";
import { TauriCommandError } from "@/lib/tauri/errors";
import { useAppStore } from "@/stores/app-store";
import {
  THEME_PRESET_DESCRIPTION_MAX,
  THEME_PRESET_NAME_MAX,
  wallpaperLabelFromSnapshot,
  type ThemePresetCapabilities,
  type ThemePresetRecord,
} from "@/types/theme-preset";

function formatUpdatedAt(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function ThemePreviewSwatch({ preset }: { preset: ThemePresetRecord }) {
  const snap = preset.snapshot;
  const isDark =
    snap.themePreference === "dark" ||
    (snap.themePreference === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  const bg = isDark ? snap.backgroundDark : snap.backgroundLight;
  const surface = isDark ? snap.surfaceDark : snap.surfaceLight;
  const accent = isDark ? snap.accentPrimaryDark : snap.accentPrimaryLight;
  const border = isDark ? snap.borderDark : snap.borderLight;
  return (
    <div
      className="theme-preset-swatch"
      style={{ background: bg, borderColor: border }}
      aria-hidden="true"
    >
      <div className="theme-preset-swatch-surface" style={{ background: surface }} />
      <div className="theme-preset-swatch-accent" style={{ background: accent }} />
    </div>
  );
}

export function ThemePresetsSettings() {
  const applyThemePreset = useAppStore((s) => s.applyThemePreset);
  const projectWallpaperActive = useAppStore((s) =>
    Boolean(s.activeProject?.wallpaperJson?.trim()),
  );

  const [presets, setPresets] = useState<ThemePresetRecord[]>([]);
  const [caps, setCaps] = useState<ThemePresetCapabilities | null>(null);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");
  const [saveDescription, setSaveDescription] = useState("");
  const [saveBusy, setSaveBusy] = useState(false);

  const [applyTarget, setApplyTarget] = useState<ThemePresetRecord | null>(null);
  const [applyFingerprint, setApplyFingerprint] = useState<string | null>(null);
  const [applyBusy, setApplyBusy] = useState(false);
  const applyInFlightRef = useRef(false);
  const applyDialogRequestRef = useRef(0);

  const [renameTarget, setRenameTarget] = useState<ThemePresetRecord | null>(null);
  const [renameName, setRenameName] = useState("");
  const [renameBusy, setRenameBusy] = useState(false);

  const [deleteTarget, setDeleteTarget] = useState<ThemePresetRecord | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);

  const [updateTarget, setUpdateTarget] = useState<ThemePresetRecord | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);

  const [menuId, setMenuId] = useState<string | null>(null);
  const presetRequestRef = useRef(0);
  const capabilitiesRequestRef = useRef(0);

  const saveTitleId = useId();
  const applyTitleId = useId();
  const applyDescId = useId();
  const renameTitleId = useId();
  const deleteTitleId = useId();
  const updateTitleId = useId();

  const refreshPresets = useCallback(async () => {
    const request = ++presetRequestRef.current;
    try {
      const list = await api.listThemePresets();
      if (request !== presetRequestRef.current) return;
      setPresets(list);
      setError(null);
    } catch (err) {
      if (request !== presetRequestRef.current) return;
      setError(
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Could not load saved themes",
      );
    } finally {
      if (request === presetRequestRef.current) setLoading(false);
    }
  }, []);

  const refreshCapabilities = useCallback(async () => {
    const request = ++capabilitiesRequestRef.current;
    try {
      const next = await api.getThemePresetCapabilities();
      if (request === capabilitiesRequestRef.current) setCaps(next);
    } catch {
      // Local preset listing remains usable while entitlement lookup is offline.
      if (request === capabilitiesRequestRef.current) setCaps(null);
    }
  }, []);

  useEffect(() => {
    void refreshPresets();
    void refreshCapabilities();
  }, [refreshCapabilities, refreshPresets]);

  useEffect(() => {
    let unlistenPresets: (() => void) | undefined;
    let cancelled = false;
    void listenThemePresetsChanged(() => {
      if (!cancelled) {
        void refreshPresets();
        void refreshCapabilities();
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlistenPresets = fn;
    }).catch(() => undefined);
    return () => {
      cancelled = true;
      applyDialogRequestRef.current += 1;
      unlistenPresets?.();
    };
  }, [refreshCapabilities, refreshPresets]);

  const dismissApplyDialog = () => {
    if (applyBusy) return;
    applyDialogRequestRef.current += 1;
    setApplyTarget(null);
    setApplyFingerprint(null);
  };

  const openApplyDialog = async (preset: ThemePresetRecord) => {
    const request = ++applyDialogRequestRef.current;
    setError(null);
    setStatus(null);
    try {
      const fingerprint = await api.getAppearanceFingerprint();
      if (request !== applyDialogRequestRef.current) return;
      setApplyFingerprint(fingerprint);
      setApplyTarget(preset);
    } catch (err) {
      if (request !== applyDialogRequestRef.current) return;
      setError(
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Could not prepare Apply confirmation",
      );
    }
  };

  const openSave = () => {
    setSaveName("");
    setSaveDescription("");
    setSaveOpen(true);
    setError(null);
    setStatus(null);
  };

  const submitSave = async () => {
    const name = saveName.trim();
    if (!name) {
      setError("Theme name is required");
      return;
    }
    if (caps && !caps.canCreate) {
      setError(
        `You've reached the limit of ${caps.limit} saved themes. Delete or update an existing theme.`,
      );
      return;
    }
    setSaveBusy(true);
    setError(null);
    try {
      await api.saveCurrentThemePreset({
        name,
        description: saveDescription.trim(),
      });
      setSaveOpen(false);
      setStatus(`Saved “${name}”.`);
      await refreshPresets();
      void refreshCapabilities();
    } catch (err) {
      setError(
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Couldn't save this theme",
      );
    } finally {
      setSaveBusy(false);
    }
  };

  const confirmApply = async () => {
    if (!applyTarget || !applyFingerprint || applyInFlightRef.current) return;
    applyInFlightRef.current = true;
    setApplyBusy(true);
    setError(null);
    try {
      await applyThemePreset(
        applyTarget.id,
        applyTarget.revision,
        applyFingerprint,
      );
      setStatus(`Applied “${applyTarget.name}”.`);
      setApplyTarget(null);
      setApplyFingerprint(null);
    } catch (err) {
      const message =
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Couldn't apply this theme. Your current appearance was not changed.";
      if (
        err instanceof TauriCommandError &&
        err.code === "conflict" &&
        message.includes("appearance changed")
      ) {
        try {
          const nextFingerprint = await api.getAppearanceFingerprint();
          setApplyFingerprint(nextFingerprint);
        } catch {
          setApplyFingerprint(null);
        }
      }
      setError(message);
    } finally {
      applyInFlightRef.current = false;
      setApplyBusy(false);
    }
  };

  const submitRename = async () => {
    if (!renameTarget) return;
    const name = renameName.trim();
    if (!name) {
      setError("Theme name is required");
      return;
    }
    setRenameBusy(true);
    try {
      await api.renameThemePreset({
        id: renameTarget.id,
        name,
        expectedRevision: renameTarget.revision,
      });
      setRenameTarget(null);
      setStatus(`Renamed to “${name}”.`);
      await refreshPresets();
      void refreshCapabilities();
    } catch (err) {
      setError(
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Couldn't rename this theme",
      );
    } finally {
      setRenameBusy(false);
    }
  };

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    setDeleteBusy(true);
    try {
      await api.deleteThemePreset({
        id: deleteTarget.id,
        expectedRevision: deleteTarget.revision,
      });
      setDeleteTarget(null);
      setStatus(`Deleted “${deleteTarget.name}”.`);
      await refreshPresets();
      void refreshCapabilities();
    } catch (err) {
      setError(
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Couldn't delete this theme",
      );
    } finally {
      setDeleteBusy(false);
    }
  };

  const confirmUpdate = async () => {
    if (!updateTarget) return;
    setUpdateBusy(true);
    try {
      await api.updateThemePresetFromCurrent({
        id: updateTarget.id,
        expectedRevision: updateTarget.revision,
      });
      setUpdateTarget(null);
      setStatus(`Updated “${updateTarget.name}” from your current theme.`);
      await refreshPresets();
      void refreshCapabilities();
    } catch (err) {
      setError(
        err instanceof TauriCommandError || err instanceof Error
          ? err.message
          : "Couldn't update this theme",
      );
    } finally {
      setUpdateBusy(false);
    }
  };

  const used = caps?.used ?? presets.length;
  const limit = caps?.limit ?? 10;
  const atLimit = !(caps?.canCreate ?? used < limit);
  const dialogOpen = Boolean(
    saveOpen || applyTarget || renameTarget || deleteTarget || updateTarget,
  );

  useEffect(() => {
    if (!menuId || dialogOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setMenuId(null);
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (target.closest(".theme-preset-menu")) return;
      setMenuId(null);
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [dialogOpen, menuId]);

  return (
    <section
      className="settings-section"
      aria-labelledby="settings-target-saved-themes"
    >
      <h3 id="settings-target-saved-themes">Saved Themes</h3>
      <p>
        Save your wallpaper, colors, and other appearance settings so you can
        switch back to them later.
      </p>
      {projectWallpaperActive ? (
        <p className="muted theme-preset-hint" role="note">
          This project has its own wallpaper. Saving captures the global
          workspace theme; applying a theme does not change the project
          wallpaper.
        </p>
      ) : null}
      <div className="theme-preset-toolbar">
        <p className="muted" aria-live="polite">
          {used} of {limit} saved
        </p>
        <button
          type="button"
          className="btn btn-primary"
          onClick={openSave}
          disabled={atLimit || loading}
        >
          Save Current Theme
        </button>
      </div>
      {atLimit ? (
        <p className="muted" role="status">
          Limit reached. You can still apply, rename, update, or delete existing
          themes.
        </p>
      ) : null}
      {status ? (
        <p className="theme-preset-status" role="status">
          {status}
        </p>
      ) : null}
      {error && !dialogOpen ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}

      {loading ? <p className="muted">Loading saved themes…</p> : null}
      {!loading && presets.length === 0 ? (
        <p className="muted">No saved themes yet.</p>
      ) : null}

      <ul className="theme-preset-list">
        {presets.map((preset) => (
          <li key={preset.id} className="theme-preset-card">
            <ThemePreviewSwatch preset={preset} />
            <div className="theme-preset-card-body">
              <div className="theme-preset-card-title">{preset.name}</div>
              <div className="muted theme-preset-card-meta">
                {preset.snapshot.themePreference} ·{" "}
                {wallpaperLabelFromSnapshot(preset.snapshot)}
                {preset.updatedAt
                  ? ` · Updated ${formatUpdatedAt(preset.updatedAt)}`
                  : ""}
              </div>
              <div className="theme-preset-card-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void openApplyDialog(preset)}
                >
                  Apply Theme
                </button>
                <div className="theme-preset-menu">
                  <button
                    type="button"
                    className="btn-icon"
                    aria-label={`More actions for ${preset.name}`}
                    aria-expanded={menuId === preset.id}
                    onClick={() =>
                      setMenuId((id) => (id === preset.id ? null : preset.id))
                    }
                  >
                    <MoreHorizontal size={16} />
                  </button>
                  {menuId === preset.id ? (
                    <div className="theme-preset-menu-panel" role="menu">
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => {
                          setMenuId(null);
                          setError(null);
                          setRenameTarget(preset);
                          setRenameName(preset.name);
                        }}
                      >
                        Rename
                      </button>
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => {
                          setMenuId(null);
                          setError(null);
                          setUpdateTarget(preset);
                        }}
                      >
                        Update from Current Theme
                      </button>
                      <button
                        type="button"
                        role="menuitem"
                        className="danger"
                        onClick={() => {
                          setMenuId(null);
                          setError(null);
                          setDeleteTarget(preset);
                        }}
                      >
                        Delete
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          </li>
        ))}
      </ul>

      {saveOpen ? (
        <ModalPortal onClose={() => !saveBusy && setSaveOpen(false)}>
          <div className="provider-modal-root" role="presentation">
            <button
              type="button"
              className="provider-modal-backdrop"
              aria-label="Close"
              onClick={() => !saveBusy && setSaveOpen(false)}
            />
            <div
              className="provider-modal project-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby={saveTitleId}
            >
              <div className="provider-modal-header">
                <div>
                  <h2 id={saveTitleId}>Save Current Theme</h2>
                  <p>
                    Save your wallpaper, colors, and other appearance settings so
                    you can switch back to them later.
                  </p>
                </div>
                <button
                  type="button"
                  className="btn-icon"
                  aria-label="Close"
                  disabled={saveBusy}
                  onClick={() => setSaveOpen(false)}
                >
                  <X size={16} />
                </button>
              </div>
              <div className="project-form">
                <label>
                  Theme name
                  <input
                    type="text"
                    value={saveName}
                    maxLength={THEME_PRESET_NAME_MAX}
                    autoFocus
                    onChange={(e) => setSaveName(e.target.value)}
                    disabled={saveBusy}
                  />
                </label>
                <label>
                  Description (optional)
                  <textarea
                    value={saveDescription}
                    maxLength={THEME_PRESET_DESCRIPTION_MAX}
                    rows={2}
                    onChange={(e) => setSaveDescription(e.target.value)}
                    disabled={saveBusy}
                  />
                </label>
                {error ? (
                  <p className="form-error" role="alert">
                    {error}
                  </p>
                ) : null}
                <div className="dialog-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={saveBusy}
                    onClick={() => setSaveOpen(false)}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={saveBusy || !saveName.trim()}
                    onClick={() => void submitSave()}
                  >
                    Save Theme
                  </button>
                </div>
              </div>
            </div>
          </div>
        </ModalPortal>
      ) : null}

      {applyTarget ? (
        <ModalPortal onClose={dismissApplyDialog}>
          <div className="provider-modal-root" role="presentation">
            <button
              type="button"
              className="provider-modal-backdrop"
              aria-label="Close"
              onClick={dismissApplyDialog}
            />
            <div
              className="provider-modal project-dialog"
              role="alertdialog"
              aria-modal="true"
              aria-labelledby={applyTitleId}
              aria-describedby={applyDescId}
            >
              <div className="provider-modal-header">
                <div>
                  <h2 id={applyTitleId}>Apply “{applyTarget.name}”?</h2>
                  <p id={applyDescId}>
                    Your current theme will be overwritten with the settings
                    saved in this preset.
                  </p>
                  <p className="muted">
                    Tip: You can save your current theme before switching so you
                    can return to it later.
                  </p>
                </div>
                <button
                  type="button"
                  className="btn-icon"
                  aria-label="Close"
                  disabled={applyBusy}
                  onClick={dismissApplyDialog}
                >
                  <X size={16} />
                </button>
              </div>
              <div className="theme-preset-apply-preview">
                <ThemePreviewSwatch preset={applyTarget} />
              </div>
              {error ? (
                <p className="form-error" role="alert">
                  {error}
                </p>
              ) : null}
              <div className="dialog-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={applyBusy}
                  autoFocus
                  onClick={dismissApplyDialog}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={applyBusy || !applyFingerprint}
                  onClick={() => void confirmApply()}
                >
                  {applyBusy ? "Applying…" : "Apply Theme"}
                </button>
              </div>
            </div>
          </div>
        </ModalPortal>
      ) : null}

      {renameTarget ? (
        <ModalPortal onClose={() => !renameBusy && setRenameTarget(null)}>
          <div className="provider-modal-root" role="presentation">
            <button
              type="button"
              className="provider-modal-backdrop"
              aria-label="Close"
              onClick={() => !renameBusy && setRenameTarget(null)}
            />
            <div
              className="provider-modal project-dialog"
              role="dialog"
              aria-modal="true"
              aria-labelledby={renameTitleId}
            >
              <div className="provider-modal-header">
                <h2 id={renameTitleId}>Rename theme</h2>
                <button
                  type="button"
                  className="btn-icon"
                  aria-label="Close"
                  disabled={renameBusy}
                  onClick={() => setRenameTarget(null)}
                >
                  <X size={16} />
                </button>
              </div>
              <div className="project-form">
                <label>
                  Theme name
                  <input
                    type="text"
                    value={renameName}
                    maxLength={THEME_PRESET_NAME_MAX}
                    autoFocus
                    onChange={(e) => setRenameName(e.target.value)}
                    disabled={renameBusy}
                  />
                </label>
                {error ? (
                  <p className="form-error" role="alert">
                    {error}
                  </p>
                ) : null}
                <div className="dialog-actions">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={renameBusy}
                    onClick={() => setRenameTarget(null)}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={renameBusy || !renameName.trim()}
                    onClick={() => void submitRename()}
                  >
                    Rename
                  </button>
                </div>
              </div>
            </div>
          </div>
        </ModalPortal>
      ) : null}

      {updateTarget ? (
        <ModalPortal onClose={() => !updateBusy && setUpdateTarget(null)}>
          <div className="provider-modal-root" role="presentation">
            <button
              type="button"
              className="provider-modal-backdrop"
              aria-label="Close"
              onClick={() => !updateBusy && setUpdateTarget(null)}
            />
            <div
              className="provider-modal project-dialog"
              role="alertdialog"
              aria-modal="true"
              aria-labelledby={updateTitleId}
            >
              <div className="provider-modal-header">
                <div>
                  <h2 id={updateTitleId}>
                    Update “{updateTarget.name}” from current theme?
                  </h2>
                  <p>
                    The saved preset will be replaced with your current
                    appearance settings. The theme name stays the same.
                  </p>
                </div>
                <button
                  type="button"
                  className="btn-icon"
                  aria-label="Close"
                  disabled={updateBusy}
                  onClick={() => setUpdateTarget(null)}
                >
                  <X size={16} />
                </button>
              </div>
              {error ? (
                <p className="form-error" role="alert">
                  {error}
                </p>
              ) : null}
              <div className="dialog-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={updateBusy}
                  onClick={() => setUpdateTarget(null)}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={updateBusy}
                  onClick={() => void confirmUpdate()}
                >
                  {updateBusy ? "Updating…" : "Update Theme"}
                </button>
              </div>
            </div>
          </div>
        </ModalPortal>
      ) : null}

      {deleteTarget ? (
        <ModalPortal onClose={() => !deleteBusy && setDeleteTarget(null)}>
          <div className="provider-modal-root" role="presentation">
            <button
              type="button"
              className="provider-modal-backdrop"
              aria-label="Close"
              onClick={() => !deleteBusy && setDeleteTarget(null)}
            />
            <div
              className="provider-modal project-dialog"
              role="alertdialog"
              aria-modal="true"
              aria-labelledby={deleteTitleId}
            >
              <div className="provider-modal-header">
                <div>
                  <h2 id={deleteTitleId}>Delete “{deleteTarget.name}”?</h2>
                  <p>
                    This removes the saved preset only. Your current appearance
                    will not change.
                  </p>
                </div>
                <button
                  type="button"
                  className="btn-icon"
                  aria-label="Close"
                  disabled={deleteBusy}
                  onClick={() => setDeleteTarget(null)}
                >
                  <X size={16} />
                </button>
              </div>
              {error ? (
                <p className="form-error" role="alert">
                  {error}
                </p>
              ) : null}
              <div className="dialog-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  disabled={deleteBusy}
                  onClick={() => setDeleteTarget(null)}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="btn btn-danger"
                  disabled={deleteBusy}
                  onClick={() => void confirmDelete()}
                >
                  {deleteBusy ? "Deleting…" : "Delete"}
                </button>
              </div>
            </div>
          </div>
        </ModalPortal>
      ) : null}
    </section>
  );
}
