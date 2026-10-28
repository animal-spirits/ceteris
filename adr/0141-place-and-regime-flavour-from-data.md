# 0141. Place and regime flavour from data

Status: proposed
Date: 2026-10-28
Design sections: 20; ADRs 0049, 0050, 0065, 0078, 0137, 0140
Phase: P3 (scripts, calendars, formats), P8 (seasons, night lights), P5-P6 (letterhead, party colours)

## Context

The owner asked for subtle details that differ by country, culture, region, religion, form of
government and ideology, to enrich each run. Without artists, details must come from datasets
and conventions, and must not turn identity into costume. Proposal section "Flavour":
https://claude.ai/artifact/JMufbLx4Yqabwd95LvFZVQ

## Options

1. None beyond the chrome of ADR 0140.
2. Hand-drawn culture packs (portraits, borders, ornaments). Artist cost and caricature risk.
3. Details drawn only from real data, each with a named source.

## Decision

Option 3. All are views; none touch state.

- Names in their own script: map labels show the English name with the local name beneath it
  (Москва, القاهرة, 東京), from Natural Earth and GeoNames name fields; fonts from the Noto
  families (ADR 0137).
- Calendars and formats: the date bar shows a second line in the seat's civil or customary
  calendar (Hijri, Solar Hijri, Hebrew, Buddhist era, Ethiopian, Japanese era) and numbers,
  currency and dates follow CLDR through ICU4X.
- Seasons and night lights: monthly snow cover and vegetation greenness from MODIS climatology
  on the relief; in the dark theme, cities glow with night lights scaled from simulated output.
- Letterhead: the seat panel carries the real emblem (Wikimedia Commons, licence checked per
  file) and the official name; documents differ by regime class (a bill with vote counts, a
  numbered decree, a communiqué), per ADR 0140.
- Register of the news: the chronicle (ADR 0078) words the same event by regime class and
  narrator.
- Statistical fog: ADR 0065's published figures show as a regime detail (stamp, halo, night-
  lights estimate beside them).
- Party colours: real colours per party and country (red is the left in Europe, the right in the
  United States), from ParlGov and Wikidata, curated for large countries first.
- Music: regional open-licence music and the synth's regional flavour (ADR 0137).
- Local standards (owner, 2026-10-28), all following the seat and the game date:
  - Numbers, money and dates from CLDR via ICU4X: grouping (lakh and crore, 万 and 亿),
    optional native digits, currency position, percent spacing, accounting negatives in
    parentheses where conventional, long and short scale in words (billón, Milliarde).
  - Clock and week: 12- or 24-hour time, the capital's UTC offset (India +5:30, Nepal +5:45,
    one zone for China), first day of the week, weekend days by date (Saudi Arabia Thursday-
    Friday until June 2013), national holidays marked on the date bar.
  - Units and accounts: imperial units where used, local land units beside hectares (acres, mu,
    bigha, tsubo, sotka), the fiscal year start in budget panels.
  - Finance charts: rises in red where that is the convention (China, Japan, Korea, Taiwan),
    with a setting to override.
  - Map: disputed boundaries from the seat's point of view (Natural Earth worldview sets), a
    Pacific-centred world view for East Asian and Oceanian seats, route shields in local
    signage style (checked against each sign manual), traffic and trade particles on the local
    driving side, local names for administrative levels.
  - Documents: A4 or US Letter proportions; a signature, a personal seal (Japan), a red office
    seal (China) or a round blue seal (Russia); right-to-left layout for Arabic, Persian and
    Hebrew; local quotation marks; optional vertical setting for Chinese and Japanese place
    names; ministry names as they stood on the date.
  - People: name order and customs (family name first, patronymics, two surnames, nasab,
    academic titles) for generated and real names.
  - Ballots: the local voting mark on results cards (cross, stamp, filled oval, handwritten
    name, indelible ink), or an honest note where no competitive national ballot exists.
- Excluded: ethnicity or religion as a visual skin, costume portraits, faith symbols as themes,
  exoticizing ornament. Religion may appear only where the data already carries it (an emblem,
  a calendar in civil use).

## Tests

- No flavour layer changes a state hash.
- Every asset in the flavour manifest has a source and a licence entry that ADR 0050 accepts.

## Consequences

Places become recognisable at a glance from sources the project already uses or can fetch. The
cost is about 5-8 part-time weeks across the pack pipeline, spread over P3-P8; the curation of
emblems and party colours is the slowest part.
