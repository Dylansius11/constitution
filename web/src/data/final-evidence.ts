// Build-time constants for the final testnet deployment, recorded 2026-11-10. The page renders them
// under the label `Recorded`: they are saved values, never reads. Live reads live in GateLedger.
// Source: the lead's observed-facts record for the final gate; every value is copied, none derived.

export const RECORDED_ON = "2026-11-10";

export const finalGate = {
  account: "constitution-gate.testnet",
  codeHash: "GjPz45TdYvvuVX5k2NtSKDm8gdBt3gvdF7WpUJXZvLGP",
  wasmSha256: "0xe9bbaf8364b7102f474dd9f7b2cebe4bac303aa3f4ee84d28d57905c3bc2c0f0",
  deployTx: "HtRC4WcD9wr9bhsZVBxLd58VnFk5wR8iPEM7sRbEHt1H",
  governanceDelayDays: 3,
} as const;

export const finalEvm = {
  executor: "0xf997a78d8c84dd07637409bfaf46b627d35ab67b",
  proxy: "0x0b54b582dc5486677622c6fde431716767165e74",
  proxyAdmin: "0x7125d3f0f815f646fb3f6a743813a15c6f6b97d0",
  vaultV1: "0xcc6af96372b6cb6c46ff56afef1b87ea4f984c37",
  vaultV2Honest: "0x30f885f098f0cc2980308680795ef4bea0603a42",
  vaultV2Drain: "0x4130c667923dbba6e7c0d92bf356dcbacd39fb15",
  derivedSigner: "0x3b20f46860d27d81917a45221c594fc051af7e94",
} as const;

export const finalVerifier = {
  ruleSetHash: "0x1efe635ac3eb0eb59b0df0d56c7243b2728922898b55487bc5cf8063db94a410",
  publicKeyB64: "8oZcQEC13CJkXPXs+iP1wh4/EgeEtHvUGSXi76tqKIE=",
  releaseHash: "0x0bf3f730e77f94ac0aebb6bf5b02c606b235e87fa8864c6c87b0189966bdc98a",
  composeHash: "0x0fa62de4b208959b1acb02efc04949d6495046d9bd1905280d49cdd244f9d81a",
  platform: "Phala tdx.small",
  appraisal: [
    "Intel DCAP chain",
    "TCB UpToDate",
    "RTMR3",
    "compose hash",
    "report data",
    "release hash",
  ],
} as const;

export const finalRuns = {
  drain: {
    proposalId: 0,
    fixture: "VaultV2Drain",
    ruleId: "R2",
    ruleName: "Solvency",
    reasonCode: 20,
    verifierSeconds: 62.6,
    proposeTx: "23jnKgx7pQK3TvW3EKTCXSR8oRcLYtebs3NMYnNmAZec",
    verdictTx: "7CmP3rRa3WYNuBa7Me3qJ5BrCBpKTX893GTVueCRJzWv",
  },
  governance: {
    changeId: 0,
    kind: "SetParams",
    scheduleTx: "2mn9jrjBzAuBHBgc5ExWJMeYLfTdDpvMEUhVfPC5vA6P",
    applicableFromUtc: "2026-11-13T16:56:01Z",
    refusedCode: "E53_TIMELOCK_NOT_ELAPSED",
    earlyApplyTx: "AvHkJNNzU7zQvQmncDSUhsXTWM8F5bciHVhZSRDBuHP",
  },
  honest: {
    proposalId: 1,
    fixture: "VaultV2Honest",
    initCall: "initializeV2(1000)",
    aiCandidates: 4,
    aiReproduced: 0,
    aiSeconds: 419,
    totalSeconds: 515.8,
    proposeTx: "6XqKZ2ptbcJhBooJMdLfstmKMP1e61DxzU3UKsGRyDwo",
    verdictTx: "cMJQU3T4r2jHmvGc3mkYytT2Lah2hV316CNQEaPd9X1",
    baseTx: "0xe323d9b9c0169174e2b3895b8bdc41dc81e8e83c1e8cb62fcd51e8f708601da2",
    block: 57042237,
    gasUsed: 105099,
    nonce: 0,
  },
  preDeletionAudit: {
    passed: 15,
    expectedFailure: "gate_has_no_access_keys",
  },
  keyDeletion: {
    tx: "FugUyo9Xa8VLTm9qM5vovYUpJU5DAqNjTUriYkmKEhn8",
    block: 277098815,
    timeUtc: "2026-11-10T17:35:22.910Z",
    keysAfter: 0,
    demoGovernorKeyDeletionTx: "45cZx1zvDCdSqrDFBgdnkpa9Wo35SfZrFEGWyqePVSGg",
  },
  keylessAudit: {
    exitCode: 0,
    lines: 16,
    check: "gate_has_no_access_keys",
    result: "PASS",
  },
} as const;

/** Explorer URLs for recorded hashes; the bases come from `site`, never from here. */
export function nearTxUrl(nearExplorer: string, hash: string): string {
  return `${nearExplorer}/txns/${hash}`;
}

export function baseTxUrl(baseExplorer: string, hash: string): string {
  return `${baseExplorer}/tx/${hash}`;
}
