# Art mocks

105 SVG drafts for rows of `docs/art-inventory.md` that benefit from visual design, plus flavour pages (instrument charm, royal houses, characters, atlas details). `manifest.json` lists each file with label, status (1.0 mock, 1.0 artist, 1.x, 2.0+, deferred) and note.

Not wired into the app. Icons draw in `currentColor` (default ink `#1C292F`). Text uses Source Serif 4, IBM Plex Sans Condensed and IBM Plex Mono by name. Silhouettes, house arms and seals are invented; no real flag, emblem or seal is drawn.

Regenerate (byte-identical):

```
python3 scripts/gen_mock_art.py app/assets/mock
```
