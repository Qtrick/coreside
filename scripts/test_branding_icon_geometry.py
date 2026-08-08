#!/usr/bin/env python3
"""High-value checks for Split/Classic dock optical + flower parity and adaptive sources.

Run: python3 scripts/test_branding_icon_geometry.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "scripts"))

from generate_branding_assets import (  # noqa: E402
    RES,
    OUT,
    assert_dock_optical_parity,
    assert_flower_geometry_parity,
    alpha_bbox,
    compute_mark_placement,
    make_split_dock_icon_from_marks,
)


def main() -> None:
    dark = RES / "coreside-dock-dark.png"
    light = RES / "coreside-dock-light.png"
    split = RES / "coreside-dock-split.png"
    for path in (dark, light, split):
        if not path.is_file():
            raise SystemExit(f"missing {path}")

    assert_dock_optical_parity(dark, split)
    assert_dock_optical_parity(light, split)

    # Opaque region must be the shared ~11% outer inset (800×800 on 1024 canvas).
    expected_span = (800, 800)
    for path in (dark, light, split):
        left, top, right, bottom = alpha_bbox(path)
        span = (right - left, bottom - top)
        if abs(span[0] - expected_span[0]) > 1 or abs(span[1] - expected_span[1]) > 1:
            raise SystemExit(f"{path.name} span {span} != {expected_span} (±1)")

    # Inner flower: regenerate Split placement from marks and compare to Classic Dark.
    mark_white = OUT / "coreside-mark-white-transparent.png"
    mark_black = OUT / "coreside-mark-black-transparent.png"
    if mark_white.is_file() and mark_black.is_file():
        from PIL import Image

        inner = 1024 - 2 * int(1024 * 0.11)
        canonical, ox, oy, tw, th = compute_mark_placement(Image.open(mark_white), inner)
        outer = int(1024 * 0.11)
        classic_placement = (ox + outer, oy + outer, ox + tw + outer, oy + th + outer)
        # Determinism: writing Split again must match committed bytes' flower placement.
        tmp = RES / ".split-geometry-check.png"
        split_placement = make_split_dock_icon_from_marks(
            Image.open(mark_white), Image.open(mark_black), tmp
        )
        tmp.unlink(missing_ok=True)
        assert_flower_geometry_parity(classic_placement, split_placement)
        _ = canonical  # placement already asserted
    else:
        print("WARN: mark PNGs missing — skipped flower regeneration check")

    icon_json = REPO / "src-tauri" / "icons" / "Coreside.icon" / "icon.json"
    mark = REPO / "src-tauri" / "icons" / "Coreside.icon" / "Assets" / "mark.png"
    assets_car = REPO / "src-tauri" / "icons" / "Assets.car"
    tauri_conf = REPO / "src-tauri" / "tauri.conf.json"
    info_plist = REPO / "src-tauri" / "Info.plist"
    fingerprint = REPO / "src-tauri" / "icons" / "Assets.car.fingerprint"

    if not icon_json.is_file():
        raise SystemExit("missing Coreside.icon/icon.json")
    if not mark.is_file():
        raise SystemExit("missing Coreside.icon/Assets/mark.png")
    if not assets_car.is_file() or assets_car.stat().st_size < 10_000:
        raise SystemExit("missing or tiny Assets.car — run brand:compile-adaptive-icon")
    if not fingerprint.is_file():
        raise SystemExit(
            "missing Assets.car.fingerprint — run brand:compile-adaptive-icon"
        )

    conf = json.loads(tauri_conf.read_text())
    icons = conf.get("bundle", {}).get("icon", [])
    if "icons/Assets.car" not in icons:
        raise SystemExit("tauri.conf.json bundle.icon missing icons/Assets.car")
    plist = info_plist.read_text()
    if "<key>CFBundleIconName</key>" not in plist or "<string>Icon</string>" not in plist:
        raise SystemExit("Info.plist missing CFBundleIconName=Icon")

    doc = json.loads(icon_json.read_text())
    if "groups" not in doc or not doc["groups"]:
        raise SystemExit("Coreside.icon icon.json missing groups")

    contact = REPO / "reports" / "evidence" / "branding" / "dock-icon-contact-sheet.png"
    if not contact.is_file():
        raise SystemExit(f"missing contact sheet evidence: {contact}")

    print("PASS branding icon geometry + flower parity + adaptive package sources")


if __name__ == "__main__":
    main()
