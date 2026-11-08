# Drain fixture (`contracts/src/demo/VaultV2Drain.sol`)

Testnet demonstration fixture only. It is the proposal the never-cut demo submits as "the attack". Its contract source stays neutral (it reads like a fee-rebate feature) so the verifier and the private AI reviewer get no hint; this file is the published explanation.

## Hidden path

From a funded vault upgraded to this implementation with `initializeV2(cap)`, one account calls, in order:

1. `deposit(1)` (stage 0 to 1)
2. `withdraw(1)` (stage 1 to 2)
3. `deposit(1)` (stage 2 to 3)
4. `withdraw(token.balanceOf(vault))`: at stage 3 the transfer amount is not checked against the recorded balance, so the call moves every asset while the account's recorded balance is 1.

Minimum depth: **4 successful vault calls by the same account** after setup (initialization, funding, token approval, victim deposits). Failed calls roll back stage changes; no other call advances a stage.

## What should catch it

- R2 checks aggregate solvency (`totalDeposits() <= token.balanceOf(vault)`) and implementation-slot preservation, not per-depositor withdrawability. This fixture empties the assets while leaving victim liabilities recorded, so it violates the aggregate invariant. Its minimum exploit depth is **4**; the current `rules/manifest.json` commits to **invariant depth 4**, loop bound 2, a 10 s assertion-solver timeout, and 2,000 seeded fuzz runs at depth 128. Those are bounded checks, not a proof for arbitrary call sequences or every depositor.
- R3 does not: the runtime has no `SELFDESTRUCT`, `CALLCODE`, `DELEGATECALL` and no ERC-1967 slot constant (`test_drainRuntimePassesR3OpcodeAndSlotFilter`).

## Local evidence

`forge test` in `contracts/` (Foundry 1.8.6, 2026-11-02 UTC, local only): `test_drainSequenceViolatesR2AtDepthFour`, `test_drainCannotSkipSequenceStages`, `test_honestSameSequencePreservesSolvency` pass. Not testnet evidence.
