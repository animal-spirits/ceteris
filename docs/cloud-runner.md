# Cloud calibration runner

Unattended 2 hour fits (`calibrate --pack P`, `--fit-epsilon`, nu fit, golden re-bless, the
`docs/free-moment-rerun.md` harness) on a rented Linux box, so no run needs the Mac or a per-run
approval.

## Machine

Debian or Ubuntu 22.04+, x86_64 or arm64. Fits are CPU-bound (rayon) and fit in memory: 16 or more
dedicated vCPU, 32 GB RAM, 150 GB SSD (pack, `data/cache`, `target/`). A dedicated-core instance,
not a burstable one, so timings are comparable. Goldens are portable across architectures
(`scripts/check_golden.sh`), so x86_64 is fine.

## One-time setup

```
git clone git@github.com:animal-spirits/ceteris.git && cd ceteris
scripts/cloud/setup.sh
```

Installs the toolchain pinned by `rust-toolchain.toml`, builds `ceteris`, creates
`~/ceteris-logs/` and `~/ceteris-fit-cache/`, then runs `ceteris pack fetch`.

## Inputs

`ceteris pack fetch` downloads every non-manual file in `crates/ceteris-pack/config/fetch.toml`
(BACI HS92, the OECD end-use key, CEPII gravity with distw, Natural Earth, ...) and exits 1 unless
each file matches its pinned sha256 and size. The OECD ICIO zips are `manual = true` (the host
rejects scripts): copy `data/cache/` from the Mac once,

```
rsync -a mac:path/to/ceteris/data/cache/ data/cache/
```

and `fetch` verifies them. `setup.sh` and `fit.sh` refuse symlinks in `data/cache`, so cached
files cannot point into an old checkout.

The built pack is made owner-side (GEOS 3.15, tippecanoe), so copy it too:
`rsync -a mac:path/to/ceteris/data/packs/2000/ data/packs/2000/`. `fit.sh` logs a combined sha256
of the pack files; compare it with `find data/packs/2000 -type f -print0 | sort -z | xargs -0
sha256sum | sha256sum` on the Mac.

## Running

```
scripts/cloud/fit.sh eps-fit data/packs/2000 --fit-epsilon --out ~/runs/eps
scripts/cloud/fit.sh calib  data/packs/2000 --out ~/runs/calib
scripts/cloud/fit.sh status eps-fit
```

`fit.sh` refuses a dirty tree, rebuilds, verifies inputs, logs commit, tree hash and pack hash,
then starts the run detached (`setsid nohup`) with the log in `~/ceteris-logs/LABEL-STAMP.log` and
the exit code in `LABEL-STAMP.exit`. Extra arguments go to `ceteris calibrate`. Other harness
commands (`cargo test --release -p ceteris-calibrate --test benchmark ...`) run the same way under
`setsid nohup` with `CETERIS_PACK` set; they need no fit cache.

## Fit cache

`--fit-cache DIR` stores each epsilon trial (one nu fit plus the gravity regression) under a key
hashing the exact inputs, targets, limits and epsilon. A rerun of the same inputs replays every
trial, and a run that died resumes from the last stored one. Output is bit-identical to an uncached
run (test `cached_fit_is_bit_identical`). `fit.sh` keeps one directory per git tree hash, so any
code change starts a fresh cache.

A cache keyed per theta class, so that only changed classes refit, is not possible without
changing results: the classes share every nu evaluation (`border::fit_border_nu`), so one class's
change moves all classes' trials. A reduced-pack quick-fit is likewise not offered: it needs a
second pack build and would change numbers. Both would need an RFC.
