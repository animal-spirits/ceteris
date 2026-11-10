"""Generate SVG art mocks for docs/art-inventory.md. Usage: python3 scripts/gen_mock_art.py app/assets/mock"""

import hashlib
import json
import math
import sys
from pathlib import Path

OUT = Path(sys.argv[1])

# Light tokens (app/ui/tokens.slint) and Night Atlas tokens (ADR 0137 mocks).
PAPER, INK, INK2, SEA, LAND = "#EDEFEA", "#1C292F", "#5B6B70", "#BCD1DB", "#E2E0D2"
EDIT, FORK = "#AE2459", "#5E45A3"
OK, WARN, CRISIS = "#3F8A5A", "#B07A1E", "#B8403A"
N_GROUND, N_PANEL, N_RAISED, N_LINE = "#11171B", "#182026", "#202A31", "#2B363C"
N_INK, N_INK2, N_SEA, N_LAND = "#E2E7E3", "#A2B0AE", "#1A2B36", "#26302C"
N_PROOF, N_BRANCH, N_WARN, N_OK, N_CRISIS = "#E35E91", "#A890E8", "#D9AA4E", "#6CB485", "#E36C63"
SERIF = "'Source Serif 4', Georgia, serif"
SANS = "'IBM Plex Sans Condensed', 'Arial Narrow', sans-serif"
MONO = "'IBM Plex Mono', Menlo, monospace"
SAMPLE_HASH = hashlib.sha256(b"ceteris run 2000-01-01 seed 0").hexdigest()

manifest: list[dict] = []


def f(x: float) -> str:
    return f"{x:.1f}".rstrip("0").rstrip(".")


def save(section: str, name: str, w: int, h: int, body: str, label: str, status: str,
         note: str = "", bg: str | None = None, icon: bool = False) -> None:
    root = f'xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"'
    if icon:
        root += f' fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" color="{INK}"'
    rect = f'<rect width="{w}" height="{h}" fill="{bg}"/>' if bg else ""
    path = OUT / section / f"{name}.svg"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"<svg {root}>{rect}{body}</svg>\n")
    manifest.append(dict(section=section, file=f"{section}/{name}.svg", label=label,
                         status=status, note=note, w=w, h=h, icon=icon))


def poly(pts) -> str:
    return " ".join(f"{f(x)},{f(y)}" for x, y in pts)


def blob(cx: float, cy: float, r: float, seed: int, n: int = 160, amp: float = 0.22):
    """Closed made-up coastline: sum of sines on a circle, deterministic."""
    pts = []
    for i in range(n):
        t = 2 * math.pi * i / n
        k = 1 + amp * (math.sin(3 * t + seed) * 0.5 + math.sin(7 * t + 2 * seed) * 0.3
                       + math.sin(13 * t + 3 * seed) * 0.15 + math.sin(29 * t + seed) * 0.05)
        pts.append((cx + r * k * math.cos(t), cy + r * 0.78 * k * math.sin(t)))
    return pts


def blob_path(pts) -> str:
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in pts) + "Z"


def offset(pts, cx, cy, d):
    out = []
    for x, y in pts:
        dx, dy = x - cx, y - cy
        L = math.hypot(dx, dy) or 1
        out.append((x + dx / L * d, y + dy / L * d))
    return out


def scallop(cx, cy, r, n, depth):
    pts = []
    for i in range(n * 8):
        t = 2 * math.pi * i / (n * 8)
        rr = r - depth * (1 - abs(math.cos(n * t / 2)))
        pts.append((cx + rr * math.cos(t), cy + rr * math.sin(t)))
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in pts) + "Z"


def gear(cx, cy, r_out, r_in, teeth):
    pts = []
    for i in range(teeth * 4):
        t = 2 * math.pi * i / (teeth * 4)
        r = r_out if i % 4 in (1, 2) else r_in
        pts.append((cx + r * math.cos(t), cy + r * math.sin(t)))
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in pts) + "Z"


def graticule(cx, cy, r, color, sw, meridians=6, parallels=5, opacity=1.0):
    g = [f'<g fill="none" stroke="{color}" stroke-width="{sw}" opacity="{opacity}">',
         f'<circle cx="{cx}" cy="{cy}" r="{r}"/>']
    for i in range(1, meridians):
        rx = r * abs(math.cos(math.pi * i / meridians))
        g.append(f'<ellipse cx="{cx}" cy="{cy}" rx="{f(rx)}" ry="{r}"/>' if rx > 0.5 else
                 f'<line x1="{cx}" y1="{cy - r}" x2="{cx}" y2="{cy + r}"/>')
    for j in range(1, parallels):
        y = cy - r + 2 * r * j / parallels
        half = math.sqrt(max(r * r - (y - cy) ** 2, 0))
        g.append(f'<line x1="{f(cx - half)}" y1="{f(y)}" x2="{f(cx + half)}" y2="{f(y)}"/>')
    g.append("</g>")
    return "".join(g)


def text(x, y, s, size, fill, family=SANS, anchor="start", weight=400, extra=""):
    return (f'<text x="{f(x)}" y="{f(y)}" font-family="{family}" font-size="{size}" fill="{fill}" '
            f'text-anchor="{anchor}" font-weight="{weight}" {extra}>{s}</text>')


def hypotrochoid(cx, cy, R, r, d, scale, steps=1400):
    turns = r // math.gcd(R, r)
    pts = []
    for i in range(steps + 1):
        t = 2 * math.pi * turns * i / steps
        x = (R - r) * math.cos(t) + d * math.cos((R - r) / r * t)
        y = (R - r) * math.sin(t) - d * math.sin((R - r) / r * t)
        pts.append((cx + x * scale, cy + y * scale))
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in pts)


# ---------------------------------------------------------------- 1 identity

def app_icon_globe(size=1024, mono=False):
    s = size / 1024
    ground = INK if mono else N_GROUND
    line = PAPER if mono else N_INK2
    body = [f'<rect x="{f(40*s)}" y="{f(40*s)}" width="{f(944*s)}" height="{f(944*s)}" rx="{f(208*s)}" fill="{ground}"/>']
    if not mono:
        body.append(f'<circle cx="{f(512*s)}" cy="{f(512*s)}" r="{f(330*s)}" fill="{N_SEA}"/>')
        land = blob(470 * s, 470 * s, 170 * s, 2, amp=0.35)
        body.append(f'<path d="{blob_path(land)}" fill="{N_LAND}" stroke="{N_INK2}" stroke-width="{f(3*s)}" '
                    f'clip-path="url(#g)"/>')
        body.insert(0, f'<defs><clipPath id="g"><circle cx="{f(512*s)}" cy="{f(512*s)}" r="{f(330*s)}"/></clipPath></defs>')
    body.append(graticule(512 * s, 512 * s, 330 * s, line, f(6 * s), opacity=0.55 if not mono else 0.9))
    # one trajectory, then a fork: the run and its branch.
    trunk = f"M{f(250*s)} {f(700*s)} C{f(380*s)} {f(640*s)} {f(470*s)} {f(560*s)} {f(530*s)} {f(470*s)}"
    a = f"M{f(530*s)} {f(470*s)} C{f(600*s)} {f(370*s)} {f(690*s)} {f(320*s)} {f(800*s)} {f(300*s)}"
    b = f"M{f(530*s)} {f(470*s)} C{f(620*s)} {f(470*s)} {f(700*s)} {f(480*s)} {f(790*s)} {f(530*s)}"
    c1, c2 = (PAPER, PAPER) if mono else (N_PROOF, N_BRANCH)
    body.append(f'<path d="{trunk}" stroke="{PAPER if mono else N_INK}" stroke-width="{f(26*s)}" fill="none" stroke-linecap="round"/>')
    body.append(f'<path d="{a}" stroke="{c1}" stroke-width="{f(26*s)}" fill="none" stroke-linecap="round"/>')
    body.append(f'<path d="{b}" stroke="{c2}" stroke-width="{f(26*s)}" fill="none" stroke-linecap="round" '
                f'{"stroke-dasharray=" + chr(34) + f(40*s) + " " + f(30*s) + chr(34) if mono else ""}/>')
    body.append(f'<circle cx="{f(530*s)}" cy="{f(470*s)}" r="{f(30*s)}" fill="{ground}" stroke="{PAPER if mono else N_INK}" stroke-width="{f(14*s)}"/>')
    return "".join(body)


def identity():
    sec = "identity"
    save(sec, "app-icon-a-globe-fork", 1024, 1024, app_icon_globe(), "App icon A: globe and fork",
         "1.0 artist", "One run (white) splits at an edit into the edited path (magenta) and a fork (violet).")

    # B: meridian ledger. A single meridian splitting, on paper.
    b = [f'<rect x="40" y="40" width="944" height="944" rx="208" fill="{PAPER}"/>',
         f'<rect x="40" y="40" width="944" height="944" rx="208" fill="none" stroke="{INK}" stroke-opacity="0.15" stroke-width="4"/>']
    for i in range(9):
        y = 200 + i * 78
        b.append(f'<line x1="170" y1="{y}" x2="854" y2="{y}" stroke="{INK}" stroke-opacity="0.18" stroke-width="5"/>')
    b.append(f'<path d="M512 860 V520" stroke="{INK}" stroke-width="44" stroke-linecap="round"/>')
    b.append(f'<path d="M512 520 C512 380 420 300 330 180" stroke="{EDIT}" stroke-width="44" fill="none" stroke-linecap="round"/>')
    b.append(f'<path d="M512 520 C512 380 604 300 694 180" stroke="{FORK}" stroke-width="44" fill="none" stroke-linecap="round"/>')
    b.append(f'<circle cx="512" cy="520" r="46" fill="{PAPER}" stroke="{INK}" stroke-width="22"/>')
    save(sec, "app-icon-b-ledger-meridian", 1024, 1024, "".join(b), "App icon B: ledger meridian",
         "1.0 artist", "Light variant: the year line as a ruled ledger, one trunk and two branches.")

    # C: dial with tick and fork notch.
    c = [f'<rect x="40" y="40" width="944" height="944" rx="208" fill="{N_PANEL}"/>',
         f'<circle cx="512" cy="512" r="340" fill="none" stroke="{N_LINE}" stroke-width="28"/>']
    for i in range(12):
        t = 2 * math.pi * i / 12 - math.pi / 2
        r1, r2 = (270, 330) if i % 3 == 0 else (295, 330)
        c.append(f'<line x1="{f(512+r1*math.cos(t))}" y1="{f(512+r1*math.sin(t))}" x2="{f(512+r2*math.cos(t))}" '
                 f'y2="{f(512+r2*math.sin(t))}" stroke="{N_INK2}" stroke-width="{18 if i % 3 == 0 else 10}" stroke-linecap="round"/>')
    c.append(f'<path d="M512 512 L512 230" stroke="{N_INK}" stroke-width="30" stroke-linecap="round"/>')
    c.append(f'<path d="M512 512 L740 390" stroke="{N_PROOF}" stroke-width="30" stroke-linecap="round"/>')
    c.append(f'<path d="M512 512 L760 560" stroke="{N_BRANCH}" stroke-width="30" stroke-linecap="round"/>')
    c.append(f'<circle cx="512" cy="512" r="40" fill="{N_INK}"/>')
    save(sec, "app-icon-c-clock-dial", 1024, 1024, "".join(c), "App icon C: clock dial",
         "1.0 artist", "The month clock: one hand for the run, two for edit and fork.")

    save(sec, "app-icon-mono-512", 512, 512, app_icon_globe(512, mono=True), "Monochrome icon",
         "1.0 mock", "Linux symbolic and tray. Dash marks the fork without colour.")

    fav = [f'<rect width="32" height="32" rx="7" fill="{N_GROUND}"/>',
           f'<circle cx="16" cy="16" r="11" fill="{N_SEA}" stroke="{N_INK2}" stroke-width="1"/>',
           f'<path d="M7 22 Q12 19 16 15" stroke="{N_INK}" stroke-width="2.4" fill="none" stroke-linecap="round"/>',
           f'<path d="M16 15 Q19 10 25 9" stroke="{N_PROOF}" stroke-width="2.4" fill="none" stroke-linecap="round"/>',
           f'<path d="M16 15 Q20 15 25 17" stroke="{N_BRANCH}" stroke-width="2.4" fill="none" stroke-linecap="round"/>']
    save(sec, "favicon-32", 32, 32, "".join(fav), "Favicon", "1.0 mock", "Derived from icon A at 32 px.")

    wm = [text(96, 92, "Ceteris Paribus", 64, INK, SERIF, weight=600, extra='letter-spacing="0.5"'),
          f'<path d="M30 108 V70" stroke="{INK}" stroke-width="5" stroke-linecap="round"/>',
          f'<path d="M30 70 Q30 46 14 30" stroke="{EDIT}" stroke-width="5" fill="none" stroke-linecap="round"/>',
          f'<path d="M30 70 Q30 46 46 30" stroke="{FORK}" stroke-width="5" fill="none" stroke-linecap="round"/>',
          f'<circle cx="30" cy="70" r="6" fill="{PAPER}" stroke="{INK}" stroke-width="3"/>',
          text(98, 124, "ALL ELSE EQUAL, UNTIL YOU CHANGE IT", 15, INK2, SANS, extra='letter-spacing="3"')]
    save(sec, "wordmark", 620, 140, "".join(wm), "Wordmark", "1.0 artist",
         "Source Serif 4 Semibold (OFL) with the ledger mark.", bg=PAPER)

    og = [f'<rect width="1200" height="630" fill="{N_GROUND}"/>']
    coast = blob(930, 330, 210, 5, amp=0.3)
    for k in range(6, 0, -1):
        og.append(f'<path d="{blob_path(offset(coast, 930, 330, k*16))}" fill="none" stroke="{N_SEA}" stroke-width="{f(1+k*0.6)}" opacity="{f(0.9-k*0.1)}"/>')
    og.append(f'<path d="{blob_path(coast)}" fill="{N_LAND}" stroke="{N_INK2}" stroke-width="1.5"/>')
    og.append(graticule(930, 330, 290, N_LINE, 1.2, 8, 6))
    og.append(f'<path d="M700 540 C800 480 870 420 910 330 C950 250 1030 180 1160 150" stroke="{N_PROOF}" stroke-width="5" fill="none"/>')
    og.append(f'<path d="M910 330 C980 330 1060 360 1170 420" stroke="{N_BRANCH}" stroke-width="5" fill="none" stroke-dasharray="14 8"/>')
    og.append(text(74, 336, "from 1 January 2000", 26, N_INK2, SANS))
    og.append(text(72, 250, "Ceteris Paribus", 76, N_INK, SERIF, weight=600))
    og.append(text(74, 300, "A deterministic, forkable world simulator", 26, N_INK2, SANS))
    og.append(text(74, 560, f"RUN {SAMPLE_HASH[:12].upper()}  ·  TICK 0  ·  1 JAN 2000", 18, N_INK2, MONO, extra='letter-spacing="2"'))
    save(sec, "og-image-1200x630", 1200, 630, "".join(og), "Docs site / OG image", "1.0 mock", "1200x630 share card.")

    dmg = [f'<rect width="660" height="400" fill="{PAPER}"/>',
           f'<rect x="20" y="20" width="620" height="360" fill="none" stroke="{INK}" stroke-opacity="0.2"/>',
           text(330, 70, "Drag Ceteris to Applications", 22, INK, SERIF, "middle", 600),
           f'<rect x="110" y="140" width="128" height="128" rx="28" fill="none" stroke="{INK}" stroke-opacity="0.25" stroke-dasharray="6 6"/>',
           f'<rect x="422" y="140" width="128" height="128" rx="28" fill="none" stroke="{INK}" stroke-opacity="0.25" stroke-dasharray="6 6"/>',
           f'<path d="M270 204 H390 M370 186 L392 204 L370 222" stroke="{EDIT}" stroke-width="3" fill="none" stroke-linecap="round"/>',
           text(174, 300, "Ceteris.app", 14, INK2, SANS, "middle"), text(486, 300, "Applications", 14, INK2, SANS, "middle"),
           text(330, 360, "Signed build. Packs download on first run.", 12, INK2, MONO, "middle")]
    save(sec, "dmg-background-660x400", 660, 400, "".join(dmg), "DMG background", "1.0 low",
         "Icon slots dashed; the real icons sit on top.")


# ------------------------------------------------------------ 2 first run

def first_run():
    sec = "first-run"
    s = [f'<rect width="1600" height="900" fill="{N_GROUND}"/>']
    coast = blob(980, 430, 330, 9, amp=0.32)
    for k in range(8, 0, -1):
        s.append(f'<path d="{blob_path(offset(coast, 980, 430, k*22))}" fill="{N_SEA}" opacity="{f(0.10+0.05*(8-k))}"/>')
    s.append(f'<path d="{blob_path(coast)}" fill="{N_LAND}"/>')
    # hachure ticks on the inland ridge
    for i in range(40):
        t = i / 40
        x, y = 860 + 260 * t, 380 + 40 * math.sin(t * 6)
        s.append(f'<line x1="{f(x)}" y1="{f(y)}" x2="{f(x+4)}" y2="{f(y+14)}" stroke="{N_INK2}" stroke-width="1.4" opacity="0.6"/>')
    s.append(f'<path d="{blob_path(coast)}" fill="none" stroke="{N_INK2}" stroke-width="1.6"/>')
    s.append(graticule(980, 430, 520, N_LINE, 1, 10, 8))
    s.append(text(120, 380, "Ceteris Paribus", 96, N_INK, SERIF, weight=600))
    s.append(text(124, 440, "1 January 2000", 34, N_INK2, SERIF, extra='font-style="italic"'))
    stages = ["Read pack", "Verify sources", "Build provinces", "Solve 2000 equilibrium", "Ready"]
    for i, st in enumerate(stages):
        x = 124 + i * 250
        done = i < 3
        col = N_INK if done else N_INK2
        s.append(f'<circle cx="{x}" cy="760" r="7" fill="{N_OK if done else "none"}" stroke="{col}" stroke-width="2"/>')
        if i < len(stages) - 1:
            s.append(f'<line x1="{x+14}" y1="760" x2="{x+236}" y2="760" stroke="{N_INK2 if i < 2 else N_LINE}" stroke-width="2"/>')
        s.append(text(x, 794, st.upper(), 15, col, SANS, extra='letter-spacing="2"'))
    s.append(text(124, 840, f"pack 0.9.2 · hash {SAMPLE_HASH[:16]}", 15, N_INK2, MONO))
    save(sec, "splash-1600x900", 1600, 900, "".join(s), "Splash and load stages", "1.0 mock",
         "Night Atlas. Made-up coast, stepped bathymetry, hachures; the load stages from F4/P13 underneath.")

    il = [f'<rect width="480" height="320" fill="{PAPER}"/>']
    # source documents flowing into a pack
    for i, (lab, y) in enumerate([("WDI", 60), ("ICIO", 120), ("GEBCO", 180), ("WPP", 240)]):
        il.append(f'<rect x="40" y="{y-22}" width="96" height="40" rx="3" fill="{PAPER}" stroke="{INK}" stroke-width="1.5"/>')
        il.append(f'<line x1="50" y1="{y-8}" x2="100" y2="{y-8}" stroke="{INK}" stroke-opacity="0.35" stroke-width="1.5"/>')
        il.append(f'<line x1="50" y1="{y+2}" x2="90" y2="{y+2}" stroke="{INK}" stroke-opacity="0.35" stroke-width="1.5"/>')
        il.append(text(126, y+12, lab, 11, INK2, MONO, "end"))
        il.append(f'<path d="M140 {y} C220 {y} 230 160 300 160" stroke="{INK}" stroke-opacity="0.4" stroke-width="1.5" fill="none" stroke-dasharray="4 4"/>')
    il.append(f'<path d="M300 110 L380 90 L440 110 L440 220 L380 240 L300 220 Z" fill="{LAND}" stroke="{INK}" stroke-width="2"/>')
    il.append(f'<path d="M300 110 L360 130 L440 110 M360 130 V240" stroke="{INK}" stroke-width="2" fill="none"/>')
    il.append(f'<path d="M375 170 l10 10 l20 -24" stroke="{OK}" stroke-width="4" fill="none" stroke-linecap="round"/>')
    il.append(text(370, 280, "data pack", 14, INK, SERIF, "middle", extra='font-style="italic"'))
    save(sec, "setup-illustration", 480, 320, "".join(il), "First-run setup illustration", "1.0 mock",
         "Licensed sources fetched on your machine, checked, packed.")

    dz = [f'<rect x="8" y="8" width="304" height="184" rx="10" fill="none" stroke="{INK}" stroke-opacity="0.45" stroke-width="2" stroke-dasharray="8 6"/>',
          f'<path d="M160 50 V112 M138 92 L160 114 L182 92" stroke="{INK}" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
          f'<path d="M116 118 V134 H204 V118" stroke="{INK}" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"/>',
          text(160, 166, "Drop a source file you downloaded by hand", 13, INK2, SANS, "middle")]
    save(sec, "drop-zone", 320, 200, "".join(dz), "Manual-file drop zone", "1.0 mock", "", bg=PAPER)

    def empty(name, label, art, line):
        save(sec, name, 160, 160, art + text(80, 150, line, 11, INK2, SANS, "middle"), label, "1.0 optional", "", bg=PAPER)

    empty("empty-no-branches", "Empty state: no branches",
          f'<path d="M30 110 H130" stroke="{INK}" stroke-width="3" stroke-linecap="round"/>'
          f'<path d="M70 110 C90 110 95 70 125 60" stroke="{FORK}" stroke-width="2.5" fill="none" stroke-dasharray="5 5"/>'
          f'<circle cx="70" cy="110" r="6" fill="{PAPER}" stroke="{FORK}" stroke-width="2.5"/>', "Fork at any tick")
    empty("empty-no-edits", "Empty state: no edits",
          f'<rect x="40" y="26" width="80" height="96" rx="3" fill="none" stroke="{INK}" stroke-width="2"/>'
          + "".join(f'<line x1="52" y1="{46+i*16}" x2="108" y2="{46+i*16}" stroke="{INK}" stroke-opacity="0.2" stroke-width="2"/>' for i in range(4))
          + f'<path d="M96 104 L128 72 L136 80 L104 112 L94 114 Z" fill="{PAPER}" stroke="{EDIT}" stroke-width="2.5" stroke-linejoin="round"/>',
          "No edits yet")
    empty("empty-no-ensemble", "Empty state: no ensemble",
          "".join(f'<path d="M24 100 C70 100 90 {100-d} 136 {100-d*1.6}" stroke="{INK}" stroke-opacity="0.18" stroke-width="2" fill="none" stroke-dasharray="3 4"/>'
                  for d in (-24, -12, 12, 24, 36))
          + f'<path d="M24 100 C70 100 90 100 136 100" stroke="{INK}" stroke-width="3" fill="none" stroke-linecap="round"/>',
          "One run. Add seeds to see spread")

    h = [f'<rect width="320" height="200" fill="{PAPER}"/>',
         f'<circle cx="110" cy="96" r="62" fill="none" stroke="{INK}" stroke-width="3"/>']
    for i in range(12):
        t = 2 * math.pi * i / 12
        h.append(f'<line x1="{f(110+52*math.cos(t))}" y1="{f(96+52*math.sin(t))}" x2="{f(110+58*math.cos(t))}" y2="{f(96+58*math.sin(t))}" stroke="{INK}" stroke-width="2"/>')
    h.append(f'<path d="M110 96 V54 M110 96 L138 110" stroke="{INK}" stroke-width="3" stroke-linecap="round"/>')
    h.append(f'<g transform="rotate(-12 236 96)"><rect x="186" y="66" width="104" height="58" rx="4" fill="none" stroke="{CRISIS}" stroke-width="3"/>'
             f'{text(238, 92, "HALTED", 18, CRISIS, SANS, "middle", 600, "letter-spacing=\"3\"")}'
             f'{text(238, 112, "TICK 4,117", 12, CRISIS, MONO, "middle")}</g>')
    h.append(text(160, 186, "Invariant failed. State and journal saved.", 12, INK2, SANS, "middle"))
    save(sec, "halt-illustration", 320, 200, "".join(h), "Halt panel illustration", "1.0 mock",
         "ADR 0214: stopped clock and a stamp naming the tick.")


# ------------------------------------------------------------------ 3 icons

ICONS: dict[str, tuple[str, str, str]] = {
    # name: (group, label, body)
    "pause": ("time", "Pause", '<rect x="6.5" y="5" width="3.5" height="14" rx="1"/><rect x="14" y="5" width="3.5" height="14" rx="1"/>'),
    "play": ("time", "Play", '<path d="M7.5 5.2v13.6L18.5 12z"/>'),
    "speed-1": ("time", "Speed 1", '<path d="M10 7l5 5-5 5"/>'),
    "speed-2": ("time", "Speed 2", '<path d="M7 7l5 5-5 5M12.5 7l5 5-5 5"/>'),
    "speed-3": ("time", "Speed 3", '<path d="M4 7l5 5-5 5M9.5 7l5 5-5 5M15 7l5 5-5 5"/>'),
    "run-to-tick": ("time", "Run to tick", '<path d="M4 6v9l7-4.5z"/><path d="M15 4v11"/><path d="M3 20h18M7 18v2M11 18v2M15 17.5v2.5M19 18v2"/>'),
    "seek": ("time", "Seek", '<path d="M3 17h18M5 15v4M9 16v2M13 16v2M17 16v2M21 15v4" stroke-opacity="0.6"/><path d="M12 4v13"/><path d="M9 4h6l-3 3z" fill="currentColor"/>'),
    "fork": ("branch", "Fork", f'<path d="M7 21V3"/><path d="M7 13c0-4 3-6 10-6" stroke="{FORK}"/><circle cx="17" cy="7" r="2" stroke="{FORK}"/><circle cx="7" cy="13" r="1.6" fill="currentColor"/>'),
    "edit-stamp": ("branch", "Edit stamp", f'<path d="M10 3h4v6h-4z"/><path d="M8 9h8l1 4H7z"/><path d="M5 13h14v3H5z"/><path d="M5 20h14" stroke="{EDIT}" stroke-width="2"/>'),
    "branch-god": ("branch", "God-mode branch", f'<path d="M6 21V3"/><path d="M6 4h12l-3 4 3 4H6" stroke="{FORK}"/><path d="M10 6.5v3" stroke="{FORK}"/>'),
    "seek-cursor": ("branch", "Seek cursor", f'<path d="M12 6v15" stroke="{EDIT}"/><path d="M8 3h8l-4 4z" fill="{EDIT}" stroke="{EDIT}"/><path d="M4 21h16" stroke-opacity="0.5"/>'),
    "hash-seal": ("branch", "Run hash seal", scallop(12, 12, 9.5, 12, 1.4).join(['<path d="', '"/>']) + '<path d="M10 8l-1 8M15 8l-1 8M8 10.5h8M7.5 13.5h8"/>'),
    "ok": ("state", "OK", f'<circle cx="12" cy="12" r="9" stroke="{OK}"/><path d="M8 12.5l3 3 5-6" stroke="{OK}"/>'),
    "warn": ("state", "Warn", f'<path d="M12 3.5L21.5 20H2.5z" stroke="{WARN}"/><path d="M12 10v4.5M12 17.2v.3" stroke="{WARN}"/>'),
    "crisis": ("state", "Crisis", f'<path d="M8.3 3h7.4L21 8.3v7.4L15.7 21H8.3L3 15.7V8.3z" stroke="{CRISIS}"/><path d="M9 9l6 6M15 9l-6 6" stroke="{CRISIS}"/>'),
    "mode-political": ("mode", "Political", '<path d="M3 5l6-2 6 2 6-2v16l-6 2-6-2-6 2z"/><path d="M9 3v16M15 5l-2 6 4 3-2 7" stroke-dasharray="2 2"/>'),
    "mode-terrain": ("mode", "Terrain", '<path d="M2 19l6.5-10 4 6 3-4L22 19z"/><path d="M8.5 9l-1 3M9.5 10.5l-.5 2.5M16 11l1 2.5"/>'),
    "mode-output": ("mode", "Output", '<path d="M3 21V11l5 3v-3l5 3V7h3l1-3h2l1 17z"/><path d="M3 21h18"/>'),
    "mode-price": ("mode", "Price", '<path d="M3 12V4h8l10 10-8 8z"/><circle cx="7.5" cy="8.5" r="1.5"/>'),
    "mode-cost": ("mode", "Cost", '<ellipse cx="12" cy="6" rx="7" ry="2.5"/><path d="M5 6v4c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5V6M5 10v4c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-4M5 14v4c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-4"/>'),
    "mode-wage": ("mode", "Wage", '<circle cx="9" cy="7" r="3"/><path d="M3 20c0-4 2.7-6.5 6-6.5s6 2.5 6 6.5"/><circle cx="18" cy="15" r="3.5"/><path d="M18 13.5v3"/>'),
    "mode-unemployment": ("mode", "Unemployment", '<circle cx="12" cy="6.5" r="3"/><path d="M6 17c0-3.5 2.7-6 6-6s6 2.5 6 6"/><path d="M4 21h16" stroke-dasharray="2.5 2.5"/>'),
    "mode-trade": ("mode", "Trade", '<path d="M4 8h15M15 4l4 4-4 4"/><path d="M20 16H5M9 12l-4 4 4 4"/>'),
    "mode-freight": ("mode", "Freight", '<path d="M2 15h20l-3 5H5z"/><path d="M5 15v-4h4v4M10 15V9h4v6M15 15v-4h4v4"/><path d="M12 9V4"/>'),
    "mode-bottleneck": ("mode", "Bottleneck", '<path d="M4 3h16l-6 8v2l6 8H4l6-8v-2z"/><path d="M12 13v3"/>'),
    "mode-edits": ("mode", "Edits", f'<path d="M3 5l6-2 6 2 6-2v16l-6 2-6-2-6 2z" stroke-opacity="0.55"/><circle cx="12" cy="10" r="3" stroke="{EDIT}"/><path d="M12 13v5" stroke="{EDIT}"/>'),
    "mode-diff": ("mode", "Diff", f'<circle cx="12" cy="12" r="9"/><path d="M12 3v18"/><path d="M12 3a9 9 0 010 18z" fill="{FORK}" fill-opacity="0.25" stroke="{FORK}"/>'),
    "ensemble-fan": ("analysis", "Ensemble fan", '<path d="M3 16h4"/><path d="M7 16c5 0 8-9 14-11M7 16c5 0 9-4 14-5M7 16c5 0 9 0 14 0M7 16c5 0 9 2 14 4" stroke-opacity="0.75"/><circle cx="7" cy="16" r="1.5" fill="currentColor"/>'),
    "paired-effect": ("analysis", "Paired effect", f'<path d="M3 17c4-2 7-6 18-7" stroke-opacity="0.55"/><path d="M3 17c4-1 8-2 18-1" stroke="{FORK}"/><path d="M18 10.5v5" stroke="{EDIT}"/><path d="M16.5 11h3M16.5 15.5h3" stroke="{EDIT}"/>'),
    "why": ("analysis", "Why is this here", '<circle cx="10.5" cy="10.5" r="6.5"/><path d="M15.5 15.5L21 21"/><path d="M8.7 8.8a1.9 1.9 0 113 1.5c-.8.5-1.2.9-1.2 1.8M10.5 13.8v.2"/>'),
    "scenario": ("nav", "Scenario", '<path d="M5 3h10l4 4v14H5z"/><path d="M15 3v4h4"/><circle cx="12" cy="14" r="4"/><path d="M8 14h8M12 10c1.5 1.2 1.5 6.8 0 8M12 10c-1.5 1.2-1.5 6.8 0 8"/>'),
    "lesson": ("nav", "Lesson", '<path d="M3 5c3-1 6-1 9 1 3-2 6-2 9-1v14c-3-1-6-1-9 1-3-2-6-2-9-1z"/><path d="M12 6v14"/><path d="M6 10h3M6 13h3M15 10h3"/>'),
    "settings": ("nav", "Settings", gear(12, 12, 9.5, 7.5, 8).join(['<path d="', '"/>']) + '<circle cx="12" cy="12" r="3"/>'),
    "language": ("nav", "Language", '<path d="M3 5h10v8H8l-3 3v-3H3z"/><path d="M6 8h4M8 7v1c0 1.5-1 2.5-2.5 3"/><path d="M14 9h7v8h-2v3l-3-3h-5v-2"/><path d="M15 15l2-5 2 5M15.6 13.5h2.8"/>'),
    "accessibility": ("nav", "Accessibility", '<circle cx="12" cy="12" r="9.5"/><circle cx="12" cy="7" r="1.3" fill="currentColor"/><path d="M7 9.5l5 1 5-1M12 10.5v3.5l-2.5 4.5M12 14l2.5 4.5"/>'),
    "fetch-source": ("setup", "Fetch source", '<path d="M7 17H6a4 4 0 01-.5-8A6 6 0 0117 8a4.5 4.5 0 011 9"/><path d="M12 11v9M9 17l3 3 3-3"/>'),
    "disk": ("setup", "On disk", '<ellipse cx="12" cy="6" rx="8" ry="3"/><path d="M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6"/><path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>'),
    "resume": ("setup", "Resume download", '<path d="M20 12a8 8 0 11-2.3-5.7"/><path d="M20 4v4h-4"/><path d="M10 9v6l5-3z"/>'),
    "verify": ("setup", "Verify checksum", f'<path d="M12 3l8 3v6c0 4.5-3.4 8-8 9-4.6-1-8-4.5-8-9V6z"/><path d="M8.5 12l2.5 2.5 4.5-5" stroke="{OK}"/>'),
    "halt": ("setup", "Halt", f'<circle cx="12" cy="12" r="9" stroke="{CRISIS}"/><rect x="8.5" y="8.5" width="7" height="7" rx="1" stroke="{CRISIS}"/>'),
    "report-problem": ("setup", "Report a problem", '<path d="M6 3h9l4 4v14H6z"/><path d="M15 3v4h4"/><path d="M12.5 9.5v5M12.5 17.2v.3"/>'),
    "drop-zone": ("setup", "Drop zone", '<path d="M4 8V5a1 1 0 011-1h3M16 4h3a1 1 0 011 1v3M20 16v3a1 1 0 01-1 1h-3M8 20H5a1 1 0 01-1-1v-3" /><path d="M12 7v8M9 12l3 3 3-3"/>'),
    "profiler": ("dev", "Tick profiler (dev)", '<path d="M3 20h18"/><rect x="3" y="15" width="18" height="3"/><rect x="5" y="11" width="9" height="3"/><rect x="15" y="11" width="4" height="3"/><rect x="6" y="7" width="5" height="3"/>'),
    "hash-diff": ("dev", "Hash diff (dev)", f'<path d="M6 5l-1 8M9.5 5l-1 8M4 7.5h6.5M3.5 10.5h6.5"/><path d="M15 11l-1 8M18.5 11l-1 8M13 13.5h6.5M12.5 16.5h6.5" stroke="{CRISIS}"/><path d="M14 4l6 4" stroke="{CRISIS}"/>'),
}

GROUP_TITLES = {"time": "Time controls", "branch": "Branch and edit", "state": "State",
                "mode": "Map modes (12 in app/src/modes.rs)", "analysis": "Analysis",
                "nav": "Navigation", "setup": "Setup and halt", "dev": "Developer builds"}


def icons():
    for name, (group, label, body) in ICONS.items():
        save("icons", name, 24, 24, body, label, "1.0 mock", GROUP_TITLES[group], icon=True)


# -------------------------------------------------------------------- 4 map

def map_furniture():
    sec = "map"
    # Compass rose, 2000s cartographic.
    c = [f'<circle cx="80" cy="80" r="62" fill="none" stroke="{INK}" stroke-width="1"/>',
         f'<circle cx="80" cy="80" r="56" fill="none" stroke="{INK}" stroke-width="0.6"/>']
    for i in range(72):
        t = 2 * math.pi * i / 72
        r1 = 50 if i % 9 == 0 else 53
        c.append(f'<line x1="{f(80+r1*math.cos(t))}" y1="{f(80+r1*math.sin(t))}" x2="{f(80+56*math.cos(t))}" y2="{f(80+56*math.sin(t))}" stroke="{INK}" stroke-width="0.6"/>')
    for i in range(8):
        t = math.pi * i / 4 - math.pi / 2
        L = 46 if i % 2 == 0 else 28
        w = 7 if i % 2 == 0 else 5
        tip = (80 + L * math.cos(t), 80 + L * math.sin(t))
        l = (80 + w * math.cos(t - math.pi / 2), 80 + w * math.sin(t - math.pi / 2))
        r = (80 + w * math.cos(t + math.pi / 2), 80 + w * math.sin(t + math.pi / 2))
        c.append(f'<polygon points="{poly([tip, l, (80, 80)])}" fill="{INK}"/>')
        c.append(f'<polygon points="{poly([tip, r, (80, 80)])}" fill="{PAPER}" stroke="{INK}" stroke-width="0.8"/>')
    c.append(text(80, 14, "N", 13, INK, SERIF, "middle", 600))
    save(sec, "compass-rose", 160, 160, "".join(c), "Compass rose", "1.x needs RFC", "Atlas details; 2000s plain style.", bg=PAPER)

    sb = [text(20, 22, "0", 11, INK, MONO, "middle")]
    for i in range(5):
        sb.append(f'<rect x="{20+i*56}" y="30" width="56" height="7" fill="{INK if i % 2 == 0 else PAPER}" stroke="{INK}" stroke-width="1"/>')
        sb.append(text(20 + (i + 1) * 56, 22, str((i + 1) * 100), 11, INK, MONO, "middle"))
    sb.append(text(316, 37, "km", 11, INK2, SANS))
    sb.append(text(20, 58, "Scale at 40°N, equal-area projection", 11, INK2, SANS))
    save(sec, "scale-bar", 340, 70, "".join(sb), "Scale bar", "1.x needs RFC", "", bg=PAPER)

    ca = [f'<rect x="10" y="10" width="300" height="130" fill="{PAPER}" stroke="{INK}" stroke-width="2"/>',
          f'<rect x="16" y="16" width="288" height="118" fill="none" stroke="{INK}" stroke-width="0.7"/>',
          text(160, 54, "IBERIA", 26, INK, SERIF, "middle", 600, 'letter-spacing="8"'),
          text(160, 80, "Output per worker, 2000 prices", 13, INK, SERIF, "middle", extra='font-style="italic"'),
          f'<line x1="70" y1="94" x2="250" y2="94" stroke="{INK}" stroke-width="0.7"/>',
          text(160, 116, "TICK 37  ·  JANUARY 2003  ·  BRANCH a7f3", 10, INK2, MONO, "middle", extra='letter-spacing="1"')]
    save(sec, "decade-cartouche-2000s", 320, 150, "".join(ca), "Decade cartouche (2000s)", "1.x needs RFC",
         "Style steps with the simulated decade.")

    # Water lining + hachures on one plate
    wl = [f'<rect width="420" height="240" fill="{SEA}"/>']
    coast = blob(270, 140, 120, 4, amp=0.3)
    for k in range(1, 9):
        wl.append(f'<path d="{blob_path(offset(coast, 270, 140, k*7))}" fill="none" stroke="{INK}" stroke-opacity="{f(0.45-k*0.045)}" stroke-width="0.8"/>')
    wl.append(f'<path d="{blob_path(coast)}" fill="{LAND}" stroke="{INK}" stroke-width="1.3"/>')
    for i in range(46):
        t = i / 46
        x = 200 + 150 * t
        y = 120 + 22 * math.sin(t * 7)
        L = 8 + 6 * abs(math.sin(t * 7 + 1))
        wl.append(f'<line x1="{f(x)}" y1="{f(y)}" x2="{f(x+2)}" y2="{f(y+L)}" stroke="{INK}" stroke-width="1"/>')
    wl.append(text(110, 210, "Mar Interior", 14, INK2, SERIF, "middle", extra='font-style="italic" letter-spacing="2"'))
    save(sec, "water-lining-hachures", 420, 240, "".join(wl), "Water lining and hachures", "1.x needs RFC",
         "Made-up coast. Lines fade offshore; hachure length follows slope.")

    bands = ["#DCE7EC", "#BCD1DB", "#9FBCCA", "#83A6B8", "#6990A5", "#4F7A92", "#386480", "#244F6C"]
    bb = []
    for i, c_ in enumerate(bands):
        bb.append(f'<rect x="{10+i*44}" y="10" width="44" height="34" fill="{c_}"/>')
        bb.append(text(10 + i * 44, 60, ["0", "200", "1k", "2k", "3k", "4k", "5k", "6k"][i], 10, INK, MONO))
    bb.append(text(10, 78, "Depth, m (GEBCO, stepped)", 11, INK2, SANS))
    save(sec, "bathymetry-bands", 372, 86, "".join(bb), "Bathymetry bands", "1.0 pipeline", "8 stepped bands.", bg=PAPER)

    seq = ["#F1EEDC", "#DCD9B4", "#BCC48E", "#93AB72", "#6A8E5F", "#46704F", "#2B5240", "#173630"]
    div = ["#8C2D3A", "#B8585A", "#DB948B", "#EFD0C6", "#E9ECE6", "#C9DCE4", "#8DB5CA", "#4E86A6", "#215A7F"]
    for name, ramp, label in (("ramp-sequential", seq, "Sequential ramp"), ("ramp-diverging", div, "Diverging ramp")):
        r = [f'<rect x="{10+i*36}" y="10" width="36" height="28" fill="{c_}"/>' for i, c_ in enumerate(ramp)]
        r.append(f'<rect x="10" y="10" width="{36*len(ramp)}" height="28" fill="none" stroke="{INK}" stroke-opacity="0.3"/>')
        save(sec, name, 20 + 36 * len(ramp), 48, "".join(r), label, "1.0 exists (code)", "Swatch; values live in ramps.rs.", bg=PAPER)

    pins = [f'<path d="M24 54 C24 54 8 34 8 22 a16 16 0 0132 0 C40 34 24 54 24 54Z" fill="{EDIT}"/>',
            f'<path d="M18 22 l10 -8 l3 3 l-10 8 l-4 1z" fill="{PAPER}"/>',
            f'<path d="M80 54 C80 54 64 34 64 22 a16 16 0 0132 0 C96 34 80 54 80 54Z" fill="{FORK}"/>',
            f'<path d="M76 30 V14 M76 22 c0-4 3-6 8-6" stroke="{PAPER}" stroke-width="2.5" fill="none" stroke-linecap="round"/>',
            f'<path d="M116 10 h28 l6 6 v28 h-34 z" fill="none" stroke="{INK}" stroke-width="2" stroke-dasharray="5 3"/>',
            f'<circle cx="116" cy="10" r="3" fill="{INK}"/><circle cx="150" cy="44" r="3" fill="{INK}"/>']
    save(sec, "map-pins", 160, 60, "".join(pins), "Edit pin, fork pin, selection edge", "1.0 mock", "", bg=PAPER)

    bs = []
    rows = [("Country", f'stroke="{INK}" stroke-width="2.4"'),
            ("Province", f'stroke="{INK}" stroke-opacity="0.5" stroke-width="1"'),
            ("Disputed", f'stroke="{INK}" stroke-width="2" stroke-dasharray="8 5"'),
            ("Line of control", f'stroke="{INK}" stroke-width="1.6" stroke-dasharray="2 4"'),
            ("Worldview B", f'stroke="{FORK}" stroke-width="2" stroke-dasharray="10 3 2 3"')]
    for i, (lab, st) in enumerate(rows):
        y = 18 + i * 26
        bs.append(f'<path d="M10 {y} q30 -8 60 0 t60 0 t60 0" fill="none" {st}/>')
        bs.append(text(200, y + 4, lab, 12, INK, SANS))
    save(sec, "border-styles", 300, 140, "".join(bs), "Border styles", "1.0 pipeline", "ADR 0141 worldview variants as shader params.", bg=PAPER)


# ------------------------------------------------------------------ 5 chrome

def chrome():
    sec = "chrome"
    chips = []
    for i, (lab, col, glyph) in enumerate([("OK", OK, "M7 12.5l2.5 2.5 4.5-5"), ("STRAIN", WARN, "M10.5 8v4M10.5 14.5v.3"),
                                           ("CRISIS", CRISIS, "M8 9l5 5M13 9l-5 5"), ("EDITED", EDIT, "M7 15l7-7 2 2-7 7H7z"),
                                           ("FORKED", FORK, "M8 16V7M8 12c0-2.5 2-4 6-4")]):
        x, yo = 10 + (i % 3) * 112, (i // 3) * 36
        chips.append(f'<rect x="{x}" y="{10+yo}" width="104" height="26" rx="13" fill="{col}" fill-opacity="0.12" stroke="{col}"/>')
        chips.append(f'<g transform="translate({x+4} {12+yo}) scale(0.95)"><path d="{glyph}" fill="none" stroke="{col}" stroke-width="1.8" stroke-linecap="round"/></g>')
        chips.append(text(x + 28, 28 + yo, lab, 12, col, SANS, weight=600, extra='letter-spacing="1.5"'))
    save(sec, "state-chips", 346, 82, "".join(chips), "State chips", "1.0 mock", "Shape plus colour for CVD (ADR 0221).", bg=PAPER)

    def panel(theme):
        bg, pn, ln, ink, ink2, acc = (PAPER, "#F6F7F3", "#1C292F33", INK, INK2, EDIT) if theme == "light" else \
            (N_GROUND, N_PANEL, N_LINE, N_INK, N_INK2, N_PROOF)
        p = [f'<rect width="340" height="220" fill="{bg}"/>',
             f'<rect x="12" y="12" width="316" height="196" fill="{pn}" stroke="{ln}"/>',
             text(28, 40, "Castilla-La Mancha", 18, ink, SERIF, weight=600),
             text(28, 58, "PROVINCE · ES42 · POP 1.76 M", 10, ink2, SANS, extra='letter-spacing="1.5"'),
             f'<line x1="28" y1="70" x2="312" y2="70" stroke="{ln}"/>']
        for j, (k, v, sp) in enumerate([("Output", "21.4 bn EUR", [4, 5, 5, 6, 6, 7, 8, 8]),
                                        ("Wage", "11.9 EUR/h", [6, 6, 5, 6, 7, 7, 7, 8]),
                                        ("Unemployment", "13.8 %", [8, 7, 7, 6, 6, 5, 4, 5])]):
            y = 96 + j * 34
            p.append(text(28, y, k, 13, ink2, SANS))
            p.append(text(220, y, v, 13, ink, MONO, "end"))
            pts = [(240 + i * 10, y - 2 - h * 1.4) for i, h in enumerate(sp)]
            p.append(f'<polyline points="{poly(pts)}" fill="none" stroke="{ink}" stroke-width="1.3"/>')
            p.append(f'<circle cx="{f(pts[-1][0])}" cy="{f(pts[-1][1])}" r="2.2" fill="{acc if j == 0 else ink}"/>')
        p.append(f'<rect x="28" y="182" width="72" height="16" fill="none" stroke="{acc}"/>')
        p.append(text(64, 194, "EDITED", 9, acc, SANS, "middle", 600, 'letter-spacing="1.5"'))
        p.append(text(312, 194, "why?", 11, ink2, SERIF, "end", extra='font-style="italic"'))
        return "".join(p)

    save(sec, "panel-frame-light", 340, 220, panel("light"), "Panel frame (light)", "1.0 exists (code)", "Flat, hairline rules.")
    save(sec, "panel-frame-night", 340, 220, panel("night"), "Panel frame (Night Atlas)", "1.0 design",
         "Dark slate tokens are not in the repo yet.")

    n = [f'<rect width="360" height="150" fill="{N_GROUND}"/>',
         f'<rect x="10" y="10" width="340" height="130" fill="{N_RAISED}" stroke="{N_LINE}"/>',
         f'<rect x="10" y="10" width="4" height="130" fill="{N_WARN}"/>',
         text(28, 38, "STRAIN  ·  MARCH 2008", 10, N_WARN, SANS, weight=600, extra='letter-spacing="2"'),
         text(28, 64, "Diesel shortfall in Andalucía", 17, N_INK, SERIF, weight=600),
         text(28, 86, "Refinery output fell 9 % below demand.", 12, N_INK2, SANS),
         text(28, 104, "Freight costs rose on 14 lanes.", 12, N_INK2, SANS),
         f'<rect x="28" y="114" width="70" height="18" fill="none" stroke="{N_INK2}"/>',
         text(63, 127, "Go to", 11, N_INK, SANS, "middle"),
         f'<rect x="106" y="114" width="70" height="18" fill="none" stroke="{N_INK2}"/>',
         text(141, 127, "Why", 11, N_INK, SANS, "middle"),
         text(336, 127, "pinned", 10, N_INK2, MONO, "end")]
    save(sec, "event-notice", 360, 150, "".join(n), "Event notice", "1.0 mock", "Paradox-style notice; severity rule at left.")

    cur = [f'<g stroke="{INK}" stroke-width="1.5" fill="none"><circle cx="16" cy="16" r="6"/><path d="M16 2v8M16 22v8M2 16h8M22 16h8"/></g>',
           f'<circle cx="16" cy="16" r="1.4" fill="{EDIT}"/>',
           f'<g transform="translate(40 0)" stroke="{INK}" stroke-width="1.5" fill="{PAPER}" stroke-linejoin="round">'
           '<path d="M10 14V7a2 2 0 014 0v6V5a2 2 0 014 0v8V6a2 2 0 014 0v8 0-4a2 2 0 014 0v8c0 6-4 10-9 10s-7-2-10-7l-3-5a2 2 0 013-2.5l3 3z"/></g>']
    save(sec, "cursors", 72, 32, "".join(cur), "Cursors: crosshair and pan", "1.0 mock", "")


# --------------------------------------------- 6 seals, flags, emblems, houses

def seals():
    sec = "seals"
    h = bytes.fromhex(SAMPLE_HASH)
    rs = [f'<rect width="240" height="240" fill="{PAPER}"/>',
          f'<circle cx="120" cy="120" r="110" fill="none" stroke="{FORK}" stroke-width="1.5"/>',
          f'<circle cx="120" cy="120" r="104" fill="none" stroke="{FORK}" stroke-width="0.6"/>']
    for ring in range(3):
        R = 60 + h[ring] % 40
        r = 7 + h[ring + 3] % 23
        d = 4 + h[ring + 6] % 30
        rs.append(f'<path d="{hypotrochoid(120, 120, R, r, d, 92 / (R - r + d))}" fill="none" stroke="{FORK}" '
                  f'stroke-width="0.45" opacity="{f(0.9 - ring*0.2)}"/>')
    rs.append(f'<path id="arc" d="M30 120 a90 90 0 0 1 180 0" fill="none"/>')
    rs.append(f'<text font-family="{MONO}" font-size="10" fill="{FORK}" letter-spacing="2"><textPath href="#arc" startOffset="50%" text-anchor="middle">{SAMPLE_HASH[:24].upper()}</textPath></text>')
    rs.append(f'<circle cx="120" cy="120" r="22" fill="{PAPER}" stroke="{FORK}"/>')
    rs.append(text(120, 125, "2000", 13, FORK, SERIF, "middle", 600))
    save(sec, "run-seal-guilloche", 240, 240, "".join(rs), "Run seal from hash", "1.0 pipeline",
         "Three hypotrochoids whose radii come from the run hash bytes. Same hash, same seal.")

    rb = [f'<circle cx="80" cy="80" r="70" fill="none" stroke="#2B4F8F" stroke-width="3"/>',
          f'<circle cx="80" cy="80" r="52" fill="none" stroke="#2B4F8F" stroke-width="1.5"/>',
          f'<path id="rb" d="M22 80 a58 58 0 1 1 116 0 a58 58 0 1 1 -116 0" fill="none"/>',
          f'<text font-family="{SANS}" font-size="11" fill="#2B4F8F" letter-spacing="2.2"><textPath href="#rb">MINISTRY OF ECONOMY · DEPARTMENT OF PLANNING · </textPath></text>',
          text(80, 76, "No.", 12, "#2B4F8F", SERIF, "middle"), text(80, 96, "0001", 16, "#2B4F8F", MONO, "middle")]
    save(sec, "seal-round-blue", 160, 160, "".join(rb), "Round blue office seal", "1.0 mock",
         "Generic shape; no real seal reproduced (ADR 0141).", bg=PAPER)
    ro = [f'<rect x="20" y="20" width="120" height="120" fill="none" stroke="#B3261E" stroke-width="5"/>',
          f'<rect x="30" y="30" width="100" height="100" fill="none" stroke="#B3261E" stroke-width="1.5"/>',
          f'<path d="M80 50 l6 18 h19 l-15 11 6 18 -16 -11 -16 11 6 -18 -15 -11 h19z" fill="#B3261E"/>',
          text(80, 122, "OFFICE SEAL", 10, "#B3261E", SANS, "middle", 600, 'letter-spacing="2"')]
    save(sec, "seal-red-office", 160, 160, "".join(ro), "Square red office seal", "1.0 mock", "", bg=PAPER)
    ps = [f'<ellipse cx="80" cy="80" rx="36" ry="52" fill="none" stroke="#C0392B" stroke-width="4"/>',
          f'<path d="M66 52 h28 M80 52 v56 M64 74 h32 M66 96 h28" stroke="#C0392B" stroke-width="5" stroke-linecap="square"/>']
    save(sec, "seal-personal", 160, 160, "".join(ps), "Personal seal", "1.0 mock", "Abstract strokes, not a real name.", bg=PAPER)

    lh = [f'<rect width="420" height="120" fill="#FBFBF7"/>',
          f'<rect width="420" height="34" fill="#2C1E23"/>', f'<rect y="34" width="420" height="3" fill="#AA151B"/>',
          text(16, 23, "MINISTERIO DE ECONOMÍA", 11, "#D48A8D", SANS, weight=600, extra='letter-spacing="1.8"'),
          text(404, 23, "Madrid, 14 de marzo de 2003", 11, "#D48A8D", SERIF, "end", extra='font-style="italic"'),
          text(16, 66, "Orden ECO/412/2003", 16, INK, SERIF, weight=600),
          text(16, 88, "Rebaja del tipo de IVA sobre gasóleo agrícola al 7 %", 12, INK2, SANS),
          f'<path d="M260 104 c10-14 20 6 30-6 s14-10 24 2 s16 -6 26 -2" stroke="{INK}" stroke-width="1.4" fill="none"/>',
          f'<line x1="250" y1="108" x2="400" y2="108" stroke="{INK}" stroke-opacity="0.3"/>']
    save(sec, "seat-letterhead-es", 420, 120, "".join(lh), "Seat letterhead (Spain chrome)", "1.0 mock",
         "ADR 0140 header and rule hexes; signature line for an Enact.")


def flags_emblems():
    sec = "flags"
    for name, label, body in (
        ("placeholder-horizontal", "Flag placeholder, horizontal",
         f'<rect width="150" height="100" fill="{LAND}"/><rect y="33" width="150" height="34" fill="{SEA}"/>'
         f'<rect width="150" height="100" fill="none" stroke="{INK}" stroke-opacity="0.25"/>'),
        ("placeholder-vertical", "Flag placeholder, vertical",
         f'<rect width="150" height="100" fill="{LAND}"/><rect x="50" width="50" height="100" fill="{SEA}"/>'
         f'<rect width="150" height="100" fill="none" stroke="{INK}" stroke-opacity="0.25"/>'),
        ("placeholder-unknown", "Flag fallback (no source)",
         f'<rect width="150" height="100" fill="{PAPER}"/>'
         + "".join(f'<path d="M{x} 0 l-100 100" stroke="{INK}" stroke-opacity="0.12" stroke-width="6"/>' for x in range(0, 260, 22))
         + f'<rect width="150" height="100" fill="none" stroke="{INK}" stroke-opacity="0.35" stroke-dasharray="5 4"/>'
         + text(75, 58, "?", 30, INK2, SERIF, "middle")),
    ):
        save(sec, name, 150, 100, body, label, "1.0 mock", "Real 2000 flags come from Commons per file.")

    em = [f'<path d="M80 22 h50 v52 c0 30-22 44-50 54 c-28-10-50-24-50-54 V22z" fill="{SEA}" stroke="{INK}" stroke-width="2.5"/>',
          f'<path d="M30 60 h100" stroke="{INK}" stroke-width="1.5"/><path d="M80 22 v106" stroke="{INK}" stroke-width="1.5"/>',
          f'<path d="M55 38 l5 10 11 1 -8 7 2 11 -10 -6 -10 6 2 -11 -8 -7 11 -1z" fill="{PAPER}" stroke="{INK}"/>',
          f'<path d="M92 95 q8 -14 16 0 q8 14 16 0" stroke="{INK}" stroke-width="2" fill="none"/>']
    for side in (-1, 1):
        for i in range(7):
            a = math.radians(200 + i * 20 if side < 0 else -20 - i * 20)
            x, y = 80 + 62 * math.cos(a), 84 - 62 * math.sin(a)
            em.append(f'<ellipse cx="{f(x)}" cy="{f(y)}" rx="4" ry="9" transform="rotate({f(math.degrees(-a)+90)} {f(x)} {f(y)})" fill="none" stroke="{OK}" stroke-width="1.4"/>')
    save("emblems", "placeholder-emblem", 160, 160, "".join(em), "Emblem placeholder", "1.0 mock",
         "Generic escutcheon in a laurel. Real emblems are the slowest sourcing item.", bg=PAPER)

    # Royal houses (2.0+): fictional arms only, by heraldic ordinaries.
    houses = [("house-per-pale", "House arms: per pale", "#2E4C7E", "#D9B44A",
               lambda a, b: f'<rect x="20" y="14" width="40" height="110" fill="{a}"/><rect x="60" y="14" width="40" height="110" fill="{b}"/>'),
              ("house-chevron", "House arms: chevron", "#7A1F2B", "#E7E2CF",
               lambda a, b: f'<rect x="20" y="14" width="80" height="110" fill="{a}"/><path d="M20 98 L60 50 L100 98 V78 L60 30 L20 78z" fill="{b}"/>'),
              ("house-bend", "House arms: bend with roundels", "#2F5D46", "#E7E2CF",
               lambda a, b: f'<rect x="20" y="14" width="80" height="110" fill="{a}"/><path d="M20 30 L36 14 L100 92 L100 116z" fill="{b}"/>'
                            f'<circle cx="78" cy="38" r="7" fill="{b}"/><circle cx="40" cy="96" r="7" fill="{b}"/>')]
    shield = "M20 14 H100 V70 C100 102 80 118 60 126 C40 118 20 102 20 70 Z"
    for name, label, a, b, fill in houses:
        body = (f'<defs><clipPath id="s-{name}"><path d="{shield}"/></clipPath></defs>'
                f'<g clip-path="url(#s-{name})">{fill(a, b)}</g><path d="{shield}" fill="none" stroke="{INK}" stroke-width="2.5"/>'
                f'<path d="M36 8 l8 -6 8 6 8 -6 8 6 8 -6 8 6 v6 H36z" fill="{WARN}" stroke="{INK}" stroke-width="1.2" transform="translate(-4 0)"/>')
        save("houses", name, 120, 140, body, label, "2.0+ deferred",
             "Fictional arms from ordinaries only. Real houses would use their recorded blazon.", bg=PAPER)


# --------------------------------------------------------- 7 lessons, covers

def covers():
    sec = "scenarios"
    specs = [("cover-accession", "Accession", "A large economy joins the trade club in 2001.", 3, N_BRANCH),
             ("cover-oil-shock", "Oil shock", "Crude doubles in eighteen months.", 7, N_WARN),
             ("cover-border-friction", "Border friction", "One border thickens tenfold.", 11, N_PROOF)]
    for name, title, sub, seed, acc in specs:
        c = [f'<rect width="640" height="360" fill="{N_GROUND}"/>']
        coast = blob(430, 180, 150, seed, amp=0.34)
        for k in range(5, 0, -1):
            c.append(f'<path d="{blob_path(offset(coast, 430, 180, k*14))}" fill="{N_SEA}" opacity="{f(0.2+0.1*(5-k))}"/>')
        c.append(f'<path d="{blob_path(coast)}" fill="{N_LAND}" stroke="{N_INK2}"/>')
        if seed == 11:
            c.append(f'<path d="M430 40 C420 120 450 220 430 330" stroke="{acc}" stroke-width="6" fill="none"/>')
        elif seed == 7:
            for i in range(9):
                c.append(f'<circle cx="{360+i*18}" cy="{170+8*math.sin(i)}" r="{3+i}" fill="{acc}" opacity="{f(0.25+0.07*i)}"/>')
        else:
            for i in range(7):
                a = -0.9 + i * 0.3
                c.append(f'<path d="M430 180 Q{f(430+120*math.cos(a))} {f(180+120*math.sin(a)-40)} {f(430+240*math.cos(a))} {f(180+240*math.sin(a))}" stroke="{acc}" stroke-width="1.6" fill="none" opacity="0.8"/>')
        c.append(f'<rect x="0" y="0" width="300" height="360" fill="{N_GROUND}" opacity="0.82"/>')
        c.append(text(32, 60, "SCENARIO", 12, acc, SANS, weight=600, extra='letter-spacing="3"'))
        c.append(text(32, 108, title, 40, N_INK, SERIF, weight=600))
        c.append(text(32, 140, sub, 14, N_INK2, SANS))
        c.append(text(32, 326, "1 JAN 2000  ·  SEED 0", 11, N_INK2, MONO, extra='letter-spacing="2"'))
        save(sec, name, 640, 360, "".join(c), f"Scenario cover: {title}", "1.0 artist",
             "Map half could be a generated thumbnail from the pack.")

    ls = [f'<rect width="320" height="220" fill="{PAPER}"/>',
          f'<path d="M50 30 V180 H290" stroke="{INK}" stroke-width="1.5" fill="none"/>',
          f'<path d="M70 50 C130 120 200 150 280 165" stroke="{INK}" stroke-width="2.5" fill="none"/>',
          f'<path d="M70 165 C140 150 200 110 280 45" stroke="{INK}" stroke-width="2.5" fill="none"/>',
          f'<path d="M70 140 C140 125 200 85 280 20" stroke="{EDIT}" stroke-width="2.5" fill="none" stroke-dasharray="6 4"/>',
          f'<circle cx="176" cy="122" r="4.5" fill="{INK}"/><circle cx="198" cy="106" r="4.5" fill="{EDIT}"/>',
          f'<path d="M176 122 L198 106" stroke="{EDIT}" stroke-width="1.5" marker-end=""/>',
          text(286, 170, "D", 13, INK, SERIF, extra='font-style="italic"'), text(286, 50, "S", 13, INK, SERIF, extra='font-style="italic"'),
          text(286, 24, "S′", 13, EDIT, SERIF, extra='font-style="italic"'),
          text(42, 34, "p", 13, INK, SERIF, "end", extra='font-style="italic"'), text(290, 198, "q", 13, INK, SERIF, "end", extra='font-style="italic"'),
          text(60, 210, "Lesson 2: a tariff moves the supply curve", 12, INK2, SANS)]
    save("lessons", "lesson-diagram-tariff", 320, 220, "".join(ls), "Lesson frame diagram", "1.0 optional",
         "One 320 px diagram per lesson at most (ADR 0158).")


# -------------------------------------------------------- 8 instrument charm

def instruments():
    sec = "instruments"
    st = [f'<circle cx="80" cy="80" r="14" fill="none" stroke="{INK}" stroke-width="2"/>',
          f'<path d="M80 80 A14 14 0 0 1 80 108 Z" fill="{INK}" transform="translate(0 -14)"/>',
          f'<path d="M80 66 L80 18" stroke="{INK}" stroke-width="2"/>',
          f'<path d="M80 18 l14 -6 M80 26 l14 -6 M80 34 l7 -3" stroke="{INK}" stroke-width="2"/>',
          text(58, 70, "3.1", 13, INK, MONO, "end"), text(58, 102, "1.8", 13, INK2, MONO, "end"),
          text(102, 70, "+0.4", 13, EDIT, MONO), text(102, 102, "92", 13, INK2, MONO),
          text(80, 148, "Province station model", 11, INK2, SANS, "middle")]
    save(sec, "station-model", 160, 160, "".join(st), "Station model", "1.0 idea 26-39",
         "Price, growth, rate change, capacity use in weather-plot positions; barbs = trade pressure.", bg=PAPER)

    wr = [f'<circle cx="80" cy="80" r="{r}" fill="none" stroke="{INK}" stroke-opacity="0.2"/>' for r in (20, 40, 60)]
    vals = [52, 20, 14, 30, 44, 10, 18, 60, 26, 12, 34, 22, 40, 16, 28, 48]
    for i, v in enumerate(vals):
        a0 = 2 * math.pi * i / 16 - math.pi / 2 - 0.16
        a1 = a0 + 0.32
        wr.append(f'<path d="M80 80 L{f(80+v*math.cos(a0))} {f(80+v*math.sin(a0))} A{v} {v} 0 0 1 {f(80+v*math.cos(a1))} {f(80+v*math.sin(a1))} Z" fill="{FORK if v > 40 else INK}" fill-opacity="{0.8 if v > 40 else 0.35}"/>')
    wr.append(text(80, 12, "N", 10, INK, SERIF, "middle"))
    save(sec, "trade-wind-rose", 160, 160, "".join(wr), "Trade wind rose", "1.0 idea 26-39",
         "Export value by bearing from the province; violet = top lanes.", bg=PAPER)

    pl = [f'<rect x="10" y="10" width="140" height="110" fill="#5E3B33"/>',
          f'<rect x="10" y="98" width="140" height="22" fill="{SEA}" opacity="0.8"/>',
          f'<circle cx="60" cy="72" r="20" fill="none" stroke="{PAPER}" stroke-width="3"/>',
          f'<line x1="34" y1="72" x2="86" y2="72" stroke="{PAPER}" stroke-width="3"/>']
    for i, (lab, y) in enumerate([("TF", 52), ("F", 60), ("T", 68), ("S", 76), ("W", 84)]):
        pl.append(f'<line x1="100" y1="{y}" x2="124" y2="{y}" stroke="{PAPER}" stroke-width="2.5"/>')
        pl.append(text(128, y + 3, lab, 8, PAPER, SANS))
    pl.append(f'<line x1="100" y1="46" x2="100" y2="88" stroke="{PAPER}" stroke-width="2.5"/>')
    pl.append(text(80, 140, "Fiscal headroom, load line", 11, INK2, SANS, "middle"))
    save(sec, "plimsoll-mark", 160, 150, "".join(pl), "Plimsoll mark", "1.0 idea 26-39",
         "Debt draft against the load lines for each season.", bg=PAPER)

    sl = [f'<rect x="10" y="40" width="220" height="40" rx="6" fill="#C9B36A" stroke="{INK}" stroke-width="1.5"/>',
          f'<rect x="70" y="50" width="100" height="20" rx="10" fill="#E8F0D2" stroke="{INK}"/>',
          f'<line x1="108" y1="50" x2="108" y2="70" stroke="{INK}"/><line x1="132" y1="50" x2="132" y2="70" stroke="{INK}"/>',
          f'<ellipse cx="142" cy="60" rx="9" ry="6" fill="#FFFFFF" stroke="{INK}" stroke-opacity="0.4"/>',
          text(120, 106, "Current account: 1.4 % off level", 11, INK2, SANS, "middle")]
    save(sec, "spirit-level", 240, 120, "".join(sl), "Spirit level", "1.0 idea 26-39", "Balance readout.", bg=PAPER)

    tr = [f'<rect width="180" height="180" fill="#E6D8BE"/>']
    r = 3.0
    widths = [6, 5, 7, 6, 4, 3, 2, 5, 6, 7, 6, 5, 6, 3, 4, 6, 7]
    for i, w in enumerate(widths):
        r += w * 0.8
        wob = [(90 + (r + 0.6 * math.sin(7 * t + i)) * math.cos(t), 90 + (r + 0.6 * math.sin(7 * t + i)) * 0.95 * math.sin(t))
               for t in [2 * math.pi * k / 90 for k in range(90)]]
        col = CRISIS if w <= 3 else "#7A5A3A"
        tr.append(f'<path d="{blob_path(wob)}" fill="none" stroke="{col}" stroke-width="{1.4 if w <= 3 else 0.9}"/>')
    tr.append(f'<rect x="0" y="162" width="180" height="18" fill="#E6D8BE" opacity="0.9"/>')
    tr.append(text(90, 175, "2000 to 2016, red = recession year", 9, INK2, SANS, "middle"))
    save(sec, "tree-rings", 180, 180, "".join(tr), "Tree rings", "1.0 idea 26-39", "One ring per simulated year; width = growth.")

    sc = [f'<rect width="320" height="140" fill="#F3F1E6"/>']
    for i in range(0, 320, 16):
        sc.append(f'<line x1="{i}" y1="0" x2="{i}" y2="120" stroke="{OK}" stroke-opacity="0.25"/>')
    for j in range(0, 120, 20):
        sc.append(f'<line x1="0" y1="{j}" x2="320" y2="{j}" stroke="{OK}" stroke-opacity="0.25"/>')
    pts = [(x, 60 - 18 * math.sin(x / 23) - 34 * math.exp(-((x - 190) / 9) ** 2)) for x in range(0, 300, 2)]
    sc.append(f'<polyline points="{poly(pts)}" fill="none" stroke="{EDIT}" stroke-width="1.6"/>')
    sc.append(f'<path d="M300 {f(pts[-1][1])} l14 -4 v8z" fill="{INK}"/>')
    sc.append(text(8, 136, "Strip-chart recorder: policy rate, 1 mm per month", 10, INK2, SANS))
    save(sec, "strip-chart", 320, 140, "".join(sc), "Strip-chart recorder", "1.0 idea 26-39", "")

    db = [f'<rect width="380" height="150" fill="#101418"/>', text(14, 24, "DEPARTURES  ·  PORT OF VALENCIA", 11, N_WARN, MONO, extra='letter-spacing="2"')]
    rows = [("0412", "ROTTERDAM", "CONTAINERS", "ON TIME"), ("0415", "PIRAEUS", "STEEL", "DELAYED"),
            ("0419", "TANGER MED", "VEHICLES", "ON TIME"), ("0422", "GENOA", "CERAMICS", "BOARDING")]
    for i, rw in enumerate(rows):
        y = 52 + i * 24
        db.append(f'<rect x="10" y="{y-15}" width="360" height="20" fill="#1A2026"/>')
        for x, s in zip((14, 64, 180, 296), rw):
            db.append(text(x, y, s, 12, N_CRISIS if s == "DELAYED" else "#F0E6C8", MONO))
    save(sec, "departures-board", 380, 150, "".join(db), "Departures board", "1.0 idea 26-39", "Port panel: sailings from the freight model.")

    co = [f'<rect width="300" height="110" fill="{PAPER}"/>',
          f'<line x1="20" y1="20" x2="280" y2="20" stroke="{INK}"/>', f'<line x1="20" y1="23" x2="280" y2="23" stroke="{INK}" stroke-width="0.5"/>',
          text(150, 44, "Printed by Ceteris 1.0 from run", 11, INK, SERIF, "middle", extra='font-style="italic"'),
          text(150, 62, SAMPLE_HASH[:32], 9, INK2, MONO, "middle"),
          text(150, 78, "Pack 0.9.2 · WDI, ICIO, GEBCO, Natural Earth · branch a7f3 · tick 37", 9, INK2, SANS, "middle"),
          f'<line x1="20" y1="90" x2="280" y2="90" stroke="{INK}" stroke-width="0.5"/>', f'<line x1="20" y1="93" x2="280" y2="93" stroke="{INK}"/>']
    save(sec, "map-colophon", 300, 110, "".join(co), "Map imprint / colophon", "1.0 idea 2", "Every export says what made it.")

    orr = [f'<rect width="200" height="200" fill="{N_GROUND}"/>']
    for r_, lab, ang in ((30, "tick", 0.3), (55, "month", 1.6), (78, "year", 3.9), (94, "decade", 5.0)):
        orr.append(f'<circle cx="100" cy="100" r="{r_}" fill="none" stroke="{N_LINE}"/>')
        x, y = 100 + r_ * math.cos(ang), 100 + r_ * math.sin(ang)
        orr.append(f'<circle cx="{f(x)}" cy="{f(y)}" r="5" fill="{N_INK}"/>')
        orr.append(text(x + 8, y + 3, lab, 9, N_INK2, MONO))
    orr.append(f'<circle cx="100" cy="100" r="8" fill="{N_WARN}"/>')
    save(sec, "orrery-clocks", 200, 200, "".join(orr), "Orrery of system clocks", "1.0 idea 5", "Each subsystem's period as an orbit.")


# ------------------------------------------------- goods (ICIO industries)

# (code, aggregate good index, label, body). Aggregates follow crates/ceteris-pack/src/econ/goods.rs.
GOODS_AGG = ["Agriculture", "Crude oil, gas and refined fuels", "Coal and other mining", "Food",
             "Light manufacturing", "Chemicals and metals", "Machinery, electronics and vehicles",
             "Electricity and utilities", "Construction", "Services"]
GOODS = [
    ("A01", 0, "Crops and livestock", '<path d="M12 21V7"/><path d="M12 9c-2.5-.5-3.5-2-3.5-4 2.5.3 3.5 1.8 3.5 4zM12 9c2.5-.5 3.5-2 3.5-4-2.5.3-3.5 1.8-3.5 4zM12 13c-2.5-.5-3.5-2-3.5-4 2.5.3 3.5 1.8 3.5 4zM12 13c2.5-.5 3.5-2 3.5-4-2.5.3-3.5 1.8-3.5 4zM12 17c-2.5-.5-3.5-2-3.5-4 2.5.3 3.5 1.8 3.5 4zM12 17c2.5-.5 3.5-2 3.5-4-2.5.3-3.5 1.8-3.5 4z"/><path d="M12 7V3.5"/>'),
    ("A02", 0, "Forestry", '<path d="M10 3l5 6.5h-2.5L16 14H4l3.5-4.5H5z"/><path d="M10 14v4"/><path d="M14 21h7M15.5 18.5h4v2.5h-4z"/><ellipse cx="17.5" cy="18.5" rx="2" ry="1"/>'),
    ("A03", 0, "Fishing", '<path d="M3 12c3-4.5 9.5-5 13 0-3.5 5-10 4.5-13 0z"/><path d="M16 12l5-4v8z"/><circle cx="7" cy="11" r=".8" fill="currentColor"/><path d="M10 9.5c1 1.5 1 3.5 0 5"/>'),
    ("B05", 2, "Coal", '<path d="M3 11h18l-2.5 7h-13z"/><circle cx="8" cy="19.5" r="1.5"/><circle cx="16" cy="19.5" r="1.5"/><path d="M5 11l2-3 2.5 1L12 6l3 2.5 2-1 2 3.5"/>'),
    ("B06", 1, "Crude oil and gas", '<path d="M7 21L11 3h2l4 18"/><path d="M9.3 13.5h5.4M10.2 8.5h3.6M8 18.5l7-5M16 18.5l-7-5"/><path d="M4 21h16"/>'),
    ("B07", 2, "Metal ores", '<path d="M3 17l3-8 6-4 7 3 2 8-6 4-8-1z"/><path d="M6 9l5 3 8-4M11 12l-1 7M11 12l10 4"/>'),
    ("B08", 2, "Quarrying", '<path d="M4 9c3-4.5 13-4.5 16 0"/><path d="M12 6.5L7 21"/><path d="M4 9l2 1M20 9l-2 1"/>'),
    ("B09", 1, "Mining support", '<path d="M3 17h18"/><path d="M5 17a7 7 0 0114 0"/><path d="M10 10.5V8h4v2.5"/><path d="M12 8V6"/>'),
    ("C10T12", 3, "Food, drink, tobacco", '<path d="M4 13a4.5 4.5 0 014-5h8a4.5 4.5 0 014 5v6H4z"/><path d="M8.5 10.5l1.5 3M12 10l1.5 3M15.5 10.5l1.5 3"/>'),
    ("C13T15", 4, "Textiles and clothing", '<path d="M6 4h8M6 20h8M7 4v16M13 4v16"/><path d="M7 8h6M7 11h6M7 14h6M7 17h6" stroke-opacity=".55"/><path d="M19 3v14l-1.5 3.5"/><circle cx="19" cy="5" r=".9"/>'),
    ("C16", 4, "Wood products", '<circle cx="7" cy="16" r="4"/><circle cx="17" cy="16" r="4"/><circle cx="12" cy="8" r="4"/><circle cx="7" cy="16" r="1.5"/><circle cx="17" cy="16" r="1.5"/><circle cx="12" cy="8" r="1.5"/>'),
    ("C17_18", 4, "Paper and printing", '<path d="M5 6h11v14H5z"/><path d="M8 4h11v14"/><path d="M7.5 10h6M7.5 13h6M7.5 16h4"/>'),
    ("C19", 1, "Refined fuels", '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M5 8h14M5 16h14"/><path d="M12 9.5c-1.5 2-2 3-2 3.8a2 2 0 004 0c0-.8-.5-1.8-2-3.8z"/>'),
    ("C20", 5, "Chemicals", '<path d="M9 3h6M10 3v6l-5.5 10a1.5 1.5 0 001.3 2h12.4a1.5 1.5 0 001.3-2L14 9V3"/><path d="M7 15h10"/><circle cx="11" cy="17.5" r=".8" fill="currentColor"/><circle cx="14" cy="16.8" r=".6" fill="currentColor"/>'),
    ("C21", 5, "Pharmaceuticals", '<path d="M10.5 4.5a4.95 4.95 0 017 7l-6 6a4.95 4.95 0 01-7-7z"/><path d="M7.5 7.5l7 7"/><circle cx="18.5" cy="18.5" r="2.5"/>'),
    ("C22", 5, "Rubber and plastics", '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="4"/><path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4L7 17M17 7l1.4-1.4"/>'),
    ("C23", 5, "Glass, cement, ceramics", '<path d="M3 20h18M3 15h18M3 10h18"/><path d="M3 10v10M21 10v10M9 10v5M15 10v5M6 15v5M12 15v5M18 15v5"/><path d="M8 7c0-2 1.5-3 4-3s4 1 4 3v3H8z"/>'),
    ("C24", 5, "Basic metals", '<path d="M3 20l2-4h6l2 4zM11 20l2-4h6l2 4zM7 15l2-4h6l2 4z"/><path d="M12 7V3M9.5 5L12 7l2.5-2"/>'),
    ("C25", 5, "Metal products", '<path d="M8 3.5h8l4 7-4 7H8l-4-7z" transform="translate(0 1.5)"/><circle cx="12" cy="12" r="3"/>'),
    ("C26", 6, "Electronics", '<rect x="6" y="6" width="12" height="12" rx="1.5"/><rect x="9.5" y="9.5" width="5" height="5"/><path d="M9 3v3M12 3v3M15 3v3M9 18v3M12 18v3M15 18v3M3 9h3M3 12h3M3 15h3M18 9h3M18 12h3M18 15h3"/>'),
    ("C27", 6, "Electrical equipment", '<path d="M9 3v5M15 3v5"/><path d="M6 8h12v3a6 6 0 01-12 0z"/><path d="M12 17v4"/>'),
    ("C28", 6, "Machinery", gear(10, 10, 7, 5.3, 8).join(['<path d="', '"/>']) + '<circle cx="10" cy="10" r="2.2"/>' + gear(18, 18, 4, 2.9, 6).join(['<path d="', '"/>'])),
    ("C29", 6, "Motor vehicles", '<path d="M3 16v-3l2-4.5h10l4 4.5h2v3z"/><path d="M7 8.5l-1 4.5M12 8.5v4.5M5 13h14"/><circle cx="7.5" cy="16.5" r="2" fill="#EDEFEA"/><circle cx="16.5" cy="16.5" r="2" fill="#EDEFEA"/>'),
    ("C30", 6, "Ships, rail, aircraft", '<rect x="6" y="3" width="12" height="14" rx="3"/><path d="M6 10h12"/><rect x="8.5" y="5" width="7" height="3" rx=".5"/><circle cx="9" cy="13.5" r="1"/><circle cx="15" cy="13.5" r="1"/><path d="M8 17l-3 4M16 17l3 4M7 20h10"/>'),
    ("C31T33", 4, "Furniture and other", '<path d="M7 3v18M7 12h10v9M7 15h10"/><path d="M7 3h9v9"/>'),
    ("D", 7, "Electricity", '<path d="M12 2L7 21M12 2l5 19M9 9h6M8 13.5h8M5.5 6h13M7.5 18L15.5 13M16.5 18l-8-4.5"/>'),
    ("E", 7, "Water and waste", '<path d="M4 8h8a3 3 0 013 3v1"/><path d="M4 5v6M12 5.5v2.5M9 5.5h6"/><path d="M15 16c-1.6 2.1-2.2 3.1-2.2 3.9a2.2 2.2 0 004.4 0c0-.8-.6-1.8-2.2-3.9z"/>'),
    ("F", 8, "Construction", '<path d="M6 21V4M3 21h7"/><path d="M6 4h15M6 7l3-3M10 4l-4 4M6 4l-3 3h3"/><path d="M18 4v6"/><rect x="16" y="10" width="4" height="3"/>'),
    ("G", 9, "Wholesale and retail", '<path d="M4 10v11h16V10"/><path d="M3 4h18l-1 6H4z"/><path d="M3 10c1.5 0 3-1 3-2.5C6 9 7.5 10 9 10s3-1 3-2.5c0 1.5 1.5 2.5 3 2.5s3-1 3-2.5c0 1.5 1.5 2.5 3 2.5" stroke-opacity=".7"/><path d="M10 21v-6h4v6"/>'),
    ("H49", 9, "Land transport", '<path d="M2 6h12v10H2zM14 9h4.5l3.5 4v3h-8"/><circle cx="6" cy="17.5" r="2" fill="#EDEFEA"/><circle cx="17.5" cy="17.5" r="2" fill="#EDEFEA"/>'),
    ("H50", 9, "Water transport", '<path d="M2 15h20l-3 5H5z"/><path d="M5 15V11h4v4M9 15V9h4v6M13 15v-3h4v3"/><path d="M19 15V7"/>'),
    ("H51", 9, "Air transport", '<path d="M21 4l-6 5.5L4 7 2.5 8.5 11 12l-3.5 4H5l-1 1.5 3.5 1 1 3.5L10 21v-2.5l4-3.5 3.5 8.5 1.5-1.5-2.5-11L22 5.5z"/>'),
    ("H52", 9, "Warehousing", '<path d="M3 21V9l9-5 9 5v12"/><path d="M7 21v-8h10v8M7 16h10M7 18.5h10"/>'),
    ("H53", 9, "Post and courier", '<rect x="3" y="5" width="18" height="14" rx="1.5"/><path d="M3 6.5l9 6.5 9-6.5"/>'),
    ("I", 9, "Hotels and restaurants", '<path d="M4 10h12v4a6 6 0 01-12 0z"/><path d="M16 11h1.5a2.5 2.5 0 010 5H16"/><path d="M8 3c-1 1.5 1 2.5 0 4M12 3c-1 1.5 1 2.5 0 4"/><path d="M3 22h14"/>'),
    ("J58T60", 9, "Publishing and broadcast", '<path d="M4 4h13v16H6a2 2 0 01-2-2z"/><path d="M17 8h3v10a2 2 0 01-2 2"/><path d="M7 7h7v4H7zM7 14h7M7 17h5"/>'),
    ("J61", 9, "Telecoms", '<path d="M12 10v11M9 21h6"/><circle cx="12" cy="8" r="1.5"/><path d="M8.5 4.5a5 5 0 000 7M15.5 4.5a5 5 0 010 7M5.6 2a9 9 0 000 12M18.4 2a9 9 0 010 12"/>'),
    ("J62_63", 9, "IT and information", '<rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M8 20h8M12 16v4"/><path d="M9.5 8l-2 2 2 2M14.5 8l2 2-2 2"/>'),
    ("K", 9, "Finance and insurance", '<path d="M3 9l9-5 9 5z"/><path d="M5 9v9M9.5 9v9M14.5 9v9M19 9v9M3 21h18M4 18h16"/>'),
    ("L", 9, "Real estate", '<path d="M3 11l9-7 9 7"/><path d="M5 9.5V20h8"/><circle cx="17" cy="15" r="2.5"/><path d="M17 17.5V21M17 19.5h2"/>'),
    ("M", 9, "Professional services", '<rect x="3" y="7" width="18" height="13" rx="1.5"/><path d="M9 7V5a1 1 0 011-1h4a1 1 0 011 1v2"/><path d="M3 12.5h18M11 12.5v2h2v-2"/>'),
    ("N", 9, "Administrative services", '<rect x="5" y="4" width="14" height="17" rx="1.5"/><path d="M9 3h6v3H9z"/><path d="M8.5 11l1.5 1.5 2.5-2.5M14 11.5h2M8.5 16l1.5 1.5 2.5-2.5M14 16.5h2"/>'),
    ("O", 9, "Public administration", '<path d="M3 21h18M5 21v-7h14v7M8 21v-4M12 21v-4M16 21v-4"/><path d="M6 14a6 6 0 0112 0"/><path d="M12 8V3l4 1.5L12 6"/>'),
    ("P", 9, "Education", '<path d="M2 9l10-5 10 5-10 5z"/><path d="M6 11v5c2 2 10 2 12 0v-5"/><path d="M21.5 9v6"/>'),
    ("Q", 9, "Health and social work", '<path d="M12 20s-8-4.6-8-10.5A4.5 4.5 0 0112 7a4.5 4.5 0 018 2.5C20 15.4 12 20 12 20z"/><path d="M5 12h3.5l1.5-3 2.5 6 1.5-3H19"/>'),
    ("R", 9, "Arts and recreation", '<path d="M9 18V5l11-2v13"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="17.5" cy="16" r="2.5"/><path d="M9 9l11-2"/>'),
    ("S", 9, "Other services", '<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M8.2 8.2L20 19M8.2 15.8L20 5"/>'),
    ("T", 9, "Household employers", '<path d="M14 3l-5 12"/><path d="M6 14l6 2.5-2 5.5-6-2.5z"/><path d="M5 17.5l5 2"/><path d="M16 21h5"/>'),
]


def goods():
    for code, agg, label, body in GOODS:
        save("goods", code.lower().replace("_", "-"), 24, 24, body, f"{code} {label}", "1.0 mock",
             GOODS_AGG[agg], icon=True)


# ------------------------------------------------------- 9 characters (2.0+)

def catmull(pts, closed=False) -> str:
    """Catmull-Rom through pts as cubic Beziers."""
    p = [pts[-1]] + pts + pts[:2] if closed else [pts[0]] + pts + [pts[-1]]
    d = [f"M{f(p[1][0])} {f(p[1][1])}"]
    for i in range(1, len(p) - 2):
        x0, y0 = p[i - 1]; x1, y1 = p[i]; x2, y2 = p[i + 1]; x3, y3 = p[i + 2]
        d.append(f"C{f(x1+(x2-x0)/6)} {f(y1+(y2-y0)/6)} {f(x2-(x3-x1)/6)} {f(y2-(y3-y1)/6)} {f(x2)} {f(y2)}")
    return " ".join(d)


def profile(v: dict) -> tuple[str, dict]:
    """Right-facing head and shoulders on a 200x200 card. v holds feature offsets."""
    fs, nl, nb, ch, lp, jw, cr, ag = (v[k] for k in ("forehead", "nose", "bump", "chin", "lips", "jaw", "crown", "age"))
    front = [(108, 26 - cr), (132, 31 - cr * 0.6), (146 + fs * 0.5, 46), (150 + fs, 62), (151, 72), (147, 79),
             (151 + nb, 85), (158 + nl, 95), (151, 100), (152 + lp, 105), (148, 109), (152 + lp, 113),
             (147, 118), (151 + ch, 126), (146 + ch * 0.5, 133 + ag * 0.4), (136, 138 + ag),
             (128, 142 + ag * 0.5), (126, 156), (132, 164)]
    back = [(46 - jw, 200), (52, 178), (74, 166), (90, 158), (92, 142), (82, 124), (74, 104), (72, 80), (78, 52), (92, 34)]
    shoulder = [(132, 164), (160, 172), (178, 186), (184, 200)]
    d = catmull(front) + " " + catmull(shoulder)[catmull(shoulder).index("C"):] + f" L{back[0][0]} {back[0][1]} " \
        + catmull(back)[catmull(back).index("C"):] + f" C100 28 104 26 {front[0][0]} {front[0][1]} Z"
    anchors = dict(ear=(103, 92), eye=(140, 80), nape=(90, 150), neck=(127, 150), chin=(150 + ch, 126),
                   brow=(150 + fs, 64), mouth=(148, 109))
    return d, anchors


def hair(style: str, v: dict, ink: str, paper: str) -> tuple[str, str]:
    """Returns (extra silhouette shapes in ink, cut lines in paper)."""
    cr, fs = v["crown"], v["forehead"]
    if style == "crop":
        return "", (f'<path d="M{f(146+fs*0.4)} {f(52)} C130 50 118 58 112 76 C108 88 100 96 92 118" stroke="{paper}" stroke-width="1.6" fill="none"/>'
                    + "".join(f'<path d="M{80+i*9} {44+abs(i-3)*4-cr} q8 6 10 18" stroke="{paper}" stroke-width=".9" fill="none"/>' for i in range(6)))
    if style == "bun":
        return (f'<circle cx="72" cy="58" r="17" fill="{ink}"/>',
                f'<path d="M148 50 C128 46 114 56 108 74 C104 86 98 98 90 116" stroke="{paper}" stroke-width="1.6" fill="none"/>'
                f'<path d="M62 50 q10 6 20 0 M60 60 q12 8 24 0" stroke="{paper}" stroke-width="1" fill="none"/>'
                + "".join(f'<path d="M{96+i*10} {36-cr} q10 4 16 16" stroke="{paper}" stroke-width=".9" fill="none"/>' for i in range(4)))
    if style == "long":
        return (f'<path d="M78 50 C62 80 58 130 66 176 L96 176 C92 150 92 128 98 112 Z" fill="{ink}"/>',
                f'<path d="M150 54 C134 44 116 52 110 70 C106 84 106 104 100 124 C96 140 94 158 96 176" stroke="{paper}" stroke-width="1.6" fill="none"/>'
                + "".join(f'<path d="M{74+i*6} {70+i*4} C{70+i*6} {110+i*4} {72+i*6} {140} {78+i*5} 172" stroke="{paper}" stroke-width=".8" fill="none"/>' for i in range(4)))
    if style == "curls":
        bumps = []
        for i in range(13):
            t = math.pi * (0.95 + i * 0.085)
            bumps.append(f'<circle cx="{f(108+44*math.cos(t))}" cy="{f(78+52*math.sin(t))}" r="10" fill="{ink}"/>')
        return ("".join(bumps),
                f'<path d="M146 50 C130 48 116 58 110 76 C106 90 100 100 92 112" stroke="{paper}" stroke-width="1.6" fill="none"/>'
                + "".join(f'<circle cx="{f(92+10*(i%4))}" cy="{f(44+10*(i//4))}" r="3" stroke="{paper}" stroke-width=".8" fill="none"/>' for i in range(8)))
    if style == "receding":
        return "", (f'<path d="M126 42 C116 52 112 64 110 78 C108 90 100 100 92 118" stroke="{paper}" stroke-width="1.6" fill="none"/>'
                    + "".join(f'<path d="M{82+i*7} {64+i*2} q4 10 2 20" stroke="{paper}" stroke-width=".9" fill="none"/>' for i in range(4)))
    if style == "cap":
        return (f'<path d="M80 50 C84 22 132 18 148 44 L172 50 L170 56 L144 54 C120 52 96 54 80 60 Z" fill="{ink}"/>',
                f'<path d="M82 56 C100 50 124 50 146 54" stroke="{paper}" stroke-width="1.6" fill="none"/>'
                f'<path d="M114 26 v26" stroke="{paper}" stroke-width=".9"/>')
    return "", ""


def face_details(v: dict, a: dict, paper: str, ink: str) -> str:
    ex, ey = a["eye"]
    out = [f'<path d="M{a["ear"][0]-4} {a["ear"][1]-8} c8 -2 10 12 2 18" stroke="{paper}" stroke-width="1.4" fill="none"/>',
           f'<path d="M{ex-4} {ey} q4 -3 8 0" stroke="{paper}" stroke-width="1.3" fill="none"/>',
           f'<path d="M{a["mouth"][0]-6} {a["mouth"][1]} h6" stroke="{paper}" stroke-width="1.1"/>']
    if v["age"] > 3:
        out.append(f'<path d="M{ex+2} {ey+6} q-4 5 -10 4 M{ex-14} {ey-2} q2 6 0 10" stroke="{paper}" stroke-width=".8" fill="none"/>')
        out.append(f'<path d="M{a["brow"][0]-14} {a["brow"][1]-8} h10 M{a["brow"][0]-15} {a["brow"][1]-4} h9" stroke="{paper}" stroke-width=".7"/>')
    if v.get("glasses"):
        out.append(f'<path d="M{ex-7} {ey-4} h14 v6 a3 3 0 01-3 3 h-8 a3 3 0 01-3 -3 z M{ex-7} {ey-2} L{a["ear"][0]+2} {ey-6}" stroke="{paper}" stroke-width="1.3" fill="none"/>')
    if v.get("beard"):
        cx, cy = a["chin"]
        out.append(f'<path d="M{a["ear"][0]+8} {a["ear"][1]+8} C112 120 124 128 {cx-8} {cy-6} C{cx-4} {cy-14} 146 112 140 108" stroke="{paper}" stroke-width="1.3" fill="none"/>')
        out.append("".join(f'<path d="M{118+i*6} {116+i*2} l2 7" stroke="{paper}" stroke-width=".7"/>' for i in range(4)))
    if v.get("moustache"):
        out.append(f'<path d="M140 103 q6 -3 12 1 q-6 4 -12 1z" fill="{ink}" stroke="{paper}" stroke-width=".8"/>')
    if v.get("earring"):
        out.append(f'<circle cx="{a["ear"][0]+2}" cy="{a["ear"][1]+16}" r="2.4" fill="{paper}"/>')
    return "".join(out)


def clothing(kind: str, paper: str, accent: str) -> str:
    if kind == "suit":
        return (f'<path d="M126 158 L136 200 M126 158 L114 178 L120 200 M90 160 L104 178 L100 200" stroke="{paper}" stroke-width="1.4" fill="none"/>'
                f'<path d="M126 160 l6 6 -4 6 6 28 -10 0 2 -28 -4 -6 z" fill="{accent}"/>')
    if kind == "open":
        return f'<path d="M128 158 L120 182 L106 162 M120 182 v18 M92 160 L106 162" stroke="{paper}" stroke-width="1.4" fill="none"/><circle cx="120" cy="190" r="1.2" fill="{paper}"/>'
    if kind == "turtleneck":
        return "".join(f'<path d="M{92+i} {146+i*3.5} Q110 {150+i*3.5} {128-i*0.3} {148+i*3.5}" stroke="{paper}" stroke-width=".9" fill="none"/>' for i in range(5))
    if kind == "uniform":
        return (f'<path d="M126 158 l-2 42 M94 160 c8 4 22 4 32 -2" stroke="{paper}" stroke-width="1.3" fill="none"/>'
                f'<path d="M60 176 h30 l2 6 h-32 z" fill="{accent}"/>'
                + "".join(f'<circle cx="123" cy="{172+i*9}" r="1.6" fill="{paper}"/>' for i in range(3)))
    if kind == "cardigan":
        return (f'<path d="M126 158 C116 172 110 184 112 200 M92 160 C100 170 108 176 112 186" stroke="{paper}" stroke-width="1.4" fill="none"/>'
                + "".join(f'<path d="M{60+i*8} 184 l2 10" stroke="{paper}" stroke-width=".7"/>' for i in range(6)))
    if kind == "scarf":
        return f'<path d="M92 152 C104 162 120 162 130 154 L134 166 C118 174 102 172 88 164 Z" fill="{accent}"/><path d="M118 166 l6 30 8 -2 -6 -28" fill="{accent}"/>'
    return ""


SITTERS = [
    # role, card colour, accent, hair, clothing, feature dict
    ("Minister of Finance", "#E9E4D6", "#AE2459", "crop", "suit", dict(forehead=1, nose=0, bump=2, chin=1, lips=0, jaw=0, crown=0, age=4, glasses=True)),
    ("Central bank governor", "#DCE4E0", "#5E45A3", "bun", "scarf", dict(forehead=-1, nose=-2, bump=0, chin=-1, lips=1, jaw=2, crown=2, age=2, earring=True)),
    ("Trade negotiator", "#E6DCD2", "#3F6E8C", "receding", "open", dict(forehead=3, nose=3, bump=4, chin=2, lips=-1, jaw=-2, crown=-2, age=6, beard=True)),
    ("Union leader", "#E2E0D2", "#B8403A", "curls", "turtleneck", dict(forehead=0, nose=1, bump=-1, chin=0, lips=1, jaw=4, crown=4, age=1)),
    ("Planning official", "#D9E1E6", "#B07A1E", "long", "cardigan", dict(forehead=-2, nose=-3, bump=-1, chin=-2, lips=1, jaw=-2, crown=1, age=0, glasses=True)),
    ("Chief of staff", "#E4DFE6", "#2F5D46", "cap", "uniform", dict(forehead=2, nose=1, bump=1, chin=3, lips=-1, jaw=6, crown=0, age=5, moustache=True)),
    ("Opposition leader", "#E9E1D0", "#5E45A3", "crop", "open", dict(forehead=-1, nose=4, bump=5, chin=1, lips=0, jaw=0, crown=1, age=3, beard=True, glasses=True)),
    ("Shipping magnate", "#DDE3D8", "#AE2459", "receding", "suit", dict(forehead=4, nose=2, bump=0, chin=-1, lips=0, jaw=8, crown=-3, age=9, moustache=True)),
]


def characters():
    sec = "characters"
    for i, (role, card, accent, style, cloth, v) in enumerate(SITTERS):
        d, a = profile(v)
        hx, cuts = hair(style, v, INK, card)
        body = [f'<rect width="200" height="220" fill="{card}"/>',
                f'<ellipse cx="104" cy="104" rx="84" ry="92" fill="none" stroke="{INK}" stroke-opacity=".35"/>',
                f'<ellipse cx="104" cy="104" rx="79" ry="87" fill="none" stroke="{INK}" stroke-opacity=".2" stroke-dasharray="1 3"/>',
                f'<defs><clipPath id="oval{i}"><ellipse cx="104" cy="104" rx="76" ry="84"/></clipPath></defs>',
                f'<g clip-path="url(#oval{i})">',
                f'<g transform="translate(3 3)" opacity=".18"><path d="{d}" fill="{INK}"/>{hx}</g>',
                f'<path d="{d}" fill="{INK}"/>{hx}{cuts}{face_details(v, a, card, INK)}{clothing(cloth, card, accent)}</g>',
                text(100, 212, role.upper(), 9, INK2, SANS, "middle", 600, 'letter-spacing="1.6"')]
        save(sec, f"silhouette-{i+1}", 200, 220, "".join(body), f"Silhouette: {role}", "2.0+ deferred",
             f"Generated: hair {style}, {cloth}; cut-paper detail. No likeness of a real person.")
    sig = [f'<rect width="300" height="90" fill="#FBFBF7"/>',
           f'<path d="M20 58 c12 -40 24 -20 18 0 s16 -30 28 -10 s10 10 20 -6 c8 -12 14 4 24 0 s20 -24 30 -6 c6 10 16 4 24 -4 q40 -10 96 4" stroke="{INK}" stroke-width="1.8" fill="none"/>',
           f'<line x1="20" y1="70" x2="280" y2="70" stroke="{INK}" stroke-opacity="0.3"/>',
           text(20, 84, "Minister of Economy, branch a7f3", 10, INK2, SANS)]
    save(sec, "signature", 300, 90, "".join(sig), "Generated signature", "2.0+ deferred", "Seeded stroke, unique per character.")
    st = [f'<g transform="rotate(-8 110 50)"><rect x="10" y="22" width="200" height="56" fill="none" stroke="{CRISIS}" stroke-width="3"/>'
          f'<rect x="16" y="28" width="188" height="44" fill="none" stroke="{CRISIS}" stroke-width="1"/>'
          f'{text(110, 56, "DOSSIER · RESTRICTED", 12, CRISIS, SANS, "middle", 600, "letter-spacing=\"1.5\"")}</g>']
    save(sec, "dossier-stamp", 220, 100, "".join(st), "Dossier stamp", "2.0+ deferred", "Intelligence file header.", bg=PAPER)


# ------------------------------------------------------- 10 store (deferred)

def store():
    sec = "store"
    for name, w, h, label in (("capsule-main-616x353", 616, 353, "Steam capsule, main"),
                              ("capsule-header-460x215", 460, 215, "Steam capsule, header")):
        s = h / 353
        c = [f'<rect width="{w}" height="{h}" fill="{N_GROUND}"/>']
        coast = blob(w * 0.68, h * 0.5, 150 * s, 13, amp=0.32)
        for k in range(5, 0, -1):
            c.append(f'<path d="{blob_path(offset(coast, w*0.68, h*0.5, k*12*s))}" fill="{N_SEA}" opacity="{f(0.2+0.1*(5-k))}"/>')
        c.append(f'<path d="{blob_path(coast)}" fill="{N_LAND}" stroke="{N_INK2}"/>')
        c.append(graticule(w * 0.68, h * 0.5, 210 * s, N_LINE, 1, 8, 6))
        c.append(f'<path d="M{f(w*0.5)} {f(h*0.95)} C{f(w*0.58)} {f(h*0.75)} {f(w*0.62)} {f(h*0.55)} {f(w*0.68)} {f(h*0.5)} C{f(w*0.75)} {f(h*0.4)} {f(w*0.85)} {f(h*0.25)} {f(w*0.98)} {f(h*0.2)}" stroke="{N_PROOF}" stroke-width="{f(4*s)}" fill="none"/>')
        c.append(f'<path d="M{f(w*0.68)} {f(h*0.5)} C{f(w*0.78)} {f(h*0.52)} {f(w*0.88)} {f(h*0.6)} {f(w*0.98)} {f(h*0.7)}" stroke="{N_BRANCH}" stroke-width="{f(4*s)}" fill="none" stroke-dasharray="{f(12*s)} {f(7*s)}"/>')
        c.append(text(36 * s, h * 0.48, "Ceteris", 64 * s, N_INK, SERIF, weight=600))
        c.append(text(38 * s, h * 0.48 + 56 * s, "Paribus", 64 * s, N_INK, SERIF, weight=600))
        c.append(text(40 * s, h * 0.48 + 92 * s, "THE WORLD FROM 2000. CHANGE ONE THING.", 14 * s, N_INK2, SANS, extra=f'letter-spacing="{f(2*s)}"'))
        save(sec, name, w, h, "".join(c), label, "deferred (P9)", "Outside the 1.0 exemption; sized for later.")


identity(); first_run(); icons(); goods(); map_furniture(); chrome(); seals(); flags_emblems(); covers(); instruments(); characters(); store()
(OUT / "manifest.json").write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + "\n")
print(len(manifest))
