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
    "0950f3dc6a812970a4a3bfc363b79baf943dcb0609a15bcf46255a02cd41127c"
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


def compute_mark_placement(
    mark: Image.Image, inner: int
) -> tuple[Image.Image, int, int, int, int]:
    """Return (resized_mark, ox, oy, tw, th) for Classic geometry."""
    glyph = trim_mark(mark)
    mark_pad = int(inner * 0.08)
    avail = inner - 2 * mark_pad
    gw, gh = glyph.size
    scale = min(avail / gw, avail / gh)
    tw, th = max(1, int(gw * scale)), max(1, int(gh * scale))
    mark_resized = glyph.resize((tw, th), Image.Resampling.LANCZOS)
    ox = mark_pad + (avail - tw) // 2
    oy = mark_pad + (avail - th) // 2
    return mark_resized, ox, oy, tw, th


def place_resized_mark(
    tile: Image.Image, mark_resized: Image.Image, ox: int, oy: int
) -> tuple[Image.Image, tuple[int, int, int, int]]:
    out = tile.copy()
    out.alpha_composite(mark_resized, (ox, oy))
    tw, th = mark_resized.size
    return out, (ox, oy, ox + tw, oy + th)


def resize_mark_to_placement(mark: Image.Image, tw: int, th: int) -> Image.Image:
    """Force a mark into the canonical placement size (shared flower geometry)."""
    return trim_mark(mark).resize((tw, th), Image.Resampling.LANCZOS)


def build_inner_tile_with_placement(
    bg_rgb: tuple[int, int, int],
    mark_resized: Image.Image,
    ox: int,
    oy: int,
    inner: int,
) -> Image.Image:
    tile = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    base = Image.new("RGBA", (inner, inner), (*bg_rgb, 255))
    mask = rounded_squircle(inner)
    tile.paste(base, (0, 0), mask)
    tile, _placement = place_resized_mark(tile, mark_resized, ox, oy)
    edge = (255, 255, 255, 36) if sum(bg_rgb) < 380 else (0, 0, 0, 22)
    return apply_squircle_polish(tile, mask, inner, edge=edge)


def make_dock_icon_pair(
    mark_white: Image.Image,
    mark_black: Image.Image,
    dark_out: Path,
    light_out: Path,
    size: int = 1024,
) -> tuple[int, int, int, int]:
    """Build Classic Dark + Light with ONE shared flower placement."""
    outer = int(size * 0.11)
    inner = size - 2 * outer
    canonical, ox, oy, tw, th = compute_mark_placement(mark_white, inner)
    light_mark = resize_mark_to_placement(mark_black, tw, th)
    dark_tile = build_inner_tile_with_placement((18, 18, 18), canonical, ox, oy, inner)
    light_tile = build_inner_tile_with_placement(
        (246, 246, 244), light_mark, ox, oy, inner
    )
    for path, tile in ((dark_out, dark_tile), (light_out, light_tile)):
        icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        icon.alpha_composite(tile, (outer, outer))
        path.parent.mkdir(parents=True, exist_ok=True)
        icon.save(path, optimize=True)
    return (ox + outer, oy + outer, ox + tw + outer, oy + th + outer)


def make_dock_icon(
    bg_rgb: tuple[int, int, int], mark: Image.Image, out_path: Path, size: int = 1024
) -> tuple[int, int, int, int]:
    """Build a single Classic dock icon (legacy single-call path)."""
    outer = int(size * 0.11)
    inner = size - 2 * outer
    mark_resized, ox, oy, tw, th = compute_mark_placement(mark, inner)
    tile = build_inner_tile_with_placement(bg_rgb, mark_resized, ox, oy, inner)
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.alpha_composite(tile, (outer, outer))
    out_path.parent.mkdir(parents=True, exist_ok=True)
    icon.save(out_path, optimize=True)
    return (ox + outer, oy + outer, ox + tw + outer, oy + th + outer)


def diagonal_split_mask(inner: int) -> Image.Image:
    """Shared diagonal: dark upper-left / light lower-right (design reference)."""
    mask = Image.new("L", (inner, inner), 0)
    draw = ImageDraw.Draw(mask)
    draw.polygon([(inner - 1, 0), (inner - 1, inner - 1), (0, inner - 1)], fill=255)
    return mask


def make_split_dock_icon_from_marks(
    mark_white: Image.Image,
    mark_black: Image.Image,
    out_path: Path,
    size: int = 1024,
) -> tuple[int, int, int, int]:
    """Build Split from canonical Classic Dark + Light flower placement.

    Flower geometry comes from the white-mark Classic placement — not from the
    flattened precomposed Split bitmap (provenance / diagonal reference only).
    """
    outer = int(size * 0.11)
    inner = size - 2 * outer
    canonical, ox, oy, tw, th = compute_mark_placement(mark_white, inner)
    light_mark = resize_mark_to_placement(mark_black, tw, th)
    squircle = rounded_squircle(inner)
    dark_base = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    light_base = Image.new("RGBA", (inner, inner), (0, 0, 0, 0))
    dark_base.paste(Image.new("RGBA", (inner, inner), (18, 18, 18, 255)), (0, 0), squircle)
    light_base.paste(
        Image.new("RGBA", (inner, inner), (246, 246, 244, 255)), (0, 0), squircle
    )
    dark_tile, _ = place_resized_mark(dark_base, canonical, ox, oy)
    light_tile, _ = place_resized_mark(light_base, light_mark, ox, oy)
    split_mask = diagonal_split_mask(inner)
    tile = Image.composite(light_tile, dark_tile, split_mask)
    tile.putalpha(ImageChops.multiply(tile.split()[-1], squircle))
    tile = apply_squircle_polish(tile, squircle, inner, edge=(0, 0, 0, 28))
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.alpha_composite(tile, (outer, outer))
    out_path.parent.mkdir(parents=True, exist_ok=True)
    icon.save(out_path, optimize=True)
    return (ox + outer, oy + outer, ox + tw + outer, oy + th + outer)


# Kept for provenance tooling — do not use as flower geometry source.
def make_split_dock_icon(src: Image.Image, out_path: Path, size: int = 1024) -> None:
    """Legacy path: full-bleed resize of precomposed Split (outer parity only).

    Prefer make_split_dock_icon_from_marks for production assets.
    """
    outer = int(size * 0.11)
    inner = size - 2 * outer
    mask = rounded_squircle(inner)
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


def assert_flower_geometry_parity(
    classic_flower: tuple[int, int, int, int],
    split_flower: tuple[int, int, int, int],
    *,
    tol: int = 2,
) -> None:
    """Inner flower bbox must match — outer tile parity alone is insufficient."""
    if not _bbox_within_tol(classic_flower, split_flower, tol=tol):
        raise SystemExit(
            f"Flower geometry diverge: classic={classic_flower} split={split_flower} (±{tol})"
        )
    cw = classic_flower[2] - classic_flower[0]
    ch = classic_flower[3] - classic_flower[1]
    sw = split_flower[2] - split_flower[0]
    sh = split_flower[3] - split_flower[1]
    if abs(cw - sw) > tol or abs(ch - sh) > tol:
        raise SystemExit(
            f"Flower size diverge: classic={cw}x{ch} split={sw}x{sh} (±{tol})"
        )
    print(
        f"OK flower parity: bbox={classic_flower} size={cw}x{ch} (tol={tol})"
    )


def write_dock_contact_sheet(
    dark: Path, light: Path, split: Path, out_path: Path
) -> None:
    """Multi-scale evidence sheet (not Settings-card UI)."""
    scales = [16, 32, 64, 128, 56, 128, 512, 1024]
    # 56 ≈ former Settings-card preview; duplicate 128 stands in for Dock-ish mid size.
    labels = ["16", "32", "64", "128", "card56", "dock128", "512", "1024"]
    gap = 24
    row_h = max(scales) + 40
    col_w = max(scales) + gap
    sheet = Image.new("RGB", (gap + col_w * len(scales), gap + row_h * 3), (40, 40, 42))
    draw = ImageDraw.Draw(sheet)
    sources = [("Classic Dark", dark), ("Classic Light", light), ("Split", split)]
    for row, (label, path) in enumerate(sources):
        src = Image.open(path).convert("RGBA")
        for col, (scale, tag) in enumerate(zip(scales, labels, strict=True)):
            thumb = src.resize((scale, scale), Image.Resampling.LANCZOS)
            x = gap + col * col_w + (max(scales) - scale) // 2
            y = gap + row * row_h + (max(scales) - scale) // 2
            # Checker underlay for transparency visibility.
            checker = Image.new("RGB", (scale, scale), (90, 90, 94))
            sheet.paste(checker, (x, y))
            sheet.paste(thumb, (x, y), thumb)
            if row == 0:
                draw.text((x, gap + row * row_h + max(scales) + 8), tag, fill=(200, 200, 200))
        draw.text((gap, gap + row * row_h - 2), label, fill=(230, 230, 230))
    out_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out_path, optimize=True)
    print(f"OK contact sheet {out_path}")


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
        help="Only regenerate Split from existing Classic marks (no Classic logo re-extract)",
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
    # Provenance check only — flower geometry no longer comes from this bitmap.
    print(f"OK Split provenance source sha256={digest[:12]}…")

    mark_white_path = OUT / "coreside-mark-white-transparent.png"
    mark_black_path = OUT / "coreside-mark-black-transparent.png"

    if not args.skip_classic_sources:
        black_candidates = list(src.glob("Coreside_Black_Logo*.png")) + list(
            OUT.glob("coreside-icon-dark-source.png")
        )
        white_candidates = list(src.glob("Coreside_White_Logo*.png")) + list(
            OUT.glob("coreside-icon-light-source.png")
        )
        if not black_candidates or not white_candidates:
            raise SystemExit(
                "Classic logo sources not found under --src; "
                "cannot build canonical Split from shared flower geometry."
            )

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
            (mark_white_path, mark_white),
            (mark_black_path, mark_black),
        ]:
            img.save(path, optimize=True)
            validate_rgba(path)
    else:
        if not mark_white_path.is_file() or not mark_black_path.is_file():
            raise SystemExit(
                "--skip-classic-sources requires existing mark PNGs under src/assets/branding"
            )
        mark_white = Image.open(mark_white_path)
        mark_black = Image.open(mark_black_path)

    if not args.skip_classic_sources:
        flower = make_dock_icon_pair(
            mark_white,
            mark_black,
            OUT / "coreside-dock-dark.png",
            OUT / "coreside-dock-light.png",
        )
        make_dock_icon_pair(
            mark_white,
            mark_black,
            RES / "coreside-dock-dark.png",
            RES / "coreside-dock-light.png",
        )
        for p in [
            OUT / "coreside-dock-dark.png",
            OUT / "coreside-dock-light.png",
            RES / "coreside-dock-dark.png",
            RES / "coreside-dock-light.png",
        ]:
            validate_rgba(p)
    else:
        outer = int(1024 * 0.11)
        inner = 1024 - 2 * outer
        _, ox, oy, tw, th = compute_mark_placement(mark_white, inner)
        flower = (ox + outer, oy + outer, ox + tw + outer, oy + th + outer)

    flower_split = make_split_dock_icon_from_marks(
        mark_white, mark_black, OUT / "coreside-dock-split.png"
    )
    make_split_dock_icon_from_marks(mark_white, mark_black, RES / "coreside-dock-split.png")
    validate_rgba(OUT / "coreside-dock-split.png")
    validate_rgba(RES / "coreside-dock-split.png")

    assert_dock_optical_parity(RES / "coreside-dock-dark.png", RES / "coreside-dock-split.png")
    assert_dock_optical_parity(RES / "coreside-dock-light.png", RES / "coreside-dock-split.png")
    assert_flower_geometry_parity(flower, flower_split)

    if not args.skip_classic_sources:
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

    evidence = REPO / "reports" / "evidence" / "branding" / "dock-icon-contact-sheet.png"
    write_dock_contact_sheet(
        OUT / "coreside-dock-dark.png",
        OUT / "coreside-dock-light.png",
        OUT / "coreside-dock-split.png",
        evidence,
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
