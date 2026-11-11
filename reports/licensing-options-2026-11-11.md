# Licensing options for the simulator and the game (2026-11-11)

Not legal advice. A short check with a software or IP lawyer is recommended before the repository
goes public and again before 1.0, mainly on the licence text, the CLA and the trademark filing.

## Goals and the tension between them

Antonio's goals: (1) as many users as possible, (2) protect himself, (3) nobody else profits from
his work. Goals 1 and 3 conflict. Every OSI-approved open source licence allows commercial use, so
none of them can stop someone else profiting. A licence can only (a) force anyone who profits to
publish their changes (copyleft), (b) delay commercial competition (source-available, converting
later), or (c) forbid commercial use outright (non-commercial, not open source).

## Facts that constrain the choice

- Repo is private, no tags, no releases, `publish = false`; nothing has been distributed under
  MIT/Apache yet. Once the repo is public under MIT OR Apache-2.0, every copy made is MIT/Apache
  forever and cannot be withdrawn. The licence must be chosen and the LICENSE files changed
  before the flip to public.
- Contributors: only Antonio's two addresses and Claude (agent commits made for Antonio). No
  third-party contributions, so Antonio can relicense today without anyone's consent. Caveat:
  purely machine-written code may have weak or no copyright protection (US Copyright Office
  guidance), which weakens enforcement of any restrictive licence on those parts.
- Spec 19 and 22 record "Apache-2.0 / MIT, open source, non-commercial". Changing the licence is a
  deviation from the design, so it needs an RFC after Antonio decides.
- Data licences are independent of the code licence and do not change: users fetch data; nothing
  restricted is bundled. ETH EPR (CC BY-NC-SA 4.0) and other NC sources stay out of any paid build;
  EPR-derived files carry CC BY-NC-SA. EXIOBASE is validation-only with zero leakage and is never
  shipped. Wittgenstein (CC BY 4.0) ships anywhere with attribution.
- Slint is used under its royalty-free desktop licence (attribution required) or GPL-3.0. Both
  work with every option below; a closed game must use the royalty-free terms.
- Employer: list Ceteris as a prior invention and get outside-activity approval before starting
  at the new employer (Feb 2027). This matters more for protecting Antonio than the licence does.

## 1. Simulator licence

| Option | OSI open source | Stops others profiting | Adoption and academia |
|---|---|---|---|
| MIT OR Apache-2.0 (current) | Yes | No. Anyone may sell builds or embed it closed. | Highest. JOSS, distros, companies all fine. |
| AGPL-3.0-or-later | Yes | Partly. Commercial use allowed, but any distributed or hosted fork must publish its full source. Internal use (e.g. a fund running it privately) is not covered. | High. JOSS and academics fine; many companies avoid AGPL. Antonio, as sole owner, can still sell commercial licences and ship a closed game. |
| FSL-1.1 or BUSL-1.1, converting to Apache/MIT after 2 to 4 years | No (yes after conversion) | Partly. Bars competing commercial products for 2 to 4 years per release; internal commercial use allowed; old releases become fully free. | Medium. Unusual for research code; not eligible for JOSS until conversion. |
| PolyForm Noncommercial 1.0.0 | No ("source-available") | Yes. All commercial use barred; companies must buy a licence from Antonio. Universities, charities, governments, individuals allowed. | Medium-high among researchers and hobbyists; excluded from JOSS, Linux distros and most open source grants; fewer outside contributors. |

Recommendation: **AGPL-3.0-or-later**, with commercial licences sold on request. It keeps the
"open source" label (adoption, JOSS, citations, grants), deters closed commercial capture, and
leaves Antonio free to ship a paid closed game. Choose PolyForm Noncommercial instead if blocking
every commercial use, including private internal use, matters more than the open source label.
Simulation outputs (results, charts, papers) are not restricted by any of these licences.

## 2. Game licence and split

- Repo: keep the simulator public; put 2.0 game mechanics in a separate private repo that depends
  on the simulator crates. Same-repo works only if both share one licence.
- Game code: proprietary, sold under the Steam EULA. Source-available game code adds little
  adoption and makes cloning a paid product easy.
- Art, music, logos: licence separately from code, all rights reserved, with redistribution allowed
  only inside unmodified official builds. Commissioned art needs a written assignment or
  work-for-hire contract (art decisions still pending). Noto fonts (OFL) are fine anywhere.
- Name: a trademark policy (TRADEMARKS.md) protects "Ceteris Paribus" and the logo without
  restricting the code: forks may use the code but not the name or logo. Do a clearance search;
  "ceteris paribus" is a common phrase, so registration (USPTO, class 9 and 41) may need the logo
  or a distinctive form. This is the cheapest strong protection against someone selling a copy.

## 3. Contributor policy

Before accepting any outside pull request, require a CLA that grants Antonio a broad licence
(not an assignment) including the right to relicense and sell commercially. Without it, AGPL or NC
contributions would block the closed game and commercial licences. DCO alone certifies origin but
does not grant relicensing rights. Fallback: accept no outside code until 1.0. A CLA bot needs a
`.github/` change, which Antonio makes himself.

## Next steps after the decision

1. RFC amending spec 19 and 22 licence rows; update LICENSE files, `Cargo.toml` `license`
   fields, `deny.toml` and README before the repo goes public.
2. TRADEMARKS.md and CONTRIBUTING.md (CLA text); asset licence file.
3. Lawyer review of the three texts; prior-invention listing with the employer.
