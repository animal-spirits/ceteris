# Roadblock recon: options from literature and prior art, 8 November 2026

Scope: four roadblocks, ranked options, no code. Read: `tick-gap-45-goods-2026-11-08.md`, `reports/second-pass-brainstorm-2026-11-08.md`, RFC 0316, spec 19.4, `specs/p3/PLAN.md` Branch E, #200, #351, #354, `crates/ceteris-pack/config/sources.toml`, `clippy.toml`, project memory. Savings are estimates from the measured numbers unless marked measured. Laws: 1 working, 2 right, 3 fast; determinism and f64 are fixed.

## Summary of recommendations

1. **Tick:** take the snapshot encode off the tick (chunk-parallel plus background, class A), then redo the batched-kernel spike on a goods-innermost layout with a fixed canonical summation order (k interleaved accumulators, one golden move). Reject GPU and tiered rates. Budget ADR only on the measured remainder.
2. **Rho:** the trade-war design breaks BLP-N's own identifying assumptions; demeaning by pair and year will not be enough. Research build: product-level LP with importer-product-year and exporter-product-year effects, using clean controls. Paid build: indirect inference against the published BLP-N impulse response, matching its shape, so theta is not needed. Validate out of sample on the 2021 UK Global Tariff, which is open data.
3. **Calibration without NC data:** add a "published estimate" source class (paper, table, value, SE) and fit to moments, not microdata. Re-check the OECD licences (CC BY 4.0 for content published from July 2024), which could replace EXIOBASE.
4. **Determinism:** about 40 std `exp`/`ln`/`powf` calls remain in the sim crates, and Rust documents their precision as platform-dependent. Ban them in `clippy.toml` (libm only) and run the cross-arch hash job (D2) on the fixture now.

---

## 1. 45-goods tick: 230 ms against a 160 ms budget

Measured (8 Nov, 14 threads): dispatch 69, produce 50, deliver 34, end_tick 32, begin 7 ms; yearly snapshot about 31 ms per tick on average (encode serial, 87 ms). Batched shared-kernel dispatch was bit-identical but 1.4 to 2.4x slower on the existing lane structure.

**Why the spike lost, and what the literature says.** Sparse matrix times multiple vectors (SpMM) beats k separate SpMVs because the index and value stream is read once for k right-hand sides (Lee, Vuduc, Demmel, Yelick 2003, [Berkeley TR](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2003/6361.html)). The gain only appears if the k values per row are **contiguous**, so the inner loop is a short dense vector op. The lane structure (ADR 0024, 350 MiB per lane) puts goods far apart, which turns one row read into k strided gathers. NEON has no gather (spec 19.4). The spike therefore tested the wrong layout, and its result does not refute the idea. Second point: since E1a, the lagged crossing kernel is per good ("weight classes are goods in P2", #200), so shared kernels are only exact while rho is off (S12 instant, the paid-build default).

| # | Option | Saving (est.) | Class | Risk | Laws fit |
|---|---|---|---|---|---|
| 1a | **Snapshot encode off the critical path.** RFC 0315 already makes chunks canonical. Compress the chunks in parallel (each chunk is its own zstd frame, so the bytes do not depend on thread count) and run it on a background task over a frozen copy. Do not rely on zstd's built-in multithreading: its maintainers guarantee thread-count-independent output only for the streaming mode ([zstd #2327](https://www.github.com/facebook/zstd/issues/2327)) | 25 to 31 ms/tick on average; 87 ms spike removed | A | Low. Memory for one frozen copy; snapshot must hash identically | Strong: right first, no model change |
| 1b | **Goods-innermost SoA, then re-spike batching.** Spec 19.4 already asks for this ("goods are the innermost dimension ... padded to a multiple of 2 (f64)"). Kernel row: `col u32, k, inv_duty, inv_loss` once; then `[f64; G_block]` per row, G_block 4 to 8 | 30 to 50 ms of dispatch (the earlier 40 to 65 ms estimate, discounted for the measured 69 ms base) | A per good only if each good keeps its own scalar order; see 4c | Medium. Layout touches source, dispatch, deliver | Strong: written design, no RFC |
| 1c | **Canonical k-accumulator order** (see 4c): defines the reference sum as 2 or 4 interleaved partial sums. This enables SIMD and fixes the 44% dependent-add stall measured at 10 goods | 10 to 25% of the compute-bound loops (produce, dispatch) | B (one golden move) | Low; RFC because it changes the reference order | Right, then fast |
| 1d | **RFC 0316 option B (netted journal)**, if not yet counted in the 69 ms | post 20 to 3-5 ms | A for state | Low | Already proposed |
| 1e | **end_tick 32 ms and produce 50 ms superlinearity: profile per good (brainstorm B1)**. Likely the dense 45x45 IO block per province: 45 goods mean 2,025 f64 per province, so 200k provinces are 3.2 GB of streaming if the block is per province rather than per country or class | unknown, could be large | A | None, measuring only | Prerequisite |
| 1f | Tiered or multi-rate goods updates | 30 to 45% of dispatch | C | Changes dynamics, no literature basis for stale routing | Fails "right" |
| 1g | GPU offload | n/a | n/a | **Apple GPUs and Metal have no f64**: Apple said Metal WebGPU implementations "would just not offer" the FP64 extension ([W3C public-gpu](https://lists.w3.org/Archives/Public/public-gpu/2022Apr/0006.html)), and MPS has no Float64 ([Apple forums](https://developer.apple.com/forums/thread/709663)). Double-float emulation ([luma.gl fp64](https://luma.gl/docs/api-guide/shaders/gpu-floating-point-precision)) gives about 48-bit mantissas, not IEEE binary64 results, so it cannot match the CPU path bit for bit | Fails f64 and determinism. Reject for 1.0 |
| 1h | Budget ADR (ADR 0235's last lever) | the rest | none | none | Last |

Projection: 230 - 28 (1a) - 40 (1b) - 10 (1c) gives about 150 to 160 ms before 1e. **Recommendation:** 1a now (small, class A). Then 1e profiling, because it decides whether produce or end_tick hides a layout error worth more than 1b. Then a one-day 1b spike on a real goods-innermost block, with this go rule: 8 goods per row in under 2x one good's time. The 1c RFC goes with 1b. Reject 1f and 1g.

## 2. Rho (short-run trade response speed): identification

State: `calibrate rho` (#351, #354) regresses class-pair ln flow changes on pair tariff changes by year around 2018-19, then grid-searches the half-life. The BACI plus Teti fit failed the pre-shock placebos twice. The planned next step is demeaning by pair and year.

**Diagnosis from the benchmark paper.** Boehm, Levchenko and Pandalai-Nayar (2023, [NBER w27064](https://nber.org/system/files/working_papers/w27064/w27064.pdf)) estimate -0.76 (SE 0.11) at 1 year and -2.06 (0.23) at 7 years, -2.12 (0.32) at 10. Their identification relies on three things the trade war lacks:
- MFN changes, which hit all non-preferential partners alike.
- Preferential partners as controls.
- **Dropping each importer's top-10 partners**, whose tariffs are endogenous.

They also include importer-HS4-year and exporter-HS4-year effects and control for a lag, because tariff changes are negatively autocorrelated. The 2018-19 shock is the opposite case: targeted, discriminatory, between the two largest partners, announced months ahead, and concurrent with China-specific demand shocks. Fajgelbaum et al. (2020, [QJE](https://akhandelwal8.github.io/files/qje_RTP/RTP.pdf)) find clean pre-trends only at HS10 with product-time and country-time effects, and still see front-running around announced dates. Class-pair aggregates with pair-and-year demeaning leave importer-class-year and exporter-class-year shocks in the error term. That is the likely placebo failure.

| # | Estimator / design | What it fixes | Data | Licence | Rating |
|---|---|---|---|---|---|
| 2a | Pair and year demeaning (planned) | pair levels, common years | as now | NC (Teti/TRAINS) | Necessary, not sufficient. Skip straight to 2b |
| 2b | **Product-level (HS6) LP with three-way effects**: Δ_h ln X_ijpt on Δτ_ijpt, with FE importer×product×year and exporter×product×year, pair×product differenced out; leads at h = -3..-1 as placebos; aggregate to class afterwards | multilateral resistance, China/US demand shocks, composition | BACI HS6 + tariffs | research only | **Best research-build fix** |
| 2c | LP-DiD with clean controls (Dube, Girardi, Jordà, Taylor 2023, [NBER w31184](https://www.nber.org/system/files/working_papers/w31184/revisions/w31184.rev4.pdf)), or imputation (Borusyak, Jaravel, Spiess): never-treated products as controls, cohorts by list (1, 2, 3, 4A) | staggered-adoption bias in TWFE event studies | as 2b | research only | Add to 2b; low cost |
| 2d | PPML with HDFE (Correia, Guimarães, Zylkin); bias-correct the three-way FE ([Weidner, Zylkin 2021](https://arxiv.org/pdf/1909.01327)) | zeros, heteroskedasticity in logs | as 2b | research only | Use if zeros are frequent at HS6; `ceteris-reference::ppml` exists |
| 2e | BLP-N MFN design over 1996-2018 | many non-targeted events, matches the falsification band directly | BACI + TRAINS MFN | NC | Cleanest identification, same licence wall |
| 2f | **Indirect inference on the published impulse response**: in the sim, apply MFN-style tariff changes to non-major partners, run the BLP-N LP on simulated flows, pick rho to match the **ratio** IRF(h)/IRF(10) for h = 1..10 (weights from the published SEs) | does not need theta, because levels differ: model long-run theta vs BLP-N about 2; the shape identifies rho | sim only plus 6 published numbers | **licence-clean** (published coefficients, not data; owner to confirm the posture) | **Best paid-build route**; it is E2b's falsification promoted to the fit |
| 2g | **UK Global Tariff (1 Jan 2021) as an out-of-sample test**: the UK replaced the EU CET with simplified, rounded MFN rates, a BLP-N-style MFN change on all non-preferential partners | validation, short horizons (2021-23) only | BACI (etalab) + UKGT (UK Open Government Licence) | open | Check of 2f; horizon too short to fit the long run |
| 2h | Structural dynamic gravity (Anderson, Larch, Yotov; Alessandria, Choi, Ruhl, [NBER w20663](https://www.nber.org/papers/w20663)) | microfounded sunk-cost dynamics | heavy | n/a | Research reference only; over-scoped for 1.0 |

Sanity number: with geometric partial adjustment, a 1-year to 10-year ratio of 0.76/2.12 = 0.36 implies (1-ρ)^12 = 0.64 and a **half-life of about 19 months**. That is inside the 12 to 48 month prior (P53), so the paid-build prior is consistent with the benchmark. The model's own response form (ρθ + 1, ADR 0090) maps differently, so 2f should be run in the model rather than through this formula.

Licence-clean tariff sources found, for 2b without NC data (US-centred):
- **US**: USITC HTS schedules and DataWeb duties (US government works, [DataWeb](https://www.usitc.gov/applications/dataweb/about)); duties collected divided by imports gives implied tariffs.
- **China's retaliation lists**: MOF notices. Official state-organ documents are excluded from Chinese copyright (Art. 5); this needs the owner's legal check.
- **Canada**: Open Government Licence.
- **EU safeguards**: Commission reuse decision 2011/833/EU.

`piie-trade-war` and Teti stay rejected (#354).

**Recommendation:** research build: 2b + 2c on the trade-war data (fixes the placebos, keeps the existing panel). Paid build: 2f against BLP-N, falsified on 2g. Both reuse `ceteris-calibrate::rho`'s grid. Drop 2a as a separate step.

## 3. Calibration without NC data (paid build)

NC or unlicensed now (sources.toml):
- tariffs: TRAINS, WTO TTD, Teti, MAcMap
- input-output and energy: EXIOBASE, IEA
- inequality and politics: WIID, UNU-WIDER GRD, EPR, WID
- other: global-flood-db, macro-nc-datasets, yotov-2016

Open: BACI and CEPII gravity (etalab-2.0), PWT, WDI, UNCTADstat, OECD stats (CC BY).

| # | Approach | Payoff | Risk | Rating |
|---|---|---|---|---|
| 3a | **Published-estimate source class**: `kind = "estimate"` rows citing paper, table, value, SE and the parameter it disciplines. Priors: theta: Armington meta-analysis median 3.8 (2.5 to 5.1) after publication-bias correction (Bajzik et al. 2020, [summary](https://academicnewsletter.sufe.edu.cn/info/297399)), trade elasticity about 4 (Simonovska, Waugh), sectoral (Caliendo, Parro). rho: BLP-N. Import demand 2.53 (Fajgelbaum et al.) as a cross-check | Every P-row gets a licence-clean prior with a stated SE; the ensemble's C1 draws already need this | Priors are not fits; aggregation mismatch between paper and model | **Do first**: small, `rows check` can enforce one source per paid-build parameter |
| 3b | **Simulated method of moments / indirect inference** against published moments (2f generalised): gravity distance coefficient about -1 (Head, Mayer 2014), Brexit ranges, ACR gains | Fits the model's own response, avoids level mismatches | Needs fast calibration (#180/#181) for structural parameters | After C1 fast calibration |
| 3c | **Open substitutes**. MRIO: OECD ICIO, if the CC BY 4.0 default (content published from 1 July 2024, [OECD open-by-default](https://www.oecd-ilibrary.org/en/about/oecd-open-by-default-policy.html)) covers the ICIO release. The source currently says "OECD-Terms", and earlier terms already allowed commercial use "on terms similar to CC BY 4.0". Energy: EIA international (US public domain) for IEA. Inequality: World Bank PIP (CC BY 4.0) for WIID | Could retire most NC rows | Coverage and vintage gaps; third-party material carve-outs | Licence re-check is a quick task; do it now |
| 3d | Two packs, one code path: research pack (fitted) and paid pack (priors), with the profile in the snapshot header and separate goldens | Already implied by the decisions; makes it testable | Golden count doubles | Formalise when 3a lands |
| 3e | Distil NC fits into shipped numbers (fit on NC data, ship the scalar) | Best fit quality | Legal: is a fitted parameter a derivative of the dataset? Owner and legal call, not technical | Ask, do not assume |

**Recommendation:** 3a + 3c (licence re-check) now, 3b once calibration is fast. Put 3e to Antonio as a question card only if 3a/3b priors fail the C7 gates.

## 4. Determinism across threads and platforms (f64)

Built: fixed-slot reductions, rayon `sum/reduce/fold` and `HashMap` banned (`clippy.toml`, ADR 0097), integer ledger, 24-tick hash gate (ADR 0294), libm crate in every sim crate.

| # | Issue / option | Evidence | Action |
|---|---|---|---|
| 4a | **Residual std transcendental calls.** grep of `src/`: trade 21, econ 17, core 1, run 1 calls to `.exp/.ln/.powf/...` outside `libm::` (some may be in tests or non-sim paths). Rust documents `exp`, `ln` and `powf` as "non-deterministic ... varies by platform, Rust version" ([std f64](https://doc.rust-lang.org/std/primitive.f64.html)); they call the system libm (Apple's libsystem_m vs glibc) | e.g. `ceteris-trade/src/source.rs:1769-1805` (sourcing shares) | Add `f64::{exp, ln, powf, powi, exp_m1, ln_1p, log2, log10, sin, cos, tan, atan2, tanh, cbrt, hypot}` to `disallowed-methods`, allow in tests. The pure-Rust `libm` crate (musl port) runs the same code on every target. Class B if any value moves |
| 4b | Correctly rounded maths (CORE-MATH, [core-math crate](https://docs.rs/crate/core-math/1.0.0)) | Results independent of implementation, but a C dependency and partial coverage | Not needed while one `libm` version is pinned; revisit only if a second implementation is needed |
| 4c | **Summation order versus SIMD/ILP.** Auto-vectorisation never reassociates floats without fast-math, so today's loops stay scalar and dependent (the 44% add stall). Hand SIMD changes the order. Define the reference order as k interleaved accumulators (k = 2 for NEON f64, 4 for AVX2), combined in fixed order; scalar code then emulates it exactly | spec 19.4 says hand kernels via `pulp`/`wide` | One RFC, one golden move, unlocks 1b/1c. Keep it a fixed k, never "the machine's width" |
| 4d | Thread-count independence | Fixed partitions are exact and cost nothing; binned reproducible sums (Demmel, Nguyen, [ReproBLAS](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2015/EECS-2015-229.html)) cost about 7 adds per element, 4x a BLAS dot; oneTBB `parallel_deterministic_reduce` is the same idea as ADR 0097 | Keep fixed partitions. Use binned sums only where a partition must be dynamic; none known. Keep ADR 0298 exact accumulation parked |
| 4e | `mul_add` | Specified as IEEE `fusedMultiplyAdd`, "guaranteed not to change"; on x86 without the `fma` target feature it is a slow software call, correct but slow | Fine for determinism; set `target-feature=+fma` on x86-64-v3 builds (with PGO, #236) or accept the cost |
| 4f | NaN payloads, signalling NaN in `powf/powi` (documented as non-deterministic) | std docs | Canonicalise NaN before hashing; assert no NaN in state (probably done; verify) |
| 4g | **Cross-arch proof** | Branch H not started (G8) | D2: Linux x86-64 vs macOS arm64 hash on fixture and toy worlds in cloud CI (cheap); full pack nightly on the Mac against one stored x86 hash |

**Recommendation:** 4a and 4g now (small, catches the realistic failure). Do 4c with 1b. 4b, 4d and binned sums are not needed.

## Open questions for Antonio (to go on cards, not answered here)

1. May a scalar fitted to published coefficients (2f, 3a) ship in the paid build? I assume yes, because the numbers are facts from papers, but this needs your legal posture.
2. Are Chinese MOF and US USITC tariff lists acceptable as open sources for the research and paid builds?
3. Should a canonical k-accumulator summation order (4c, one golden move) be adopted?
