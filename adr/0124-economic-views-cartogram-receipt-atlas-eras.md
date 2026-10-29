# 0124. Economic views: trade-cost cartogram, receipt, atlas and diegetic eras

Status: proposed; amended by ADR 0239 (game items to 2.0+)
Date: 2026-10-28
Design sections: 5.4, 8.3, 20.1, 20.2; ADR 0088
Phase: P3 (cartogram, receipt, atlas); 1.x (eras)

## Context

Map modes show levels on geographic space. Some of the model's best stories are about distance
and provenance, which geography hides. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 14, 15, 17 and 18).

## Options

1. Status quo.
2. Four views, all read-only: a trade-cost cartogram, a household receipt, a printable atlas
   and era-dependent UI chrome.

## Decision

Option 2. None writes state.

- Cartogram: place provinces by multidimensional scaling of generalized trade cost (5.4)
  instead of coordinates, with a smooth morph between geographic and economic space. Animated
  over time, the world contracts as costs fall and tears at closures.
- Receipt: for a province, one representative household's monthly basket, each line a good with
  price and a provenance chain through trade flows (origin shares and main route). After a
  shock, lines show the change and its decomposition.
- Atlas: `ceteris atlas` renders a run to PDF (maps, ledgers, chronicle excerpts, fan charts),
  reproducible from the run hash.
- Diegetic eras: app chrome themes keyed to the simulated date. Cosmetic, off by default.

## Migration

- P3: cartogram, receipt and atlas in the app and CLI. 1.x: era themes.

## Tests

- Rendering any view changes no state hash.
- Atlas output is byte-identical for the same run hash and app version.

## Consequences

The trade core gets views that show what it is about. Era themes are optional polish.
