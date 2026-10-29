# 0025. Sequence-space HANK reference for the macro block

Status: proposed
Date: 2026-10-24
Design sections: 3.4, 8.6, 9, 15.3 (V1), 21.1 (P5)
Paths (ADR 0113, 2026-10-29): the reference is built in `ceteris-reference`; `python/` is gone.

## Context

Trade and migration nest into Caliendo-Dvorkin-Parro and Kleinman-Liu-Redding, but household
saving, money and fiscal policy had no reference model, so P5's acceptance rested on plausible
signs and lags. The standard tool for heterogeneous-agent macroeconomics is the sequence-space
Jacobian method (Auclert, Bardoczy, Rognlie and Straub 2021), which solves linearized HANK
models in seconds and has an open-source Python implementation. Auclert, Rognlie and Straub
(2024) show that intertemporal MPCs summarize the household block for aggregate demand.

## Options

(1) No reference; plausibility only. (2) A representative-agent New Keynesian reference. (3) A
small-open-economy HANK per state, solved in sequence space.

## Decision

Option 3. A representative agent cannot match MPCs and would fail the transfer experiments that
matter most for fiscal counterfactuals.

## Migration

- P2: add a harness that computes the intertemporal MPC matrix of the consumption rule (8.6)
  from simulated one-off transfers to each pop group. It runs even with m = 1 and lambda = 0.
- P5: build the reference in `ceteris-reference` (in Rust, ADR 0113) with each state's income process, Taylor
  rule (P35), fiscal rule and trade openness from its initial data, and calibrate its household
  block to the same intertemporal MPC targets as P37.
- V1 macro test: with perfect foresight in place of heuristics (S04) and frictions off, impulse
  responses of output, consumption and inflation to policy-rate and transfer shocks lie within
  tolerance of the reference for small shocks; with heuristics on, signs and peak timing only.

## Tests

The harness reproduces the closed-form case: with m = 1 and lambda = 0 the households'
intertemporal MPC matrix is (1 - s_c - g_c) times the identity (tax and saving shares of ADR
0099), and aggregate spending out of income is 1.

## Consequences

P5 gets a quantitative gate. The reference is linear, so it checks small shocks only; crises
stay judged by the frequency targets of 9.13.
