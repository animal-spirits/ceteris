# Ceteris Paribus identity

Final logo kit, fixed by Antonio on 2026-11-13 (#567). Replaces the identity mocks in `../placeholder/app-icon` and `../mock/identity`.

- Logo: CETERIS PARIBVS in Cinzel 500, tracked 0.2 em, Latin V for U, a downward triangle as the stop between words.
- Day: satin Carrara marble, surface `#E9EBE8`, vein `#B2B8BA`.
- Night: satin granite, surface `#161C21`, cut `#C9D0CC`.
- Icon: the C with the triangle as its full stop.
- Everything outside the logo uses the ADR 0137 interface tokens; `tokens.json` lists both.

| Path | Contents |
|---|---|
| `carved/slab-{day,night}.png` | Stacked lockup, 1200x640 |
| `carved/horizontal-{day,night}.png` | One-line lockup, 1600x260 |
| `icons/icon-carved-{day,night}-{1024,512,256,128,64}.png` | Carved app icon |
| `icons/favicon-{day,night}-{32,16}.png` | Flat icon, transparent corners |
| `svg/wordmark-{stacked,horizontal}-{day,night}.svg` | Flat wordmarks, outlined |
| `svg/icon-*.svg`, `svg/favicon-*.svg` | Flat icon masters, outlined |

Use rules: carved art only at 400 px wide and up, flat vector below; carved icon at 64 px and up, flat at 32 and 16; no stone texture behind interface text.

Provenance: generated in-house. The carved images are rendered from a signed-distance V-groove height field lit by a raking light (numpy, scipy, Pillow); the vectors are Cinzel glyphs outlined with fontTools and HarfBuzz, so no font is needed to display them. The generator scripts live with the studio brand work, outside this repository.

Licence: own work. Glyph outlines derive from Cinzel by Natasha Raissa (SIL Open Font License 1.1).
