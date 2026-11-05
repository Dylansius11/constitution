# CONSTITUTION: integrations and setup

## TL;DR

- [V] Exercised on 2026-10-26, read-only only: MPC `public_key` and `config` views on `v1.signer-prod.testnet`; Base Sepolia `eth_chainId` (`0x14a34`) and `eth_getCode` of the canonical CREATE2 deployer; OutLayer `get_max_limits` and `get_pricing_full` views; npm registry, crates.io sparse index, PyPI and GitHub release metadata. Nothing else has been exercised: no account, key, deployment, transaction, inference or paid call exists.
- Every integration below is **documented, not working** until its BUILD-PLAN task records observed evidence.
- Founder manual actions (accounts, faucets, cards, credits, publishing) are owner-tagged BLOCKERS B1 to B8 in `docs/OPEN-DECISIONS.md`. None may start before 2026-11-01T00:00:00Z.
- Secrets live only in the founder's local `.env` (gitignored), the NEAR CLI keychain, the Foundry keystore, or as encrypted CVM secrets. Never in docs, chat, logs or commits.

## Integration inventory

| Integration | Purpose | Owner | Manual / automatic | Access or cost gate | Status and source |
| --- | --- | --- | --- | --- | --- |
| NEAR testnet accounts | Gate, guardian, demo governor, verifier relay, deployer | founder creates; agent scripts | manual creation, automatic use | faucet | documented; docs.near.org/tools/cli |
| Chain Signatures MPC | Signs for the gate-derived EOA | external | automatic | 1 yoctoNEAR + gas per sign | live views exercised 2026-10-26 |
| NEAR RPC (FastNEAR testnet) | Reads and tx submission | external | automatic | free public, quota unpublished | documented; docs.near.org/api/rpc/providers |
| Base Sepolia RPC | Reads (`sepolia.base.org`), send (`sepolia-sequencer.base.org`) | external | automatic | free public, quota unpublished | `eth_chainId` exercised |
| Base Sepolia faucet | ETH for derived EOA and deployer | founder | manual | faucet limits | documented; docs.base.org/get-started/get-funds |
| Validation Cloud | Archive + `debug_traceCall` prestate + `eth_getProof` | founder account; agent use | manual setup, automatic use | free 50M CU/month | documented (D10) |
| Phala Cloud dstack CVM | Attested verifier host | founder account; agent deploys with approval | manual setup, approved deploys | USD 20 credit after card check (D2) | documented |
| NEAR AI Cloud | Private red-team model | founder key | manual setup, automatic use | hackathon credits requested (D11) | documented |
| Rust + cargo-near + near-sandbox | Gate build and tests | agent | automatic after opening | none | documented; Linux/WSL2 |
| Foundry + solc + OpenZeppelin | EVM contracts and tests | agent | automatic after opening | none | documented |
| Halmos + solvers | R2 bounded checks | agent (in verifier image) | automatic | none | documented |
| Node + pnpm | AI sidecar, upgrades-core, page, viem oracle | agent | automatic | none | documented |
| Blockscout (per-instance) | Source verification without a key | agent | automatic | none | documented; optional |
| Etherscan V2 | Optional verification | founder key | manual | Base Sepolia marked "Paid Plan required" in chainlist | ambiguous, avoid |
| Page hosting | Public page | founder | manual | D8 open | not chosen |
| GitHub repo `Dylansius11/constitution` | Public code | founder | manual | after opening | D9 |
| X post and submission | Required demo and entry | founder only | manual | human-only per rules | D9, EVENT.md |

## Environment variables (names only)

One convention for the whole repo. `.env.example` will list these names with empty values when the build opens.

| Name | Consumer | Purpose | Class |
| --- | --- | --- | --- |
| `NEAR_NETWORK_ID` | cli, verifier | `testnet` | public |
| `NEAR_RPC_URL` | cli, verifier | `https://test.rpc.fastnear.com` | public |
| `GATE_ACCOUNT_ID` | cli, verifier, page | gate account; required at verifier startup in chain-connected mode; other request gates return `400 wrong_gate` before RPC | public |
| `GUARDIAN_ACCOUNT_ID` | cli | guardian account (keys in NEAR keychain) | public id |
| `DEMO_GOVERNOR_ACCOUNT_ID` | page, cli | public demo account (D17) | public |
| `PUBLIC_DEMO_GOVERNOR_KEY` | page | function-call key shown as QR; public by design, testnet only | public by design |
| `VERIFIER_RELAY_ACCOUNT_ID` | verifier | submits verdicts | public id |
| `VERIFIER_RELAY_KEY_FILE` | verifier (key file mounted as a CVM encrypted secret) | path to the near-cli key file of a function-call key for `submit_verdict` only | secret file |
| `BASE_SEPOLIA_RPC_URL` | cli, verifier | `https://sepolia.base.org` | public |
| `BASE_SEPOLIA_SEND_URL` | cli, verifier relay loop | `https://sepolia-sequencer.base.org` | public |
| `BASE_SEPOLIA_ARCHIVE_RPC_URL` | verifier (CVM secret), cli dev | Validation Cloud endpoint, contains key | secret |
| `NEARAI_API_KEY` | AI sidecar (CVM secret) | NEAR AI Cloud bearer key (SDK doc name; `NEAR_AI_CLOUD_API_KEY` from the manual E2EE doc is not used) | secret |
| `NEARAI_BASE_URL` | AI sidecar | `https://cloud-api.near.ai/v1` | public |
| `NEARAI_MODEL` | AI sidecar | `z-ai/glm-5.3-flash` | public |
| `VERIFIER_URL` | cli, page (`PUBLIC_VERIFIER_URL`) | verifier HTTPS base | public |
| `EXECUTOR_ADDRESS` | cli, page (`PUBLIC_EXECUTOR_ADDRESS`) | executor on Base Sepolia | public |
| `PUBLIC_NEAR_RPC_URL`, `PUBLIC_BASE_RPC_URL`, `PUBLIC_GATE_ACCOUNT_ID` | page | read endpoints | public |
| `ETHERSCAN_API_KEY` | forge verify (optional) | only if Etherscan is chosen | secret |
| `PAGE_ORIGIN` | verifier | CORS allowed origin of the hosted page (D8) | public |
| `PUBLIC_EXPLORER_BASE_URL`, `PUBLIC_EXPLORER_FALLBACK_URL` | page | `https://sepolia.basescan.org`, `https://base-sepolia.blockscout.com` | public |
| `PUBLIC_X_POST_URL` | page | founder's X demo post; link hidden while empty | public |

EVM deployer key: Foundry encrypted keystore (`cast wallet import deployer --interactive`), referenced with `--account deployer`; never a private key in `.env`.

## Step-by-step recipes

Every command below is **planned, not run**, unless marked "exercised". Commands with writes need the authorization noted.

### 1. NEAR testnet accounts (B1)

Docs: https://docs.near.org/tools/cli (checked 2026-10-26); near-cli-rs v0.30.1.

Human steps (founder, after 2026-11-01):
1. Choose ids: `<gate>.testnet`, `<guardian>.testnet`, `<demo-governor>.testnet`, `<verifier-relay>.testnet`, `<deployer>.testnet` (top-level testnet names; sub-accounts of the deployer are also fine).
2. For each, run the faucet-sponsored creation and keep keys in the OS keychain:
   `near account create-account sponsor-by-faucet-service <id> autogenerate-new-keypair save-to-keychain network-config testnet create` [V docs syntax].
3. Top up from https://near-faucet.io/ if needed (resource hub advertises 2 NEAR, rate limited).
4. Write the account ids (not keys) into `.env` and SMART-CONTRACT "Deployment".

Agent steps: confirm exact subcommands with `near account --help` on 0.30.1 before use; read balances and keys with the `view-account-summary` / `list-keys` subcommands (exact syntax confirmed at that time). Creating the demo governor function-call key (receiver = gate, methods `propose`, `schedule_change`) happens in P3.3 with founder approval.

Verification: account summary shows a non-zero balance. Expected error: faucet exhausted; stop and tell the founder.

### 2. Chain Signatures and the derived EOA (P1.2)

Docs: https://docs.near.org/chain-abstraction/chain-signatures/implementation ; near/mpc `crates/contract` (checked 2026-10-26).

Exercised read-only probe (2026-10-26):

```text
curl -sS https://rpc.testnet.near.org -H "Content-Type: application/json" --data-raw '{"jsonrpc":"2.0","id":"x","method":"query","params":{"request_type":"call_function","finality":"final","account_id":"v1.signer-prod.testnet","method_name":"public_key","args_base64":"e30="}}'
```

Result decoded to `secp256k1:4NfTiv3UsGahebgTaHyD9vF8KYKMBnfd6kh94mK6xv8fGBiJB8TBtFMP5WWXz6B89Ac1fbpzPwAvoyQebemHFwx3` at block 274744526. `config` view showed `sign_call_gas_attachment_requirement_tera_gas = 15`. Use `NEAR_RPC_URL` (FastNEAR) for build work; near.org RPC is deprecated with reduced limits.

Agent steps (P1.2): call view `derived_public_key({"path":"constitution/base-sepolia/v1","predecessor":"<gate>.testnet","domain_id":0})`; compute the EOA offline in `constitution-types` and compare; record both. Expected: identical address.

### 3. Base Sepolia funding (B2)

Docs: https://docs.base.org/get-started/connect-to-base and https://docs.base.org/get-started/get-funds (checked 2026-10-26). Chain id 84532; explorer https://sepolia.basescan.org ; alternative explorer https://base-sepolia.blockscout.com/.

Exercised: `eth_chainId` on `https://sepolia.base.org` returned `0x14a34`.

Human steps: claim from a listed faucet (CDP advertises up to 0.1 ETH per 24 h) to (a) the derived EOA from step 2 and (b) the deployer EOA from the Foundry keystore. Amounts per D14.

Verification (planned): `cast balance <address> --rpc-url $BASE_SEPOLIA_RPC_URL` non-zero.

### 4. Base Sepolia archive / debug and proof RPC (B5, D10)

Docs: https://docs.validationcloud.io/nodeapi/base/overview , `debug_tracecall.md`, `eth_getproof.md`, `about/billing` (checked 2026-10-26).
Proof endpoint and retention context: https://docs.base.org/base-chain/node-operators/run-a-base-node (including historical proofs and public endpoint limits; checked 2026-11-05).

Human steps: create a free account, create a Base Sepolia endpoint, paste the URL into `.env` as `BASE_SEPOLIA_ARCHIVE_RPC_URL`. Set `BASE_SEPOLIA_PROOF_RPC_URL=https://sepolia.base.org` for the founder-approved hackathon split. Chain mode requires the proof variable explicitly; for local Anvil, set it to the same Anvil URL as the archive endpoint. No QuickNode subscription is required.

Agent verification (P1.7): both endpoints return chain id `0x14a34` and the same hash-verified header for one fixed block N; the archive endpoint provides historical code/storage and `debug_traceCall` with `prestateTracer`, while the proof endpoint provides `eth_getProof` for every discovered account/slot at N. Verify all proofs against N's `stateRoot` and recheck N's hash on both endpoints at completion. Any unavailable, mismatched or invalid response withholds certification; never silently reduce R5 scope or fall back to `latest`.

Initial observation (2026-11-05, exercised on Base Sepolia): the supplied Validation Cloud endpoint passes chain id, historical code/storage and `prestateTracer` with verified override/diff behavior, but fixed/recent numeric-block proofs fail with `-32602` maximum proof window; `latest` only returns a proof response. Resolution under refined D10: the production split-provider witness fetch passed at fixed block 54,676,922 with 8 accounts and 7 slots fully proof-authenticated (BUILD-PLAN P1.7). The public endpoint is rate-limited and proof-history availability is bounded; this is a hackathon liveness tradeoff, not a production uptime guarantee. Revalidate the deployed CVM configuration and final corpus during activation. No credential-bearing URL is recorded here.

### 5. Phala Cloud dstack CVM (B3, D2, D18)

Docs: https://docs.phala.com/phala-cloud/getting-started/sign-up-for-cloud-account ; https://docs.phala.com/dstack/getting-started.md ; https://phala.com/pricing ; https://cloud.phala.com/about/pricing ; https://docs.phala.com/phala-cloud/key-management/get-a-key.md (checked 2026-10-26).

Human steps (founder):
1. Register at https://cloud.phala.com/register with GitHub, Google or email.
2. Create a workspace; add a card (USD 1 check, refunded) to receive the verified-account credit; confirm USD 20 CVM credit is visible.
3. Do not deploy anything yourself; approve each deploy the agent proposes, with instance size and expected hourly cost.

Agent steps (P1.6, with approval): build `verifier-image/` with digest-pinned base image and tool binaries; deploy the compose; read `GET /v1/identity`; run `constitution verifier appraise <url>`; record `release_hash`, compose hash, image digest, instance size, observed hourly cost. The certificate key is ephemeral (D18); do not use `GetKey` for it.

Verification: appraisal passes DCAP chain, TCB `UpToDate`, RTMR3 replay equals compose hash, `report_data` matches published keys. Errors: credit missing (escalate D2 fallback decision), tier too small (escalate D18).

Fallback (only after a new founder decision): OutLayer, docs https://app.outlayer.ai/docs/getting-started ; live `outlayer.testnet.get_max_limits` returned `[500000000000,180,300]` on 2026-10-26.

### 6. NEAR AI Cloud (B4, D11)

Docs: https://docs.near.ai/cloud/quickstart.md ; https://docs.near.ai/cloud/models.md ; https://docs.near.ai/cloud/inference-sdk/javascript.md ; https://docs.near.ai/cloud/verification/cloud-api/response-signatures.md ; hackathon credits via https://docs.near.org/getting-started/hackathons (checked 2026-10-26).

Human steps: credits already requested (2026-10-26). When granted, create an API key in the console and store it as `NEARAI_API_KEY` in local `.env` and later as an encrypted CVM secret. Never paste it into chat.

Agent steps (P2.6): sidecar with `@nearai/inference-sdk` 0.1.0 on Node 24.22.1 (`import { InferenceClient } from '@nearai/inference-sdk/node'`), `e2ee: true`, strict TCB policy, `x-no-aliasing: true`, model `z-ai/glm-5.3-flash`; after each completion `verifyResponse(completion.id)` and require `signatureKind === 'provider_tee'`.

Verification: authenticated `GET /v1/attestation/report?model=z-ai/glm-5.3-flash&provider=near&nonce=<fresh>` succeeds (documented non-billable); one test completion verifies as `provider_tee`. Cost per review measured and recorded. Errors: gateway-only signature means review incomplete, never pass.

### 7. Rust, cargo-near and near-sandbox (P0, P1.1)

Docs: https://docs.near.org/smart-contracts/quickstart ; https://raw.githubusercontent.com/near/cargo-near/main/README.md ; https://github.com/near/near-sandbox-rs (checked 2026-10-26).

Agent steps (Linux or WSL2; native Windows sandbox not supported by upstream docs): install rustup toolchain 1.99.1 with target `wasm32-unknown-unknown` (pinned by `rust-toolchain.toml`); install cargo-near 0.22.0 and near-cli-rs 0.30.1 from their releases; `cargo near build non-reproducible-wasm` for iteration and `cargo near build reproducible-wasm` (Docker) for deployable code. near-workspaces 0.23.0 downloads a sandbox binary at build time unless `NEAR_SANDBOX_BIN_PATH` is set.

Verification: reproducible build prints the wasm sha256; two builds give the same hash.

### 8. Foundry, solc and OpenZeppelin (P1.3)

Sources: Foundry v1.8.6 and solc v0.8.38 GitHub releases; `@openzeppelin/contracts` and `-upgradeable` 5.6.1, `@openzeppelin/upgrades-core` 1.47.0 via `npm view` (2026-10-26).

Agent steps: install Foundry v1.8.6 with `foundryup` pinned to that release (confirm the version flag with `foundryup --help` first; syntax not verified in planning); `foundry.toml` pins `solc = "0.8.38"`, explicit `evm_version`, `build_info = true`, `extra_output = ["storageLayout"]`, `ffi = false`; OZ contracts installed at tag v5.6.1 as git submodules or npm packages (choose one at P1.3 and record).

Verification: `forge test` green on executor tests; `npx @openzeppelin/upgrades-core@1.47.0 validate out/build-info --contract VaultV2Honest --reference VaultV1 --requireReference` passes, and the same command fails on a deliberately broken-layout fixture.

### 9. Halmos (P1.6, inside verifier image)

Sources: https://pypi.org/pypi/halmos/json (0.3.3, Python >= 3.11), tagged README and config (checked 2026-10-26). Install in the image with pinned versions: `halmos==0.3.3`, `z3-solver==4.12.6.0`, yices 2.6.4.x resolved to an exact version and recorded. Always pass explicit bounds from the manifest and `--solver-timeout-branching 0`.

Verification: R2 harness finds the planted bounded counterexample fixture and passes on VaultV1 and VaultV2Honest within the committed bounds.

### 10. Node, pnpm, Astro and viem (P2.7, P3)

Sources: Node 24.22.1 LTS (nodejs.org/dist/index.json); `npm view` for pnpm 12.15.0, astro 7.4.2, typescript 7.0.2, viem 2.57.11, near-api-js 7.4.0, @biomejs/biome 2.6.3, vitest 5.0.5, @playwright/test 1.64.1 (2026-10-26). Lockfile committed; `packageManager` field pins pnpm.

Verification: `pnpm --dir tools/vectors-viem vectors` reproduces every T-ENC-1 vector byte for byte.

### 11. Explorer source verification (optional, P4)

Prefer Blockscout per-instance (no key): `forge verify-contract --verifier blockscout --verifier-url https://base-sepolia.blockscout.com/api/ <address> <contract>` [V docs.blockscout.com, checked 2026-10-26]. Etherscan V2 chainlist marks Base Sepolia "Paid Plan required"; use only if the founder provides a key and accepts the ambiguity. Explorer verification is publication support, never part of the gate's decision.

### 12. Hosting (B7, D8)

Not chosen. All three candidates serve a static Astro build with `PUBLIC_*` variables only. Founder creates the account; agent prepares the deploy config after the choice.

### 13. Repo, X and submission (B6, D9)

Founder creates `github.com/Dylansius11/constitution` after 2026-11-01 and makes it public before the cutoff. Founder publishes the X demo mentioning NEAR HACKATHON with media. Founder submits once, after the repo is public and the post is live, approving the exact payload; the `website` field stays empty (spam trap per https://nearhackathon.com/HACKATHON.md, re-read 2026-10-26). Cutoff 2026-11-14T23:59:00Z.

## Skills: provenance, installation and restore

[V] 25 project skills are installed in `.claude/skills/` (gitignored); `skills-lock.json` holds a `computedHash` per skill. Lead check on 2026-10-26: the installed directory set equals the lock key set (25 = 25, no missing, no extra). Installed earlier with `npx --yes skills@1.7.4 add <source> -a claude-code --copy -y -s <names>`. Upstream revisions are `git ls-remote` HEAD observations on 2026-10-26, not pins.

| Skill | Upstream owner / repository | Revision observed | License | Why needed | Installed path |
| --- | --- | --- | --- | --- | --- |
| near-smart-contracts | near/agent-skills (official) | cb32b6af195e636c1e9c2efe2ea75a6d85b6b67b | MIT stated in SKILL.md | Gate contract patterns (verify versions live; its Chain Signatures response shape is outdated) | `.claude/skills/near-smart-contracts/SKILL.md` |
| near-api-js | near/agent-skills | same | not established; local instruction use only | Page signing with the demo key | `.claude/skills/near-api-js/SKILL.md` |
| near-ai-cloud | near/agent-skills | same | not established; local instruction use only | AI sidecar (its "all inference" claim is overridden by tier docs) | `.claude/skills/near-ai-cloud/SKILL.md` |
| secure-workflow-guide | trailofbits/skills | 196a9073ab10d35bacf111f945e02c00b2c6883a | CC-BY-SA-4.0 | Security workflow for gate and executor | `.claude/skills/secure-workflow-guide/SKILL.md` |
| property-based-testing | trailofbits/skills | same | CC-BY-SA-4.0 | Fuzz and invariant tests | `.claude/skills/property-based-testing/SKILL.md` |
| upgrade-solidity-contracts | OpenZeppelin/openzeppelin-skills | 6f215af60eb60017ab1a933ce9d22a479cd42b26 | AGPL-3.0-only (skill text only) | Proxy and layout practice | `.claude/skills/upgrade-solidity-contracts/SKILL.md` |
| gsap-core | greensock/gsap-skills | aed9cfd3277740755f6bfc1155c7aa645403b760 | MIT | Optional page motion (not a selected dependency) | `.claude/skills/gsap-core/SKILL.md` |
| brainstorming, writing-plans, executing-plans, subagent-driven-development, test-driven-development, systematic-debugging, verification-before-completion, requesting-code-review | obra/superpowers | 2ce8aa15d05bd57213e6f746f953aafb7ded7a8b | MIT | Process discipline | `.claude/skills/<name>/SKILL.md` |
| codebase-design, improve-codebase-architecture, code-review | mattpocock/skills | 5beb35b040e3de97e54fa9e386d7e42d15c295c2 | MIT | Module boundaries and review | `.claude/skills/<name>/SKILL.md` |
| impeccable | founder local tree, upstream pbakaus/impeccable | local snapshot 4.3.1 | Apache-2.0 upstream | UI quality (planning guidance only; launcher not executed) | `.claude/skills/impeccable/SKILL.md` |
| emil-design-eng | founder local, upstream emilkowalski/skills | local snapshot | MIT upstream | UI polish | `.claude/skills/emil-design-eng/SKILL.md` |
| design-taste-frontend | founder local, upstream Leonxlnx/taste-skill | local snapshot | MIT upstream | Visual direction | `.claude/skills/design-taste-frontend/SKILL.md` |
| motion | founder local, upstream motiondivision/ai-kit (official) | local snapshot | stated MIT/open source by the server; no license file seen at repo root | Page motion guidance; no Motion+ implied | `.claude/skills/motion/SKILL.md` |
| ponytail | founder local, upstream DietrichGebert/ponytail | local snapshot | MIT | Scope control | `.claude/skills/ponytail/SKILL.md` |
| find-docs | founder local, upstream upstash/context7 | local snapshot | MIT upstream | Live docs lookup (no Context7 login or global install) | `.claude/skills/find-docs/SKILL.md` |
| orca-cli | founder local stub, upstream stablyai/orca | local snapshot | not established from stub | Terminal orchestration only when needed | `.claude/skills/orca-cli/SKILL.md` |

Also used, not project-installed: `hackathon-winning` (global junction to the founder's ideation workspace `.agents/skills/hackathon-winning/`).

License notes: near/agent-skills shows MIT only in `near-smart-contracts`; do not redistribute the other two or claim MIT for all. Trail of Bits CC-BY-SA-4.0 requires attribution and share-alike if redistributed. The OpenZeppelin skill's AGPL applies to the skill text, not to the OpenZeppelin Contracts library.

Restore (supported by inspected CLI help; not run as a restore test): from the project root `npx --yes skills@1.7.4 experimental_install`. The seven local rows (`sourceType: local`, source `../../../../../.agents/skills`) resolve only on this machine layout. Elsewhere: `npx --yes skills@1.7.4 add "<trusted-founder-skill-tree>" -a claude-code --copy -y -s impeccable emil-design-eng design-taste-frontend motion ponytail find-docs orca-cli`, then compare hashes with `skills-lock.json` before accepting. Lock hashes detect drift; they are not commit pins. For exact historical restore keep the trusted snapshot or install a reviewed commit and review the lock diff.

| Skipped candidate | Reason / evidence |
| --- | --- |
| near-intents | Out of scope; mainnet-only per brief |
| near-dapp, near-kit | Redundant with near-api-js and near-smart-contracts for Stack C |
| Foundry / Halmos skill | No verified official SKILL.md found; use official docs |
| trailofbits rust-review | Its SKILL.md excludes NEAR contracts and depends on plugin scripts not present in a standalone copy |
| Extra GSAP plugins, extra UI packs | No demonstrated need for one page |

## omp model routing

- [V] omp 18.12.1. Project `.omp/config.yml` sets `modelRoles` default and plan `anthropic/claude-opus-5-5:high`, task `openai-codex/gpt-6.1-sol:high`, secondary `anthropic/claude-sonnet-5-5:high`; `task.maxConcurrency: 3`; a 26-name `skills.includeSkills` allowlist; `skills.customDirectories` pointing at this project's `.claude/skills`. Lead verified on 2026-10-26 with `omp config get modelRoles --json` and `omp config get task.maxConcurrency --json` from the project root.
- [V] `C:/Users/HP Victus/.omp/agent/agents/executor.md` declares `model: '@task'`; `secondary-executor.md` declares `@secondary`.
- [V] Observed in this planning session's worker transcripts: executor workers resolved to `openai-codex/gpt-6.1-sol` with `resolvedModelIsFallback: false`; the design worker resolved to `anthropic/claude-sonnet-5-5` with `resolvedModelIsFallback: false`.
- Rule: at most 3 workers at once, at most 1 Sonnet worker. Non-UI work routes to `executor` (Sol); UI and design to `secondary-executor` (Sonnet) or the Opus lead. Inspect a worker's first `model_change` record in its session file before trusting its output; stop on a fallback model.

## Manual actions and blockers

Ordered for the first build day (2026-11-01), all founder-owned:

1. B6 create the public repo (lead commits the docs and scaffold, pushes only when asked).
2. B1 create NEAR testnet accounts.
3. B3 create the Phala workspace and confirm the credit.
4. B5 create the Validation Cloud endpoint.
5. B4 add the NEAR AI key when credits arrive.
6. B2 fund the derived EOA after P1.2 prints it.
7. D7, D8 when the page phase approaches.
