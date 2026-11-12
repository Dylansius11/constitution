/**
 * Recorded identity of the live demo gate and its verifier (README "Live demo gate" table,
 * deployments/README.md "Separate live demo deployment", deployments/demo.json). Saved values from
 * 2026-11-11; the page reads the gate live where it can and labels these as recorded.
 */
export const DEMO_RECORDED_ON = "2026-11-11";

export const demoGate = {
  account: "constitution-demo.testnet",
  governanceDelayDays: 1,
} as const;

/**
 * The demo gate's only access key was deleted on 2026-11-12 after the founder's wallet acceptance
 * (deployments/README.md, deployments/demo-receipts/audit-keyless-2026-11-12.txt). The identical
 * audit that failed only gate_has_no_access_keys on 2026-11-11 then exited 0. Recorded values;
 * the live access-key count is read from the chain by the evidence section.
 */
export const demoKeyless = {
  date: "2026-11-12",
  keyDeletionTx: "8xzaoDru5ashVSXMf7aqs1xEPL8MzG2MgBmbCWPiaEjj",
  auditExitCode: 0,
  auditChecks: 15,
} as const;

export const demoVerifier = {
  url: "https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network",
  composeHash: "0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e",
  releaseHash: "0xfe5ca64ccd11b9ecfd29d0a2ba5553bafd6d642d7f7fc8db4528117daaacea5b",
} as const;
