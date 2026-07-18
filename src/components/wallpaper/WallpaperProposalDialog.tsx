import { useEffect, useId, useState } from "react";
import { X } from "lucide-react";
import type { MediaAsset } from "@/types/media";
import { ModalPortal } from "@/components/ui/ModalPortal";
import {
  buildMediaWallpaperProposal,
  schemaWallpaperToJson,
  type SchemaWallpaperConfig,
} from "@/types/wallpaper";

export type WallpaperProposalTarget = "workspace" | "project";

export type WallpaperProposalResult = {
  target: WallpaperProposalTarget;
  projectId?: string | null;
  wallpaperJson: string;
  config: SchemaWallpaperConfig;
};

type WallpaperProposalDialogProps = {
  open: boolean;
  asset: MediaAsset | null;
  target: WallpaperProposalTarget;
  projectId?: string | null;
  projectName?: string | null;
  busy?: boolean;
  onClose: () => void;
  onApply: (result: WallpaperProposalResult) => Promise<void>;
};

export function WallpaperProposalDialog({
  open,
  asset,
  target,
  projectId,
  projectName,
  busy,
  onClose,
  onApply,
}: WallpaperProposalDialogProps) {
  const titleId = useId();
  const [fit, setFit] = useState<"cover" | "contain">("cover");
  const [opacity, setOpacity] = useState(0.85);
  const [reducedMotionFallback, setReducedMotionFallback] = useState("#141714");

  useEffect(() => {
    if (!open) return;
    setFit("cover");
    setOpacity(asset?.category === "video" ? 0.9 : 0.85);
    setReducedMotionFallback("#141714");
  }, [open, asset?.category, asset?.id]);

  if (!open || !asset) return null;

  const isVideo = asset.category === "video";
  const isImage =
    asset.category === "image" ||
    asset.category === "animated" ||
    asset.category === "animated-image";

  const apply = async () => {
    const config = buildMediaWallpaperProposal({
      assetId: asset.id,
      category: asset.category,
      fit: isImage ? fit : undefined,
      opacity,
      reducedMotionFallback,
    });
    await onApply({
      target,
      projectId: projectId ?? null,
      wallpaperJson: schemaWallpaperToJson(config),
      config,
    });
  };

  const targetLabel =
    target === "project" && projectName
      ? `${projectName} wallpaper`
      : "Workspace background";

  return (
    <ModalPortal>
      <div className="provider-modal-root" role="presentation">
      <button
        type="button"
        className="provider-modal-backdrop"
        aria-label="Close"
        onClick={() => !busy && onClose()}
      />
      <div
        className="provider-modal project-dialog wallpaper-proposal-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="provider-modal-header">
          <div>
            <h2 id={titleId}>Apply wallpaper</h2>
            <p>
              Set <strong>{asset.title}</strong> as the {targetLabel}. Only
              local media library assets are used — no remote URLs.
            </p>
          </div>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close"
            disabled={busy}
            onClick={onClose}
          >
            <X size={18} />
          </button>
        </div>

        <div className="provider-modal-body">
          {isImage ? (
            <fieldset className="wallpaper-fit-options">
              <legend>Image fit</legend>
              <label>
                <input
                  type="radio"
                  name="wallpaper-fit"
                  checked={fit === "cover"}
                  disabled={busy}
                  onChange={() => setFit("cover")}
                />
                Cover
              </label>
              <label>
                <input
                  type="radio"
                  name="wallpaper-fit"
                  checked={fit === "contain"}
                  disabled={busy}
                  onChange={() => setFit("contain")}
                />
                Contain
              </label>
            </fieldset>
          ) : null}

          <label className="field">
            <span>Opacity ({Math.round(opacity * 100)}%)</span>
            <input
              type="range"
              min={0.2}
              max={1}
              step={0.05}
              value={opacity}
              disabled={busy}
              onChange={(e) => setOpacity(Number(e.target.value))}
            />
          </label>

          <label className="field">
            <span>Reduced-motion fallback color</span>
            <input
              type="color"
              value={reducedMotionFallback}
              disabled={busy}
              onChange={(e) => setReducedMotionFallback(e.target.value)}
            />
          </label>

          {isVideo ? (
            <p className="muted wallpaper-proposal-note">
              Video wallpapers play muted and loop. Motion pauses when the window
              is hidden and shows the fallback color when reduced motion is on.
            </p>
          ) : null}
        </div>

        <div className="provider-modal-footer">
          <button
            type="button"
            className="btn btn-secondary"
            disabled={busy}
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() => void apply()}
          >
            {busy ? "Applying…" : "Apply wallpaper"}
          </button>
        </div>
      </div>
    </div>
    </ModalPortal>
  );
}
