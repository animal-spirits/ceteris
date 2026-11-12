# RFC 0319 S0, ADR 0294 check and S4 on the Mac

Prereqs: pack built (`CETERIS_PACK` set), Mac idle (J2 finished), repo on main containing #403 to #405.
Long runs: detached `caffeinate -imsu` (never -d), logs in `~/ceteris-logs/`.

## 1. ADR 0294 identity gate (merged tree against the pre-S1 tree)

    git worktree add ../ceteris-base c7416de   # RFC 0319 merge, before S1
    for t in 1 0; do   # 0 = all threads
      (cd ../ceteris-base && cargo run --release -p ceteris-econ --example identity -- --ticks 24 --threads $t --goods 45) > ~/ceteris-logs/id-base-$t.txt
      cargo run --release -p ceteris-econ --example identity -- --ticks 24 --threads $t --goods 45 > ~/ceteris-logs/id-new-$t.txt
    done
    for t in 1 0; do diff ~/ceteris-logs/id-base-$t.txt ~/ceteris-logs/id-new-$t.txt && echo same-$t; done

Expect no diff (state and journal hashes per tick). Any diff: stop, S1 to S3 are not class A.
Optionally also compare id-new-1 with id-new-0.

## 2. S0 baseline (before) and S4 re-measure (after), quiet host

    scripts/bench_quiet.sh ceteris-econ goods            # on the merged tree (S4)
    # S0: same command in ../ceteris-base (c7416de), appends to its own tests/perf/history.csv
    cargo run --release -p ceteris-econ --example identity -- --ticks 24 --goods 45 --timing

Record: tick, deliver, t2b.apply, t2b.transit, in_flight_all, begin.totals, end.ledger.totals,
end.leontief, end.ledger.sign (ms), load, threads. S0 also records the buyers count
(n_pops + n_enterprises of the pack) and slot bytes = 4 x buyers x 45 x 8.
Single-thread CPU per year for G1: `scripts/bench_quiet.sh ceteris-econ flatness`.

## 3. S4
Restate the 19.2 45-goods table (ADR 0235) from the S4 numbers; if the tick is still above
160 ms, the remainder goes to the masked block-dispatch variant (#373) or ADR 0235 levers.
The 19.2 edit is a spec PR: Antonio merges.
