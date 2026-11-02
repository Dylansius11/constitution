# R2 depth evidence (P2.5)

Local measurements only, 2026-11-02 and 2026-11-03 (host clock), not testnet evidence. Every run used a fresh container from `constitution-verifier-spike:local` with `--network none --read-only --cap-drop ALL --cpus 1 --memory 1536m`, Halmos 0.3.3, solc 0.8.38, `--solver yices --solver-threads 1 --loop 2 --solver-timeout-assertion 10s --no-status`, and the harness `rules/r2/Solvency.t.sol` (+ `rules/r2/foundry.toml`). Driver: `verifier-image/probe-depth.ps1` (writes `results.tsv` per tag). Classification: `spikes/verifier-min/classify.py` unchanged (the invariant JSON has the same shape, test name is `invariant_solvency()`).

## Committed harness, sequential runs (`two-actor-total/`)

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
