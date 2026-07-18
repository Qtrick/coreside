import { Film, ImageIcon, Sparkles } from "lucide-react";
import { MediaPreview } from "@/components/media/MediaPreview";
import { formatByteSize, mediaCategoryLabel } from "@/types/media";
import type { MediaAsset } from "@/types/media";

type MediaAssetCardProps = {
  asset: MediaAsset;
  selected?: boolean;
  onSelect: (asset: MediaAsset) => void;
};

function CategoryIcon({ category }: { category: string }) {
  if (category === "video") return <Film size={14} aria-hidden />;
  if (category === "animated" || category === "animated-image")
    return <Sparkles size={14} aria-hidden />;
  return <ImageIcon size={14} aria-hidden />;
}

export function MediaAssetCard({ asset, selected, onSelect }: MediaAssetCardProps) {
  return (
    <button
      type="button"
      className={`media-asset-card${selected ? " selected" : ""}`}
      onClick={() => onSelect(asset)}
      aria-pressed={selected}
    >
      <div className="media-asset-card-preview">
        <MediaPreview asset={asset} objectFit="cover" preferThumbnail />
      </div>
      <div className="media-asset-card-meta">
        <span className="media-asset-card-title">{asset.title}</span>
        <span className="media-asset-card-sub">
          <CategoryIcon category={asset.category} />
          {mediaCategoryLabel(asset.category)} · {formatByteSize(asset.byteSize)}
        </span>
      </div>
    </button>
  );
}
