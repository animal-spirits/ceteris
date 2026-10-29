# 0220. Localization pipeline and 1.0 languages

Status: proposed
Date: 2026-10-28
Design sections: 20; ADRs 0078, 0088, 0140, 0141, 0143, 0158
Phase: App shell (pipeline), R (English strings complete)

## Context

ADRs 0078, 0140, 0143 and 0158 assume Fluent files and refer to "every game language", which no
ADR defines. Slint has its own translation path (`@tr` with gettext catalogues), so the two could
both end up in the app. Number, date and currency formats, scripts beyond Latin, right-to-left
layout and the translation workflow are open. Audit gap 13.

## Options

1. No pipeline until a second language is wanted.
2. English only in 1.0, with every user-facing string through one pipeline from the App shell on.
3. Several languages in 1.0.

## Decision

Option 2.

- **One pipeline: Fluent.** Slint markup holds no literal user-facing text; Rust supplies every
  string to Slint through a strings model resolved from Fluent. Slint's `@tr` is not used. Fluent
  keeps the grammar ADRs 0078 and 0143 need.
- **Game languages** are the shipped Fluent locales. 1.0 ships `en`.
- **Formats:** numbers, dates and currencies through ICU4X (Unicode licence, already allowed) with
  the locale's rules.
- **Scripts:** fonts with Latin, Greek and Cyrillic in 1.0; other scripts and right-to-left layout
  arrive with the first language that needs them.
- **Workflow after 1.0:** a hosted translation platform with a free plan for open-source projects,
  once the repository is public (ADR 0218). State names keep ADR 0143's CLDR source.

## Migration

- App shell: strings model, Fluent loading, the lint below.
- ADRs 0140, 0143, 0158: "every game language" means the shipped locales.

## Tests

- A lint fails on a string literal in Slint markup outside an allowlist (icons, units).
- A pseudo-locale (accented, 30% longer) renders every panel without clipping.

## Consequences

Adding a language later is translation work, not refactoring. The strings model costs some
boilerplate in every panel.
