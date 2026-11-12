# Sudan frictionless V1: diagnosis on the B8 pack (2026-10-31)

## Result

The Sudan frictionless failure does not reproduce on the installed B8 pack. Nothing to fix.

Setup: pack data/packs/2000 as installed by B8b (09:19, option A / ADR 0276), copied read-only;
code pre-p3/econ-review fc2a538; release build on x86 Linux; V1 harness copied from p2_gate.rs
(scratch test, not committed). The seed-20 frictionless run reproduces B8b's figures exactly
(price 1.939e-4 at province 2699 good 9, wage 2.027e-4), so the x86 build matches the Mac.

| Run (1,200 ticks, 10% perturbation) | World worst price gap | Sudan worst price gap | Sudan rank of 233 |
|---|---|---|---|
| Frictionless seed 20 | 1.94e-4 (prov 2699) | 2.3e-5 (prov 3287 g9) | 23 |
| Frictionless seed 21 | 1.54e-4 | 2.2e-5 | 19 |
| Frictionless seed 22 | 1.24e-4 | 1.9e-5 | 19 |
| Default seed 20 | 2.43e-4 | 3.9e-5 | 18 |
| Default seed 21 | 7.3e-5 | 2.8e-5 | 16 |
| Default seed 22 | 6.9e-5 | 2.5e-5 | 17 |

All six runs pass V1's 1% on prices, wages and flows, none excluded by the J2 cell rule.
Sudan's gap decays monotonically after tick 360 in every run (0.1 at tick 0, ~1e-2 at 120,
~1e-4 by 480).

Previously (k263 scratch pack, ADR 0267 As built): frictionless seed 20 Sudan 8.37 (local good 9,
province 3474), default seed 20 1.3e-2; block scale 0.341 frictionless / 0.244 default.
On the B8 pack the block scale is 0.957 frictionless (cell limit 0.0957 vs kappa1/m 0.1) and
1 (uncapped) in the default mode.

## Cause of the old failure

Sudan's crude oil (good 1) technology. On the k263 pack, built before ADR 0256, Sudan's good-1
value added came from oil rents (ADR 0249) but its input column came from a donor without crude
(Senegal, VA share 0.072). Gross output was about 14 times value added, so the oil cells had a
very large supply elasticity and J, and the loop ran through Sudan's local goods (good 9 is 13%
of the oil input column) and wages, which is why it showed on local good 9 and why the ADR 0267
block cap could not reach it. Evidence:
- the VA thread measured ADR 0256 alone removing the Sudan drift on main (seeds 21-22, 1.02 to <1e-4);
- on the B8 pack (ADR 0256 + 0270 extraction column) Sudan's good-1 VA share is 0.666 and
  good-1 J is 13.9-20.3, against a J_ref of 20;
- the P3 branch's own notes called it "the pre-0256 drift; recheck at B8".
This is inferred from those records plus the new runs; the old pack was not rerun (it is in the
Mac's Trash, outside the shared folder).

## Remaining work

- B8: record these numbers under ADR 0267 As built and drop "Sudan frictionless" from the open list.
- Optional: commit V1 at seeds 21 and 22 to p2_gate (B8b ran only 20). Not done here.
