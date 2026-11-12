# Open-source repo plus low-cost Steam build

2026-11-05. Research, not legal advice. Items marked **[lawyer]** need one.

## Short answer

The plan works, and most of it is already in place. The code is MIT OR Apache-2.0
(`Cargo.toml`, `LICENSE-*`). Those are OSI licences that Valve names as Steam-compatible. The 24
sources the current pack actually fetches (`fetch.toml`) are all `commercial_use = true` in
`sources.toml`: public domain, CC BY, Etalab 2.0 or OECD terms. That makes two tracks:

- **GitHub (free):** code plus fetch scripts, with no binaries, data or music. A script pulls the
  best free data, NC sources included, and free music onto the user's machine after the user
  accepts each licence. A script is simpler than an in-game menu and fine for this audience.
- **Steam (paid, about $10 to $15):** a signed binary plus a bundled data pack built only from
  commercial-OK sources, plus CC0/CC BY or commissioned music, auto-updates and Workshop
  scenarios. The paid build neither ships NC data nor contains code that fetches it.

The one real conflict is non-commercial (NC) data. It belongs in the free track only.

## 1. Data licences per source (from `sources.toml`)

| Group | Sources | Free repo (user fetches) | Paid Steam build |
|---|---|---|---|
| Public domain / CC0 | Natural Earth, ETOPO, USGS, EIA, NGA WPI, OurAirports, ISIMIP, Kummu GDP, SWIID, ParlGov, BMR, ATOP 3, GMSD, CIA Factbook | yes | **bundle** |
| CC BY / Etalab / OECD terms | OECD ICIO, BACI, CEPII Gravity, PWT, WDI, WB DTA, DOSE, WorldPop, GHSL, MapSPAM 2000, GEM, Maddison, ILOSTAT, WIC, UN WPP (BY-IGO), ADB KIDB, OECD BTiGE key | yes | **bundle** with attribution screen and NOTICE file |
| Share-alike | OSM (ODbL), V-Dem/V-Party (BY-SA) | yes | bundle derived tables as **separate artifacts** under their own licence (already the policy). V-Dem cannot share a derived table with EPR. |
| NC or permission needed (planned fallbacks and validation) | EPR/GeoEPR (BY-NC-SA, confirmed by ETH 2026-10-27), GAEZ v4, EM-DAT, Global Flood DB, WTO IDB/CTS, UNCTAD TRAINS, IMF, BIS, Laeven-Valencia, GDL SHDI, COW, Pink Sheet; from the 10-27 audit also IEA, IRENA, ITU, Freedom House, EXIOBASE 3.9+, FRED | yes, by script, with licence acceptance | **substitute or exclude.** User-side fetch from inside a paid product is a grey area **[lawyer]**. ETH's consent was given for a non-commercial project. |
| No licence stated | IRR regimes, WID, VIEWS, ATOP 5, about 20 academic sets | fetch only, private use | exclude unless the author grants a licence by email (as Wittgenstein did) |

**What blocks a paid build entirely:** nothing in the current pack. Later, the paid build loses
the ethnic layer (EPR) unless ETH sells a commercial licence, along with bilateral tariffs
(WTO/TRAINS, which have no open equivalent), wealth distribution (WID), the IMF policy rates and
WEO benchmark, and the hazard-impact validation (EM-DAT). Each of these needs a commercial
licence, an open substitute (the existing `reason` fields already name some: SWIID, GMSD,
Cline, ISIMIP), or must be dropped from the paid build. Validation-only sources do no harm when
absent from the paid build.

**Mechanism:** `ceteris pack check-sources --profile release` already sorts sources by licence.
The Steam pipeline should build the pack with `release` and fail if any `commercial_use = false`
source is reached. `[project] distribution = "closed"` must flip to `"open"` before the first
public release, which forces the `closed_source_only` cleanup.

## 2. Downloader: script or in-game menu

Shipping URLs and fetch code is not redistribution. Each user downloads under the provider's terms
and becomes the licensee. Risks, by source terms:

- **Automated-download bans:** IMF bars bulk automated download, WTO bars mass downloads,
  TRAINS needs a login, WIID returns 403 to scripts, and GEM and EM-DAT go through forms. These
  stay `manual = true`: the script prints the page, the user saves the file, and the script checks
  its hash. Do not work around a login or a form; that is the scraping risk.
- **Acceptance flow:** for each NC or unlicensed source, show the licence URL and a one-line
  summary, then require an explicit `--accept <id>` or a typed "yes". Log the acceptance locally.
  Ship a default set that is commercial-OK only, with NC sources opt-in.
- **Attribution:** generate `ATTRIBUTION.md` from `sources.toml` (licence, citation, "modified"
  note). CC BY and EPR require this; the IFPRI and GEM notes already give the text.
- **Rate limits:** fetch once, cache with hashes (as `fetch.toml` already does), send a plain
  User-Agent, no parallel hammering.
- **Script vs menu:** a script (`ceteris data fetch`, `ceteris music fetch`) is simpler and
  UNIX-shaped, and it keeps NC-fetch code out of the Steam binary. Add a menu later only for the
  free build, if users ask.

**Music the user may fetch or supply:**
- The user's own files: a `music/` folder the game scans. This is the OpenRCT2/OpenMW model, where
  the user supplies owned assets (OpenRCT2 requires original RCT2 files; OpenMW requires
  Morrowind's data).
- Free libraries: CC0/CC BY tracks from OpenGameArt and Kevin MacLeod (CC BY), plus Musopen
  public-domain recordings. A public-domain composition is not a public-domain recording; check
  the recording licence.
- Avoid the Jamendo API. Its terms bar apps built to cache content or offer offline access, and
  commercial use needs a quote. Free Music Archive tracks carry per-track licences, many of them
  NC.
- **Precedent: OpenTTD** ships free replacement sets (OpenGFX/OpenSFX/OpenMSX) and an in-game
  content service (BaNaNaS) with a licence per item. It uses original TTD files if the user owns
  them.

**Paid build music:** ship only CC0/CC BY tracks or commissioned tracks with a written licence.
NC music in a paid build has the same grey area as NC data.

## 3. Licence and open-core structure

- **Keep MIT OR Apache-2.0.** It is OSI open source, Valve lists both as compatible with the
  Steamworks SDK, it matches the Rust ecosystem, and it needs no CLA. Contributions arrive under
  the same licence (inbound = outbound, which Apache section 5 covers), and that is enough to sell
  builds.
- **A non-commercial code licence (e.g. PolyForm Noncommercial) is not open source.** It fails
  OSD clause 6 and must be called "source-available". It would not stop Antonio selling, since he
  holds the copyright, but outside contributors would need a CLA so he can sell their code.
- **GPL/AGPL** is an option if the goal is to stop closed forks. Valve says copyleft conflicts
  with the Steamworks SDK unless the author grants an exception. As copyright holder Antonio can
  grant one, but contributors' code then needs a CLA. Mindustry (GPL-3.0) does this.
- **The real moat is the name.** Under MIT anyone may upload a build to Steam. A registered
  trademark on "Ceteris Paribus" plus a trademark policy (forks rename) stops that
  **[lawyer: registrability, classes, cost]**.
- **Data licences stay separate from the code licence.** Derived data files carry their own
  licences (ODbL, BY-SA, and BY-NC-SA for EPR) and never MIT.
- **What the Steam build adds without conflict:** a signed binary for each OS, a prebuilt
  commercial-OK data pack (hours of GEOS and tippecanoe work saved), bundled free music, auto-updates,
  cloud saves, Workshop scenarios and mods, achievements later, and a support channel. Do not
  hold back features (Cataclysm DDA promises no premium features).
- **Employment [lawyer/HR]:** a paid product is an outside business activity. The Brevan Howard
  outside-activity approval and the prior-invention listing (from the 10-27 memo) should name
  Ceteris and the Steam sale before Feb 2027.

## 4. Precedents

| Game | Licence | Steam | Notes |
|---|---|---|---|
| Mindustry | GPL-3.0 code | paid, €9.99, about 22k reviews, 94% positive | free builds on GitHub and itch; compiling yourself barely dents sales |
| Cataclysm: DDA | CC BY-SA | paid (2023) | revenue funds a developer full-time; no premium features; free version stays |
| Shattered Pixel Dungeon | GPL-3.0 | paid | same model |
| OpenTTD, Wesnoth | GPL | free | Steam used for reach only |

Pattern: people pay for convenience, updates, Workshop and supporting the developer. "Piracy by
compile" is negligible for games that need a toolchain. For Ceteris the pack build (GEOS,
tippecanoe, about 2 h) is a stronger moat than compiling. The real risk is a third party
reselling a build on Steam, which the trademark covers. Mindustry's sales can be estimated at
about 0.7 to 1.3M from the 30 to 60 sales-per-review rule of thumb, an unverified heuristic.

## 5. Steam logistics

- **Fee:** $100 per app, recouped once the app has $1,000 in adjusted gross revenue. Valve takes
  30% up to $10M, 25% to $50M and 20% above (press reports).
- **Tax interview:** W-9 or W-8BEN depending on residency at signup. A Brazil-to-US move changes
  this **[accountant]**. Verification takes up to 30 days.
- **Timeline:** a 21-day wait after paying the fee for early titles, and a public coming-soon
  page for at least 2 weeks. The review checks that the game launches on every listed OS and
  that every listed feature works. In-game purchases must use Steam Wallet, and the store
  description may not contain external links.
- **External data at first run:** the review pages I read do not forbid it, but a paid build
  whose core data must be fetched fails the "listed features work" bar in spirit and invites
  refunds. Bundle the release pack in the depot. Optional extras (NC layers) stay in the free
  track.
- **AI disclosure:** the content survey asks for player-facing AI content (art, sound,
  narrative, localization) in detail. Efficiency tools "are not the focus", but disclose AI-assisted
  code briefly to be safe. Any AI-generated text or art in-game must be declared. Third-party
  data: you warrant that you hold the rights (Distribution Agreement), which is the reason for the
  `release` profile.
- **Refunds:** 14 days and under 2 h of playtime. A dense simulator is a refund risk if the first
  hour is setup. Ship a fast default scenario, and use Early Access with a clear "lab, not game"
  page.
- **Price:** comparable open-source paid titles sell at $7 to $15. Suggest $9.99 to $14.99 at
  1.0, or Early Access at $9.99.

## 6. Decisions for Antonio

1. **Code licence:** keep MIT OR Apache-2.0 (recommended), or move to GPL plus a CLA.
2. **Trademark:** register "Ceteris Paribus" and publish a fork-rename policy (recommended)
   **[lawyer]**.
3. **When to sell:** paid Steam at 1.0 alongside the free repo (this memo), or free 1.0 on Steam
   and paid 2.0 (earlier plan).
4. **Price:** $9.99 or $14.99, and Early Access or not.
5. **NC data in the paid build:** none (recommended), or ask ETH (EPR), WTO and WID for
   commercial terms or permission.
6. **Downloader form:** a script only (recommended, per your message), or a script plus an
   in-game menu in the free build later.
7. **Music:** a user folder plus a CC0/CC BY fetch for the free build, and commissioned or CC BY
   tracks for Steam.
8. **Employer approval:** list Ceteris and the Steam sale in the outside-activity filing.

## Sources

- Repo: `crates/ceteris-pack/config/sources.toml`, `fetch.toml`, `Cargo.toml`; memory notes
  epr-data-licence, licence-check-2026-10-27, monetization-options-2026-10-27.
- Steam: [open source on Steam](https://partner.steamgames.com/doc/sdk/uploading/distributing_opensource),
  [app fee](https://partner.steamgames.com/doc/gettingstarted/appfee),
  [onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding),
  [review process](https://partner.steamgames.com/doc/store/review_process),
  [content survey / AI](https://partner.steamgames.com/doc/gettingstarted/contentsurvey),
  [refunds](https://store.steampowered.com/steam_refunds/),
  [revenue tiers](https://techcrunch.com/?p=1753729).
- Precedents: [Cataclysm Steam FAQ](https://cataclysmdda.org/steam_faq.md),
  [Mindustry store data](https://www.wasdland.com/game/mindustry-1127400/),
  [OpenTTD on Steam](https://www.gamingonlinux.com/2021/02/classic-open-source-simulation-game-openttd-is-coming-to-steam),
  [OpenRCT2 assets](https://forums.openrct2.org/topic/5646-how-is-openrct2-legal).
- Music: [Jamendo API terms](https://devportal.jamendo.com/api_terms_of_use).
- Licences: [PolyForm](https://polyformproject.org/licenses).
