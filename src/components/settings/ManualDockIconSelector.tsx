/**
 * Dormant manual Dock icon selector (Classic Dark / Classic Light / Split).
 *
 * Not mounted while MANUAL_DOCK_ICON_SELECTION_ENABLED is false.
 * Keep this module intact so reactivation does not require rewriting the UI.
 *
 * Owns its store wiring so SettingsPanel stays free of unused Dock imports
 * while the product gate is off.
 */
import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
import dockDarkUrl from "@/assets/branding/coreside-dock-dark.png";
import dockLightUrl from "@/assets/branding/coreside-dock-light.png";
import dockSplitUrl from "@/assets/branding/coreside-dock-split.png";
import { settingsTargetDomId } from "@/lib/settings-categories";
import type { DockIconConfig } from "@/types/agent";
import {
  DEFAULT_DOCK_ICON,
  dockIconStatusLabel,
} from "@/types/agent";
import { useAppStore } from "@/stores/app-store";

function isMacOsClient(): boolean {
  if (typeof navigator === "undefined") return false;
  const platform = navigator.platform || "";
  const ua = navigator.userAgent || "";
  return /Mac|Macintosh|MacIntel|MacPPC/i.test(platform) || /Mac OS X/i.test(ua);
}

const MANUAL_CHOICES: Array<{
  config: DockIconConfig;
  label: string;
  preview: string;
}> = [
  {
    config: {
      schemaVersion: 1,
      authority: "manual",
      artwork: "classic",
      style: "dark",
    },
    label: "Classic Dark",
    preview: dockDarkUrl,
  },
  {
    config: {
      schemaVersion: 1,
      authority: "manual",
      artwork: "classic",
      style: "light",
    },
    label: "Classic Light",
    preview: dockLightUrl,
  },
  {
    config: {
      schemaVersion: 1,
      authority: "manual",
      artwork: "split",
      style: "original",
    },
    label: "Split",
    preview: dockSplitUrl,
  },
];

export function ManualDockIconSelector() {
  const {
    dockIcon,
    setDockIcon,
    dockIconPending,
    dockIconError,
    clearDockIconError,
  } = useAppStore(
    useShallow((s) => ({
      dockIcon: s.dockIcon,
      setDockIcon: s.setDockIcon,
      dockIconPending: s.dockIconPending,
      dockIconError: s.dockIconError,
      clearDockIconError: s.clearDockIconError,
    })),
  );
  const [dockRetryTarget, setDockRetryTarget] = useState<DockIconConfig | null>(
    null,
  );

  const select = (config: DockIconConfig) => {
    if (dockIconPending) return;
    setDockRetryTarget(config);
    void setDockIcon(config);
  };

  if (!isMacOsClient()) {
    return (
      <>
        <h4 className="settings-subheading" id={settingsTargetDomId("dock-icon")}>
          Dock icon
        </h4>
        <p>
          Dock icon settings are available on macOS. On this platform the setting
          is hidden.
        </p>
      </>
    );
  }

  const dockManual = dockIcon.authority === "manual";
  const isManualSelected = (config: DockIconConfig) =>
    dockIcon.authority === "manual" &&
    dockIcon.artwork === config.artwork &&
    dockIcon.style === config.style;

  return (
    <>
      <h4 className="settings-subheading" id={settingsTargetDomId("dock-icon")}>
        Dock icon
      </h4>
      <p>
        Follow macOS (Recommended) clears any temporary Dock override so the
        packaged application icon is authoritative. When the app bundle includes
        Assets.car, macOS can apply Default, Dark, Clear, or Tinted Icon &amp;
        Widget Style. In development (unpackaged), Follow macOS uses Classic Dark
        as a stand-in so the Dock does not fall back to a generic executable
        icon. Choose manually to lock Classic Dark, Classic Light, or Split while
        Coreside is open.
      </p>
      <div
        className="dock-authority-options"
        role="radiogroup"
        aria-labelledby={settingsTargetDomId("dock-icon")}
        aria-disabled={dockIconPending || undefined}
      >
        <button
          type="button"
          className="dock-authority-option"
          role="radio"
          aria-checked={dockIcon.authority === "follow_macos"}
          disabled={dockIconPending}
          onClick={() => select({ ...DEFAULT_DOCK_ICON })}
        >
          <span className="dock-authority-title">
            Follow macOS
            <span className="dock-recommended">Recommended</span>
          </span>
          <span className="dock-authority-hint">
            Uses the packaged icon; follows macOS Icon &amp; Widget Style when
            Assets.car is present
          </span>
        </button>
        <button
          type="button"
          className="dock-authority-option"
          role="radio"
          aria-checked={dockManual}
          disabled={dockIconPending}
          onClick={() =>
            select({
              schemaVersion: 1,
              authority: "manual",
              artwork: dockIcon.artwork ?? "classic",
              style:
                dockIcon.artwork === "split"
                  ? "original"
                  : dockIcon.style === "light"
                    ? "light"
                    : "dark",
            })
          }
        >
          <span className="dock-authority-title">Choose manually</span>
          <span className="dock-authority-hint">
            Classic Dark, Classic Light, or Split
          </span>
        </button>
      </div>

      <p className="dock-icon-status" aria-live="polite">
        {dockIconPending
          ? "Updating Dock icon…"
          : dockIconStatusLabel(dockIcon)}
      </p>

      {dockManual ? (
        <div
          className="dock-icon-options"
          role="radiogroup"
          aria-label="Manual Dock artwork"
        >
          {MANUAL_CHOICES.map((option) => (
            <button
              key={option.label}
              type="button"
              className="dock-icon-option"
              role="radio"
              aria-checked={isManualSelected(option.config)}
              disabled={dockIconPending}
              onClick={() => select(option.config)}
            >
              <img
                src={option.preview}
                alt=""
                className="dock-icon-preview"
                width={56}
                height={56}
              />
              <span>{option.label}</span>
            </button>
          ))}
        </div>
      ) : null}

      {dockIconError ? (
        <div className="dock-icon-error" role="alert" aria-live="assertive">
          <p>{dockIconError}</p>
          <button
            type="button"
            className="btn btn-secondary"
            disabled={dockIconPending || !dockRetryTarget}
            onClick={() => {
              if (!dockRetryTarget) return;
              clearDockIconError();
              void setDockIcon(dockRetryTarget);
            }}
          >
            Retry
          </button>
        </div>
      ) : null}

      <p className="dock-icon-footnote">
        Manual choices update the running Dock tile. Finder, Launchpad, and the
        closed app continue to use Coreside’s packaged icon.
      </p>
    </>
  );
}

/** Exported for reactivation tests / inventory — not rendered while capability is off. */
export const MANUAL_DOCK_ICON_LABELS = MANUAL_CHOICES.map((c) => c.label);
