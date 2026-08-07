#!/usr/bin/env python3
"""High-value checks for Split/Classic dock optical parity and adaptive icon sources.

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
    assert_dock_optical_parity,
    alpha_bbox,
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

    icon_json = REPO / "src-tauri" / "icons" / "Coreside.icon" / "icon.json"
    mark = REPO / "src-tauri" / "icons" / "Coreside.icon" / "Assets" / "mark.png"
    assets_car = REPO / "src-tauri" / "icons" / "Assets.car"
    tauri_conf = REPO / "src-tauri" / "tauri.conf.json"
    info_plist = REPO / "src-tauri" / "Info.plist"

    if not icon_json.is_file():
        raise SystemExit("missing Coreside.icon/icon.json")
    if not mark.is_file():
        raise SystemExit("missing Coreside.icon/Assets/mark.png")
    if not assets_car.is_file() or assets_car.stat().st_size < 10_000:
        raise SystemExit("missing or tiny Assets.car — run brand:compile-adaptive-icon")

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

    print("PASS branding icon geometry + adaptive package sources")


if __name__ == "__main__":
    main()
