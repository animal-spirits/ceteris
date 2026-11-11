# RFC 0343. Licences for the simulator and the game

Status: accepted
Date: 2026-11-11
Design sections: 19.1, 19.3, 22.1, 23
Release: 1.0 (before the repository goes public); 2.0 for the game
Research: reports/licensing-options-2026-11-11.md (not legal advice)

## Summary

The owner's goals are the widest use, protection for himself, and no one else profiting from the
work. MIT OR Apache-2.0 meets only the first. Nothing has been distributed under MIT or Apache
(private repository, no tags, no releases, `publish = false`) and every commit is the owner's or
made for him, so he can relicense without anyone's consent. This RFC moves the simulator to
AGPL-3.0-or-later with commercial licences from the owner, puts the 2.0 game in a separate
source-available non-commercial repository, and requires a CLA for outside contributions. Data
licences are unchanged: users fetch data, NC sources stay out of any paid build, EXIOBASE is
validation-only, Wittgenstein ships with attribution. Trademark registration stays deferred
past 1.0 (#261). A lawyer reviews the licence, CLA and game licence before 1.0.

## Decisions

### D1. Which licence should the simulator use before the repo goes public?

Options:

- A. AGPL-3.0-or-later plus paid commercial licences: OSI open source; distributed or hosted
  forks must publish their source; the owner can sell commercial licences and ship the game.
  Private internal use is not restricted.
- B. PolyForm Noncommercial 1.0.0: all commercial use barred; not OSI open source.
- C. FSL or BUSL converting to Apache-2.0 after 2 to 4 years: bars competing products per
  release for a period; not OSI until conversion.
- D. Keep MIT OR Apache-2.0: maximum adoption; anyone may sell builds or embed the code closed.

Recommendation: A, because it keeps the open source label (JOSS, grants, academic use) while
deterring closed commercial capture.

Decision: A, Antonio, 2026-11-11.
Row: ADR 0346

### D2. How should the 2.0 game be split and licensed?

Options:

- A. Separate private repository, closed game under the Steam EULA.
- B. Separate repository, source-available game under a non-commercial licence.
- C. Same repository, one licence for both.

Recommendation: A, because it protects the paid product most.

Decision: B, Antonio, 2026-11-11. The game code goes in its own repository that depends on the
simulator crates; no simulator crate depends on it. Its licence is PolyForm Noncommercial 1.0.0
unless the lawyer review names a better non-commercial text. Art, music and logos are all rights
reserved and licensed separately from code; commissioned art needs a written assignment. The
owner, as sole copyright holder of the simulator (D3), is not bound by the AGPL in the game.
Row: ADR 0347

### D3. What should outside contributors sign so the owner can relicense?

Options:

- A. A CLA granting the owner a broad licence, including the right to relicense and sell
  commercially; contributors keep their copyright.
- B. No outside code until 1.0.
- C. DCO sign-off only: certifies origin, grants no relicensing right.

Recommendation: A, because D1's commercial licences and D2's game need it.

Decision: A, Antonio, 2026-11-11. The CLA text is `CLA.md`; until a CLA bot is set up (a
`.github/` change the owner makes), a contributor signs by stating agreement in the pull request.
Row: ADR 0348

## Spec changes

- 19.1 licence row and 22.1 licence row: AGPL-3.0-or-later code, commercial licences from the
  owner (ADR 0346); game repository (ADR 0347); CLA (ADR 0348).
- `LICENSE` (AGPL-3.0 text) replaces `LICENSE-MIT` and `LICENSE-APACHE`; `license` fields in
  `Cargo.toml` and `app/Cargo.toml`; `deny.toml` allows AGPL-3.0-or-later; `scripts/package_app.sh`
  ships `LICENSE`; README licence section; `CLA.md` and `CONTRIBUTING.md` added; release plan #178.
- Slint stays under its royalty-free desktop licence or GPL-3.0, both compatible.
