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

- R2 solvency (an account withdraws more than its recorded balance). The committed R2 invariant depth must be at least 4 for R2 to reach this path; the bound and this depth are shown side by side on the page (BUILD-PLAN P3.1).
- R3 does not: the runtime has no `SELFDESTRUCT`, `CALLCODE`, `DELEGATECALL` and no ERC-1967 slot constant (`test_drainRuntimePassesR3OpcodeAndSlotFilter`).

## Local evidence

`forge test` in `contracts/` (Foundry 1.8.6, 2026-11-02 UTC, local only): `test_drainSequenceViolatesR2AtDepthFour`, `test_drainCannotSkipSequenceStages`, `test_honestSameSequencePreservesSolvency` pass. Not testnet evidence.
