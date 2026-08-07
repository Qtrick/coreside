#!/usr/bin/env python3
"""Generate Coreside branding assets from repository-owned sources.

Produces genuine RGBA transparent marks (no baked checkerboard), Classic
Liquid-Glass-inspired dock tiles, and the manual Split dock tile.

Usage (from repo root):
  python3 scripts/generate_branding_assets.py
  python3 scripts/generate_branding_assets.py --src /path/to/logo/sources
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter


REPO = Path(__file__).resolve().parents[1]
DEFAULT_SRC = REPO / "design" / "branding" / "sources"
OUT = REPO / "src" / "assets" / "branding"
RES = REPO / "src-tauri" / "resources" / "branding"
ICONS = REPO / "src-tauri" / "icons"
SPLIT_EXPECTED_SHA256 = (
    "4ced90e8089b3248c9b978979dd22c9e8679bb6e5a27cdea7583f4047d1ea93a"
)


def soft_mask_from_luma(rgb: Image.Image, *, mark_is_light: bool) -> Image.Image:
    gray = rgb.convert("L")
    if mark_is_light:
        alpha = gray.point(lambda p: min(255, max(0, int((p / 255.0) ** 0.9 * 255))))
    else:
        inv = Image.eval(gray, lambda p: 255 - p)
        alpha = inv.point(lambda p: min(255, max(0, int((p / 255.0) ** 0.9 * 255))))
    return alpha.point(lambda p: 0 if p < 18 else (255 if p > 240 else p))


def compose_mark(
    rgb: Image.Image, alpha: Image.Image, *, force_rgb: tuple[int, int, int]
) -> Image.Image:
    out = Image.new("RGBA", rgb.size, (0, 0, 0, 0))
    color = Image.new("RGBA", rgb.size, (*force_rgb, 255))
    return Image.composite(color, out, alpha)


def trim_mark(mark: Image.Image, *, alpha_threshold: int = 12) -> Image.Image:
    """Crop transparent margins so the glyph can fill the dock tile."""
    alpha = mark.getchannel("A")
    bbox = alpha.point(lambda p: 255 if p > alpha_threshold else 0).getbbox()
    if not bbox:
        return mark
    return mark.crop(bbox)


def rounded_squircle(size: int, radius_ratio: float = 0.223) -> Image.Image:
    r = int(size * radius_ratio)
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=r, fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(radius=size * 0.004))
    return mask.point(lambda p: 255 if p > 128 else int(p * 1.2) if p > 40 else 0)


def apply_squircle_polish(
    tile: Image.Image,
    mask: Image.Image,
    inner: int,
    *,
    edge: tuple[int, int, int, int],
) -> Image.Image:
    """Shared highlight + edge ring used by Classic and Split dock tiles."""
    highlight = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    hdraw = ImageDraw.Draw(highlight)
    for i, alpha in enumerate([28, 18, 10]):
        pad = int(inner * (0.02 + i * 0.01))
        hdraw.ellipse(
            (pad, -int(inner * 0.35), inner - pad, int(inner * 0.55)),
            fill=(255, 255, 255, alpha),
        )
    highlight.putalpha(ImageChops.multiply(highlight.split()[-1], mask))
    tile = Image.alpha_composite(tile, highlight)

    ring = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    rdraw = ImageDraw.Draw(ring)
    inset = int(inner * 0.028)
    r = int(inner * 0.223) - inset // 2
    rdraw.rounded_rectangle(
        (inset, inset, inner - 1 - inset, inner - 1 - inset),
        radius=max(8, r),
        outline=edge,
        width=max(2, inner // 256),
    )
    ring.putalpha(ImageChops.multiply(ring.split()[-1], mask))
    return Image.alpha_composite(tile, ring)


def make_dock_icon(
    bg_rgb: tuple[int, int, int], mark: Image.Image, out_path: Path, size: int = 1024
) -> None:
    """Build a macOS-sized dock icon.

    The squircle is inset in a transparent canvas so the on-Dock visual size
    matches other apps. Inside the squircle the mark stays large.
    """
    # Outer transparent margin (~11%) so the tile matches native Dock icon scale.
    outer = int(size * 0.11)
    inner = size - 2 * outer

    tile = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    base = Image.new("RGBA", (inner, inner), (*bg_rgb, 255))
    mask = rounded_squircle(inner)
    tile.paste(base, (0, 0), mask)
    edge = (255, 255, 255, 36) if sum(bg_rgb) < 380 else (0, 0, 0, 22)
    tile = apply_squircle_polish(tile, mask, inner, edge=edge)

    # Fill most of the squircle: trim source margins, keep comfortable pad.
    glyph = trim_mark(mark)
    mark_pad = int(inner * 0.08)
    avail = inner - 2 * mark_pad
    # Fit glyph to the largest square that preserves aspect ratio.
    gw, gh = glyph.size
    scale = min(avail / gw, avail / gh)
    tw, th = max(1, int(gw * scale)), max(1, int(gh * scale))
    mark_resized = glyph.resize((tw, th), Image.Resampling.LANCZOS)
    ox = mark_pad + (avail - tw) // 2
    oy = mark_pad + (avail - th) // 2
    tile.alpha_composite(mark_resized, (ox, oy))

    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.alpha_composite(tile, (outer, outer))
    out_path.parent.mkdir(parents=True, exist_ok=True)
    icon.save(out_path, optimize=True)


def make_split_dock_icon(src: Image.Image, out_path: Path, size: int = 1024) -> None:
    """Build Split with the same geometry and polish as Classic.

    Root cause of the smaller Split tile: the previous path padded artwork inside
    the squircle and left a transparent ring, so the opaque region and flower
    read smaller than Classic. Fill the full squircle first (Classic does the
    same with a solid base), then apply the shared highlight and edge ring.
    """
    outer = int(size * 0.11)
    inner = size - 2 * outer
    mask = rounded_squircle(inner)

    # Full-bleed Split fill — same outer/inner canvas as Classic.
    base = (
        src.convert("RGB")
        .resize((inner, inner), Image.Resampling.LANCZOS)
        .convert("RGBA")
    )
    tile = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    tile.paste(base, (0, 0), mask)
    tile = apply_squircle_polish(tile, mask, inner, edge=(0, 0, 0, 28))

    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.alpha_composite(tile, (outer, outer))
    out_path.parent.mkdir(parents=True, exist_ok=True)
    icon.save(out_path, optimize=True)


def validate_rgba(path: Path) -> None:
    im = Image.open(path)
    if im.mode != "RGBA":
        raise SystemExit(f"{path} is {im.mode}, expected RGBA")
    for xy in [
        (0, 0),
        (im.width - 1, 0),
        (0, im.height - 1),
        (im.width - 1, im.height - 1),
    ]:
        if im.getpixel(xy)[3] != 0:
            raise SystemExit(f"{path} corner {xy} not transparent: {im.getpixel(xy)}")
    hist = im.getchannel("A").histogram()
    if hist[0] < 1000:
        raise SystemExit(f"{path} has too few transparent pixels")
    print(f"OK {path.name}: transparent corners + alpha channel")


def alpha_bbox(path: Path, *, threshold: int = 12) -> tuple[int, int, int, int]:
    im = Image.open(path)
    bbox = im.getchannel("A").point(lambda p: 255 if p > threshold else 0).getbbox()
    if not bbox:
        raise SystemExit(f"{path} has no opaque pixels")
    return bbox


def _bbox_within_tol(
    a: tuple[int, int, int, int], b: tuple[int, int, int, int], *, tol: int = 1
) -> bool:
    return all(abs(x - y) <= tol for x, y in zip(a, b, strict=True))


def assert_dock_optical_parity(
    classic_path: Path, split_path: Path, *, size: int = 1024, tol: int = 1
) -> None:
    """Split must share Classic outer geometry (same opaque squircle bounds).

    Allows ±1px tolerance so solid vs photo antialias does not flake the check.
    """
    expected_outer = int(size * 0.11)
    expected = (
        expected_outer,
        expected_outer,
        size - expected_outer,
        size - expected_outer,
    )
    classic_bbox = alpha_bbox(classic_path)
    split_bbox = alpha_bbox(split_path)
    for path, bbox in ((classic_path, classic_bbox), (split_path, split_bbox)):
        if not _bbox_within_tol(bbox, expected, tol=tol):
            raise SystemExit(
                f"{path.name} alpha bounds {bbox} != expected {expected} (±{tol})"
            )
    if not _bbox_within_tol(classic_bbox, split_bbox, tol=tol):
        raise SystemExit(
            f"Split/Classic alpha bounds diverge: classic={classic_bbox} split={split_bbox}"
        )
    print(
        f"OK optical parity: {classic_path.name} and {split_path.name} "
        f"~ {classic_bbox} (tol={tol})"
    )


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--src",
        type=Path,
        default=DEFAULT_SRC,
        help="Directory with Classic logo sources and coreside-split-logo.png",
    )
    parser.add_argument(
        "--skip-classic-sources",
        action="store_true",
        help="Only regenerate Split from design/branding/sources (Classic logos optional)",
    )
    args = parser.parse_args()
    src: Path = args.src

    OUT.mkdir(parents=True, exist_ok=True)
    RES.mkdir(parents=True, exist_ok=True)
    ICONS.mkdir(parents=True, exist_ok=True)

    split_src = src / "coreside-split-logo.png"
    if not split_src.is_file():
        raise SystemExit(f"Missing Split source: {split_src}")
    digest = sha256_file(split_src)
    if digest != SPLIT_EXPECTED_SHA256:
        raise SystemExit(
            f"Split source hash mismatch: {digest} (expected {SPLIT_EXPECTED_SHA256})"
        )

    make_split_dock_icon(Image.open(split_src), OUT / "coreside-dock-split.png")
    make_split_dock_icon(Image.open(split_src), RES / "coreside-dock-split.png")
    validate_rgba(OUT / "coreside-dock-split.png")
    validate_rgba(RES / "coreside-dock-split.png")

    classic_dark = RES / "coreside-dock-dark.png"
    if classic_dark.is_file():
        assert_dock_optical_parity(classic_dark, RES / "coreside-dock-split.png")

    if args.skip_classic_sources:
        print("Split Dock tile regenerated; Classic sources skipped.")
        return

    black_candidates = list(src.glob("Coreside_Black_Logo*.png")) + list(
        OUT.glob("coreside-icon-dark-source.png")
    )
    white_candidates = list(src.glob("Coreside_White_Logo*.png")) + list(
        OUT.glob("coreside-icon-light-source.png")
    )
    if not black_candidates or not white_candidates:
        print(
            "Classic logo sources not found under --src; "
            "Split tile updated. Pass Classic sources to regenerate Classic tiles."
        )
        return

    black_bg = Image.open(black_candidates[0]).convert("RGB")
    white_bg = Image.open(white_candidates[0]).convert("RGB")

    black_bg.save(OUT / "coreside-icon-dark-source.png")
    white_bg.save(OUT / "coreside-icon-light-source.png")

    mark_white = compose_mark(
        black_bg, soft_mask_from_luma(black_bg, mark_is_light=True), force_rgb=(255, 255, 255)
    )
    mark_black = compose_mark(
        white_bg, soft_mask_from_luma(white_bg, mark_is_light=False), force_rgb=(0, 0, 0)
    )

    for path, img in [
        (OUT / "coreside-mark-white-transparent.png", mark_white),
        (OUT / "coreside-mark-black-transparent.png", mark_black),
    ]:
        img.save(path, optimize=True)
        validate_rgba(path)

    make_dock_icon((18, 18, 18), mark_white, OUT / "coreside-dock-dark.png")
    make_dock_icon((246, 246, 244), mark_black, OUT / "coreside-dock-light.png")
    make_dock_icon((18, 18, 18), mark_white, RES / "coreside-dock-dark.png")
    make_dock_icon((246, 246, 244), mark_black, RES / "coreside-dock-light.png")
    for p in [
        OUT / "coreside-dock-dark.png",
        OUT / "coreside-dock-light.png",
        RES / "coreside-dock-dark.png",
        RES / "coreside-dock-light.png",
    ]:
        validate_rgba(p)

    assert_dock_optical_parity(RES / "coreside-dock-dark.png", RES / "coreside-dock-split.png")
    assert_dock_optical_parity(RES / "coreside-dock-light.png", RES / "coreside-dock-split.png")

    dock_dark = Image.open(OUT / "coreside-dock-dark.png")
    for name, size in {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 1024,
    }.items():
        dock_dark.resize((size, size), Image.Resampling.LANCZOS).save(
            ICONS / name, optimize=True
        )

    # Runtime resources: only Dock tiles (no design-only Split source / marks).
    for orphan in RES.glob("*"):
        if orphan.name not in {
            "coreside-dock-dark.png",
            "coreside-dock-light.png",
            "coreside-dock-split.png",
        }:
            orphan.unlink()
            print(f"removed non-runtime resource {orphan.name}")

    print("Branding assets generated.")


if __name__ == "__main__":
    main()
