#!/usr/bin/env python3
"""Deterministic generator for app/assets/placeholder/ (see docs/art-inventory.md).

Stdlib only. Usage: python3 scripts/gen_placeholder_art.py [out_dir]
Re-running overwrites byte-identically.
"""
import math
import struct
import sys
import zlib
from pathlib import Path

PAPER, INK, SEA, EDIT, FORK = "#EDEFEA", "#1C292F", "#BCD1DB", "#AE2459", "#5E45A3"
OK, WARN, CRISIS = "#4E8F68", "#B8862B", "#C0463E"

out = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "app/assets/placeholder")


def write(rel: str, text: str) -> None:
    p = out / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text if text.endswith("\n") else text + "\n")


def svg(w: int, h: int, body: str, vb: str | None = None) -> str:
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
            f'viewBox="{vb or f"0 0 {w} {h}"}">\n{body}\n</svg>')


# --- icons: 24x24 grid, 1.5 stroke, custom glyph set (Lucide supplies the rest) ---
ICONS: dict[str, str] = {
    "edit-stamp": '<rect x="4" y="15" width="16" height="5"/><path d="M12 15v-4M8 11h8l-1-6h-6z"/>',
    "fork": '<circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="8" r="2"/><path d="M6 7v10M18 10c0 4-12 3-12 7"/>',
    "branch-god": '<circle cx="12" cy="12" r="3"/><path d="M12 2v4M12 18v4M2 12h4M18 12h4"/>',
    "seek-cursor": '<path d="M12 3v18M6 8l-4 4 4 4M18 8l4 4-4 4"/>',
    "run-to-tick": '<path d="M5 5l9 7-9 7zM18 5v14"/>',
    "pause": '<path d="M8 5v14M16 5v14"/>',
    "play": '<path d="M7 4l12 8-12 8z"/>',
    "speed-1": '<path d="M5 5l7 7-7 7"/>',
    "speed-2": '<path d="M4 5l7 7-7 7M12 5l7 7-7 7"/>',
    "speed-3": '<path d="M2 5l6 7-6 7M9 5l6 7-6 7M16 5l6 7-6 7"/>',
    "hash-seal": '<circle cx="12" cy="12" r="9"/><path d="M9 7l-1 10M15 7l-1 10M6 10h12M5 14h12"/>',
    "halt": '<path d="M8 3h8l5 5v8l-5 5H8l-5-5V8z"/><path d="M12 8v5M12 16v.5"/>',
    "report-problem": '<path d="M5 21V4h11l-1 4 1 4H5"/>',
    "drop-zone": '<rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="3 3"/><path d="M12 8v8M8 12l4 4 4-4"/>',
    "fetch-source": '<path d="M12 4v10M8 10l4 4 4-4M4 19h16"/>',
    "disk": '<ellipse cx="12" cy="6" rx="8" ry="3"/><path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6"/>',
    "ok": '<path d="M5 12l5 5 9-10"/>',
    "warn": '<path d="M12 3l10 18H2zM12 10v5M12 18v.5"/>',
    "crisis": '<path d="M13 2L5 13h6l-1 9 8-12h-6z"/>',
    "ensemble-fan": '<path d="M3 12h4c6 0 8-6 14-8M7 12c6 0 8 0 14 0M7 12c6 0 8 6 14 8"/>',
    "paired-effect": '<path d="M4 8h16M4 16h16"/><circle cx="9" cy="8" r="2"/><circle cx="15" cy="16" r="2"/>',
    "diff-map": '<path d="M3 5l6-2 6 2 6-2v16l-6 2-6-2-6 2z"/><path d="M9 3v16M15 5v16"/>',
    "freight-lane": '<path d="M3 17c4-8 14-8 18 0M3 17l2-1M21 17l-2-1"/><circle cx="12" cy="10" r="1"/>',
    "price": '<path d="M4 4h8l8 8-8 8-8-8z"/><circle cx="8.5" cy="8.5" r="1"/>',
    "output": '<path d="M3 21h18M6 21V11M11 21V5M16 21v-8"/>',
    "political": '<path d="M5 3v18M5 4h13l-3 4 3 4H5"/>',
    "why": '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 114 2c-1 .8-1.5 1.2-1.5 2.5M12 17v.5"/>',
    "scenario": '<rect x="4" y="3" width="16" height="18" rx="1"/><path d="M8 8h8M8 12h8M8 16h5"/>',
    "lesson": '<path d="M2 8l10-4 10 4-10 4zM6 10v5c0 1.5 3 3 6 3s6-1.5 6-3v-5"/>',
    "settings": '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2"/>',
    "language": '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"/>',
    "accessibility": '<circle cx="12" cy="4.5" r="1.5"/><path d="M5 8h14M12 8v6M9 21l3-7 3 7"/>',
}
for name, body in ICONS.items():
    write(f"icons/{name}.svg", svg(24, 24, f'<g fill="none" stroke="{INK}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{body}</g>'))

# --- app icon: globe with graticule, tilted meridian, "c" mark; 1024 and mono 512 ---
def graticule(cx: float, cy: float, r: float, stroke: str, sw: float) -> str:
    parts = []
    for k in (-0.5, 0, 0.5):
        y = cy + r * k
        half = math.sqrt(max(r * r - (r * k) ** 2, 0))
        parts.append(f'<path d="M{cx-half:.1f} {y:.1f}H{cx+half:.1f}" stroke="{stroke}" stroke-width="{sw}"/>')
    for f in (0.35, 0.7):
        parts.append(f'<ellipse cx="{cx}" cy="{cy}" rx="{r*f:.1f}" ry="{r}" fill="none" stroke="{stroke}" stroke-width="{sw}"/>')
    return "".join(parts)

write("app-icon/app-icon-1024.svg", svg(1024, 1024,
    f'<rect width="1024" height="1024" rx="228" fill="{INK}"/>'
    f'<circle cx="512" cy="512" r="340" fill="{SEA}"/>'
    f'<g fill="none">{graticule(512, 512, 340, INK, 6)}</g>'
    f'<path d="M512 172A340 340 0 0 1 852 512" fill="none" stroke="{EDIT}" stroke-width="18"/>'
    f'<path d="M512 852A340 340 0 0 1 172 512" fill="none" stroke="{FORK}" stroke-width="18"/>'
    f'<circle cx="512" cy="512" r="340" fill="none" stroke="{PAPER}" stroke-width="10"/>'))
write("app-icon/app-icon-mono-512.svg", svg(512, 512,
    f'<g fill="none" stroke="{INK}" stroke-width="10"><circle cx="256" cy="256" r="200"/>{graticule(256, 256, 200, INK, 6)}</g>'))
write("app-icon/dock-badge-template.svg", svg(1024, 1024,
    f'<rect x="100" y="100" width="824" height="824" rx="185" fill="none" stroke="{INK}" stroke-width="4" stroke-dasharray="16 12"/>'
    f'<text x="512" y="520" text-anchor="middle" font-family="monospace" font-size="40" fill="{INK}">macOS safe area 824 px (squircle in 1024)</text>'))

# --- first-run / splash ---
write("splash/splash-1600x900.svg", svg(1600, 900,
    f'<rect width="1600" height="900" fill="{PAPER}"/>'
    f'<g fill="none" stroke="{INK}" stroke-opacity=".18" stroke-width="1">'
    + "".join(f'<path d="M0 {y}H1600"/>' for y in range(90, 900, 90))
    + "".join(f'<path d="M{x} 0V900"/>' for x in range(160, 1600, 160)) + '</g>'
    f'<circle cx="1100" cy="450" r="300" fill="{SEA}"/>'
    f'<g fill="none">{graticule(1100, 450, 300, INK, 2)}</g>'
    f'<text x="140" y="430" font-family="serif" font-size="88" fill="{INK}">Ceteris Paribus</text>'
    f'<text x="144" y="490" font-family="monospace" font-size="22" fill="{INK}" fill-opacity=".7">PLACEHOLDER SPLASH</text>'))
write("splash/first-run-illustration.svg", svg(640, 360,
    f'<rect width="640" height="360" fill="{PAPER}"/>'
    f'<rect x="20" y="20" width="600" height="320" rx="6" fill="none" stroke="{INK}" stroke-dasharray="8 6"/>'
    f'<path d="M320 120v100M280 180l40 40 40-40" fill="none" stroke="{INK}" stroke-width="4"/>'
    f'<text x="320" y="290" text-anchor="middle" font-family="monospace" font-size="18" fill="{INK}">drop source files here (placeholder)</text>'))

# --- map: swatches, ramps, texture legends ---
def strip(name: str, colours: list[str]) -> None:
    w = 40
    write(f"map/{name}.svg", svg(w * len(colours), 40,
        "".join(f'<rect x="{i*w}" width="{w}" height="40" fill="{c}"/>' for i, c in enumerate(colours))))

strip("ramp-sequential", ["#EDEFEA", "#CFDCD5", "#A5C4BE", "#72A5A5", "#43838F", "#1F5F78", "#143F5C"])
strip("ramp-diverging", ["#8C2D5E", "#B8689A", "#DCA9C6", "#EDEFEA", "#A9C3B0", "#5E9A7A", "#2C6A4F"])
strip("bathymetry-bands", ["#DCE8EE", "#CBDDE6", "#BCD1DB", "#A9C3D0", "#95B3C3", "#82A3B7"])

# --- PNG helpers (stdlib) ---
def png(path: Path, w: int, h: int, px) -> None:
    raw = b"".join(b"\x00" + b"".join(bytes(px(x, y)) for x in range(w)) for y in range(h))
    def chunk(t: bytes, d: bytes) -> bytes:
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

def height(x: float, y: float) -> float:
    return (math.sin(x * 0.045) * math.cos(y * 0.037) + 0.5 * math.sin(x * 0.11 + y * 0.09)
            + 0.25 * math.sin(x * 0.23 - y * 0.19))

def shade(x: int, y: int) -> list[int]:
    # multidirectional-ish hillshade, light from NW and N
    dx = height(x + 1, y) - height(x - 1, y)
    dy = height(x, y + 1) - height(x, y - 1)
    v = 0.5 + 0.35 * (-dx - dy) + 0.15 * (-dy)
    g = max(0, min(255, int(v * 255)))
    return [g, g, g]

png(out / "map/hillshade-tile-256.png", 256, 256, shade)

def bathy(x: int, y: int) -> list[int]:
    d = (height(x, y) + 1.75) / 3.5
    bands = ["#DCE8EE", "#CBDDE6", "#BCD1DB", "#A9C3D0", "#95B3C3", "#82A3B7"]
    c = bands[min(len(bands) - 1, int(d * len(bands)))]
    return [int(c[i:i + 2], 16) for i in (1, 3, 5)]

png(out / "map/bathymetry-tile-256.png", 256, 256, bathy)

# --- flags / emblems / seals (generic stand-ins; real ones from Commons per ADR 0141/0142) ---
def tricolour(name: str, cols: list[str], vertical: bool) -> None:
    n = len(cols)
    body = "".join(
        f'<rect x="{i*90 if vertical else 0}" y="{0 if vertical else i*60}" width="{90 if vertical else 270}" height="{180 if vertical else 60}" fill="{c}"/>'
        for i, c in enumerate(cols))
    write(f"flags/{name}.svg", svg(270, 180, body + f'<rect width="270" height="180" fill="none" stroke="{INK}" stroke-opacity=".3"/>'))

tricolour("placeholder-horizontal", [EDIT, PAPER, FORK], False)
tricolour("placeholder-vertical", [INK, PAPER, SEA], True)
write("flags/placeholder-unknown.svg", svg(270, 180,
    f'<rect width="270" height="180" fill="{PAPER}" stroke="{INK}"/><path d="M0 0L270 180M270 0L0 180" stroke="{INK}" stroke-opacity=".3"/>'))
write("emblems/placeholder-emblem.svg", svg(128, 128,
    f'<path d="M64 8l48 16v40c0 30-20 48-48 58C36 112 16 94 16 64V24z" fill="{PAPER}" stroke="{INK}" stroke-width="4"/>'
    f'<circle cx="64" cy="58" r="18" fill="none" stroke="{INK}" stroke-width="4"/>'))
write("seals/seal-round-blue.svg", svg(128, 128,
    f'<circle cx="64" cy="64" r="58" fill="none" stroke="#2A4F9E" stroke-width="4"/><circle cx="64" cy="64" r="46" fill="none" stroke="#2A4F9E" stroke-width="2"/>'))
write("seals/seal-red-office.svg", svg(128, 128,
    f'<rect x="10" y="10" width="108" height="108" fill="none" stroke="#B3261E" stroke-width="6"/><path d="M34 44h60M34 64h60M34 84h60" stroke="#B3261E" stroke-width="5"/>'))
write("seals/seal-personal.svg", svg(96, 96,
    f'<rect x="6" y="6" width="84" height="84" rx="4" fill="#B3261E" fill-opacity=".85"/><path d="M26 30h44M26 48h44M26 66h44" stroke="{PAPER}" stroke-width="6"/>'))
write("seals/run-seal-template.svg", svg(160, 160,
    f'<circle cx="80" cy="80" r="72" fill="none" stroke="{INK}" stroke-width="3"/>'
    + "".join(f'<path d="M80 80L{80+72*math.cos(a*math.pi/8):.1f} {80+72*math.sin(a*math.pi/8):.1f}" stroke="{INK}" stroke-opacity=".4"/>' for a in range(16))
    + f'<circle cx="80" cy="80" r="26" fill="{PAPER}" stroke="{INK}" stroke-width="3"/>'))

# --- UI chrome ---
write("ui/panel-frame-light.svg", svg(320, 200,
    f'<rect x=".5" y=".5" width="319" height="199" fill="{PAPER}" stroke="{INK}" stroke-opacity=".2"/><path d="M0 28.5H320" stroke="{INK}" stroke-opacity=".2"/>'))
write("ui/sparkline-column.svg", svg(96, 24,
    f'<path d="M2 18l12-4 12 2 12-8 12 4 12-6 12 3 12-5" fill="none" stroke="{INK}" stroke-width="1.5"/>'))
write("ui/cursor-crosshair.svg", svg(32, 32,
    f'<path d="M16 4v24M4 16h24" stroke="{INK}" stroke-width="1.5"/><circle cx="16" cy="16" r="5" fill="none" stroke="{INK}"/>'))
write("ui/state-chips.svg", svg(240, 40,
    f'<rect x="4" y="8" width="48" height="24" rx="12" fill="{OK}"/><rect x="64" y="8" width="48" height="24" rx="12" fill="{WARN}"/>'
    f'<rect x="124" y="8" width="48" height="24" rx="12" fill="{CRISIS}"/><rect x="184" y="8" width="48" height="24" rx="12" fill="{EDIT}"/>'))

# --- store / social (deferred for 1.0; sized stand-ins only) ---
for nm, w, h in [("steam-capsule-main-616x353", 616, 353), ("steam-capsule-header-460x215", 460, 215),
                 ("steam-library-hero-3840x1240", 3840, 1240), ("steam-library-logo-1280x720", 1280, 720)]:
    write(f"deferred-store/{nm}.svg", svg(w, h,
        f'<rect width="{w}" height="{h}" fill="{INK}"/><circle cx="{w*0.78:.0f}" cy="{h/2:.0f}" r="{h*0.36:.0f}" fill="{SEA}"/>'
        f'<text x="{w*0.06:.0f}" y="{h/2:.0f}" font-family="serif" font-size="{h*0.12:.0f}" fill="{PAPER}">Ceteris Paribus</text>'))

write("README.md", "Generated by `scripts/gen_placeholder_art.py`; do not hand-edit. Placeholders only: shapes are stand-ins, "
      "not designs. Inventory and status: `docs/art-inventory.md`. Not referenced by app code.\n")
