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

export const demoVerifier = {
  url: "https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network",
  composeHash: "0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e",
  releaseHash: "0xfe5ca64ccd11b9ecfd29d0a2ba5553bafd6d642d7f7fc8db4528117daaacea5b",
} as const;
