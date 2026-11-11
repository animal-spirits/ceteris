# 0172. Bank forbearance and trade finance

Status: proposed
Date: 2026-10-28
Design sections: 8.4, 8.5, 9.4, 15.5; ADR 0092
Phase: P5 (trade finance opt-in if its ablation is unclear)

## Context

Banks lend and absorb losses (8.5), but they have no rollover choice: a loan to an insolvent
firm is written off on schedule. Weak Japanese banks in the 1990s and weak euro-area banks after
2012 kept such loans alive, which depressed investment and entry by healthy firms (Caballero,
Hoshi and Kashyap 2008; Acharya, Eisert, Eufinger and Hirsch 2019). Cross-border trade needs no
financing in the model either. Exports by firms tied to troubled banks fell more in Japan's
crisis (Amiti and Weinstein 2011), and letters of credit are used more for risky destinations
(Niepmann and Schmidt-Eisenlohr 2017). Both channels bear on the 2008 crisis case of 15.5.
Gap audit (2026-10-28)

## Options

1. Status quo.
2. Forbearance only.
3. Forbearance and a trade-finance wedge, each switchable.

## Decision

Option 3, with each part as its own module so the ablation rule (15.7) decides each.

- Forbearance: a bank whose capital ratio is near the regulatory minimum rolls over a loan to
  an insolvent borrower when recognizing the loss would breach the minimum. The rolled-over
  firm keeps its share of inputs and labour. Supervisory strictness (9.4) is the lever. One
  register entry: the capital buffer at which forbearance starts.
- Trade finance: shipments on a cross-border link carry a cost wedge that rises with the
  destination's risk premium and falls with the exporter's bank capital. The share of trade on
  letters of credit is data (SWIFT shares published by Niepmann and Schmidt-Eisenlohr), not a
  parameter.

## Migration

- P5: a rollover rule in the bank step; a wedge on the trade kernel's link costs.

## Tests

- With both modules off, golden hashes are unchanged.
- A toy undercapitalized bank produces lower entry and investment by healthy firms in its
  province than a well-capitalized twin.
- With trade finance on, a bank-capital shock lowers exports of its borrowers by more than
  their domestic sales, with the ratio of Amiti and Weinstein.

## Consequences

Japan- and euro-style zombie stagnation becomes a mechanism, and the 2008-09 trade collapse
gains a finance channel next to ADR 0170's inventories. Register: 1-2 entries.
