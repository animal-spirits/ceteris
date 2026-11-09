# Linux x86_64 golden check (P3 Branch H, #203)

Date: 9 November 2026. Host: quant-dev, Linux x86_64, 4 cores, rustc 1.99.1 (pinned). Commit: main at 7b307e6.

Command: `RAYON_NUM_THREADS=<n> scripts/check_golden.sh` for n = 1 and 14.

Result: `ceteris-core` golden and p0_acceptance, `ceteris-econ` golden and p2_acceptance all pass unchanged at both thread counts. No golden moved. This is a correctness result only; timings from this host are not comparable to the Mac and are not budget measurements.

Not covered: Windows, arm64 on this host, and the CI job itself (needs `.github/`, an owner PR from a non-`claude/*` branch).

Environment note: on quant-dev `/usr/bin/cargo` (1.92) shadows rustup's. Put `~/.rustup/toolchains/1.99.1-*/bin` first on `PATH`, or the build fails with "rustc 1.92.0 is not supported".
