# R2 depth evidence (P2.5)

Historical depth measurements below (through “Superseded and exploratory runs”) were local measurements on 2026-11-02 and 2026-11-03 (host clock), not testnet evidence. Those runs used fresh `constitution-verifier-spike:local` containers with `--network none --read-only --cap-drop ALL --cpus 1 --memory 1536m`, Halmos 0.3.3, solc 0.8.38, `--solver yices --solver-threads 1 --loop 2 --solver-timeout-assertion 10s --no-status`, and variants of `rules/r2/Solvency.t.sol` (+ `rules/r2/foundry.toml`). Driver: `verifier-image/probe-depth.ps1` (writes `results.tsv` per tag). Classification: `spikes/verifier-min/classify.py` (the invariant JSON has the same shape, test name is `invariant_solvency()`). The security-review measurements below use the production Rust sandbox and its 1 GiB limit instead.

## Pre-hardening aggregate harness, sequential runs (`two-actor-total/`)

Two fixed actors, handlers `deposit(uint96)` and `withdraw(uint96)`, one assert `vault.totalDeposits() <= token.balanceOf(vault)`, `--solver-timeout-branching 1ms --early-exit`.

| candidate | depth | exit | wall s | max RSS KB | cgroup peak bytes | class |
|---|---|---|---|---|---|---|
| VaultV2Honest | 2 | 0 | 4.8 | 96228 | 94457856 | Pass |
| VaultV2Honest | 3 | 0 | 16.6 | 101600 | 93544448 | Pass |
| VaultV2Honest | 4 | 0 | 75.9 | 139936 | 127238144 | Pass |
| VaultV2Drain | 2 | 0 | 8.7 | 96148 | 97234944 | Pass |
| VaultV2Drain | 3 | 0 | 55.5 | 128508 | 115048448 | Pass |
| VaultV2Drain | 4 | 1 | 71.7 | 163944 | 152489984 | Fail |

Repeats at depth 4 (`two-actor-total-repeat/`, 2 runs each): honest 99.4 s and 78.2 s Pass, drain 77.6 s and 72.5 s Fail. Depth 5 honest (`two-actor-total-d5/`): 420.7 s Pass, so depth 5 exceeds the 300 s budget. Drain at depth 4 without `--early-exit` (`two-actor-total-noearly/`): timeout 124 at 335.9 s, Incomplete; `--early-exit` is required.

Counterexample for the drain: `two-actor-total/VaultV2Drain.d4.counterexample.txt` (deposit 1, withdraw 1, deposit 1, withdraw 0x40000000000000000, same actor).

## Superseded and exploratory runs

- `baseline-draft/`: previous worker, `--solver-timeout-branching 0`, 180 s timeout, both Incomplete.
- `two-actor-floor/`: the same two actors with an extra assert (vault balance >= victim deposit); honest d4 133.1 s Pass, drain d4 118.1 s Fail. Slower, not committed.
- `exploration/`: harness variants and the bench scripts used for the shape search (one actor, per-actor token-balance asserts, a minimal token, a Halmos-only generic `svm.createCalldata` handler). The generic handler (sequential): honest d4 Pass 139.5 s, drain d4 Fail 172.0 s (wall printed by `bench.sh`, parallel pairs earlier gave 128.1 s and 192.0 s). It is Halmos-only because Foundry has no `createCalldata` cheatcode.
- `fuzz/`: Foundry invariant runs of the same harness, `forge test` with `FOUNDRY_INVARIANT_RUNS`, `FOUNDRY_INVARIANT_DEPTH`, `FOUNDRY_FUZZ_SEED` (names encode candidate, runs, depth, seed).

Root cause of the earlier Incomplete results: `--solver-timeout-branching 0` (unbounded) lets one z3 branch check dominate (cProfile: 79 of 95 s inside `Z3_solver_check_assumptions` waits for a one-actor depth-3 run; with the default `1ms` the same run takes 11 s). Per-actor token-balance asserts also made depth 4 unfinished (assertion solver timeouts).

## Security-review victim withdrawal measurement (2026-11-03)

[V, local WSL/Docker, not testnet] Both fixtures were run once, sequentially,
through the real `Sandbox::solvency` at the unchanged committed depth **4**,
loop bound 2, assertion timeout 10 s, branch timeout 1 ms, one CPU, 1 GiB,
and the unchanged **150 s inner / 180 s outer** wall budgets. The pre-existing
image was `constitution-verifier-spike:local`,
`sha256:1b2a2c17ba038dfb1047e5089faadeb92a5ea713eddb438e34c9ee7263b7e038`.
No image rebuild, live-network access or testnet operation was used.

The measured variant retained aggregate solvency, chain id 84532, the candidate
runtime-hash check and implementation-slot preservation, then asserted
`token.balanceOf(vault) >= VICTIM_DEPOSIT`, performed a real victim withdrawal,
asserted its token balance increased by the deposit, and redeposited to restore
the victim position for subsequent invariant invocations. Exact source and
machine-readable measurements are in `security-review-victim-withdraw/`.
Its harness-directory digest (including the unchanged `foundry.toml`) was
`0x536d05cdc1868860fd5e685f345c9fbb258d1915da6bd8ac608a0667cef80af3`.

| Fixture | Sandbox::solvency wall time | Observed result |
| --- | --- | --- |
| Honest | 154,714 ms | `infrastructure:infrastructure_timeout` |
| R2 Fail fixture | 154,972 ms | `infrastructure:infrastructure_timeout` |

Command (repository root, WSL; Rust/Foundry on PATH):

```sh
RULES_R2_EVIDENCE_DIR=crates/verifier/evidence/depth/security-review-victim-withdraw \
cargo test -p constitution-verifier --test sandbox \
  t_rule_2_committed_depth_four_harness_passes_honest_and_fails_drain \
  -- --nocapture --test-threads=1
```

The test failed after measuring both fixtures; neither completed within the
inner budget, let alone with the required **20 s margin**. These times include
the initial compilation used to derive the expected candidate runtime hash,
container setup and the timed-out symbolic invocation; Cargo build/lock waiting
is excluded. The victim-floor/withdrawal addition was **reverted**, not made
passing by widening a bound or timeout. The final rule remains **aggregate
solvency, not per-depositor withdrawability**; it also checks bounded
implementation-slot preservation. Depth, loop/solver/fuzz bounds and timeouts
remain unchanged.

## Final security-review harness acceptance (2026-11-03)

[V, local WSL/Docker, not testnet] After removing the additional victim property,
the final source in `rules/r2/Solvency.t.sol` was run once per fixture, sequentially,
through `Sandbox::solvency` with all committed depth/solver/fuzz bounds unchanged.
The candidate code-hash requirement, chain id 84532 and implementation-slot
post-condition remain enabled. Directory commitment:
`0x240f6c97ab34099e65c9c0e400f8f271a934b22ce9eaa5ac4f8f8df8ecfec91b`.

| Fixture | Sandbox::solvency wall time | Observed result |
| --- | --- | --- |
| Honest | 102,856 ms | Pass, reason 0 (Halmos and seeded Foundry fuzzing) |
| R2 Fail fixture | 70,243 ms | Fail, reason 20 (bounded counterexample) |

Exact logs and JSON results: `security-review-final/`. Command: the measurement
command above with `RULES_R2_EVIDENCE_DIR=crates/verifier/evidence/depth/security-review-final`.
Both fixtures completed inside the unchanged 150 s inner budget. These are local
measurements, not a runtime guarantee, testnet evidence or a per-depositor
withdrawability claim.
