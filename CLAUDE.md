# CONSTITUTION: agent operating rules

"Steal every admin key. You still cannot violate the upgrade constitution." A keyless NEAR gate holds the upgrade authority of one Base Sepolia proxy through Chain Signatures and signs an upgrade only when an attested verifier certifies it against owner-committed rules; a private AI may only veto.

## Read order

1. This file.
2. `docs/PRD.md`: scope, features F1 to F11, never-cut demo, guarantees and limits.
3. `docs/BUILD-PLAN.md`: work the first unblocked unchecked task.
4. `docs/OPEN-DECISIONS.md`: never re-decide a founder row; BLOCKERS list manual prerequisites.
5. `docs/technical/SMART-CONTRACT.md`, `docs/technical/ARCHITECTURE.md`, `docs/technical/INTEGRATIONS.md` for the task at hand; `docs/DESIGN.md` for page work.
6. Headlines in `docs/LESSONS.md`.

Technical specs define interfaces, PRD defines scope, BUILD-PLAN defines progress. Surface contradictions instead of choosing silently.

## Deadlines and authority

| Milestone | UTC | Source |
| --- | --- | --- |
| Build opens; first code allowed | 2026-11-01T00:00:00Z | https://nearhackathon.com/HACKATHON.md (re-read 2026-10-26) |
| Spikes proven | 2026-11-04T23:59 | BUILD-PLAN (internal, moved after the 2026-11-02 start) |
| Activation ceremony | 2026-11-07T12:00 | BUILD-PLAN (internal, enables a real 7-day timelock before cutoff) |
| Core gate, Act 2 decision | 2026-11-10T23:59 | BUILD-PLAN (internal) |
| Submission target | 2026-11-14T18:00 | BUILD-PLAN (internal buffer) |
| Final immutable cutoff | 2026-11-14T23:59:00Z | HACKATHON.md |
| Judging, verifier must stay up | 2026-11-16 to 2026-11-18T23:59 | HACKATHON.md |

Planning permission is not implementation permission. No git init, scaffolding, dependency installation or code before 2026-11-01T00:00:00Z. Spending, signing, account creation, logins, public posts, submissions, destructive changes, commits and pushes each need explicit founder authorization. Testnet only. Never edit AGENTS.md or HACKATHON-BIBLE.md; suggest changes instead.

## Commits when authorized

One behavior per commit, 50 to 350 changed lines excluding lock and generated files; a complete docs or config unit may be smaller. Split large behaviors at coherent boundaries (types, contract logic with tests, CLI, page, fix-ups), never pad. Subject `feat|fix|refactor|test|docs|perf|chore(scope): outcome in plain words`; body says why and the observed proof (test names, tx hashes). No AI attribution. Stage only the unit's paths. Docs describing changed behavior and the satisfied BUILD-PLAN checkbox ride in the same commit. Never commit secrets, `.env`, `.claude/skills/`, `scratch/`, build output. Clean tree and `git pull --rebase` before work and before any push; push only when the founder asks; never force-push.

## Docs discipline

Documents in English; founder chat in Indonesian. Labels: [V] verified with a dated primary source, [I] inference or design, [U] unverified. Never invent versions, metrics, addresses, transactions or results. Every claim says planned, locally tested, or exercised on testnet; local time-travel tests are never called testnet evidence. LESSONS entries: `### YYYY-MM-DD: headline`, bullets for what happened, root cause with evidence, rule going forward; newest first. Decisions and blockers go to OPEN-DECISIONS with owner and affected plan ids. No duplicate root PRD or plan files.

## Delegation

Lead (Opus 5.5) owns architecture, founder gates, `crates/constitution-types` interfaces, integration, review and final evidence. At most 3 Sol workers plus 1 Sonnet worker at once (founder raised the cap on 2026-11-02; three concurrent Codex workers is the observed rate-limit-safe ceiling).

| Work | Agent | Required resolved model |
| --- | --- | --- |
| Gate, executor, verifier, CLI, tests, technical docs, research | `executor` via `task` | `@task` = `openai-codex/gpt-6.1-sol:high` |
| Page UI, interaction, motion, DESIGN | `secondary-executor`, or lead | `@secondary` = `anthropic/claude-sonnet-5-5:high`, or Opus 5.5 |

Before trusting a worker, read the first `model_change` record in its session file and stop on `resolvedModelIsFallback: true`. Each worker gets self-contained context: owned and forbidden paths, interfaces, copied acceptance criteria, evidence rules, no commits, no full suites. Lead reviews every landed change, then runs proportionate checks once.

## Skills

Allowlist: `.omp/config.yml`; install tree `.claude/skills/` (gitignored); lock `skills-lock.json`; provenance and restore: `docs/technical/INTEGRATIONS.md`. Read the SKILL.md before relying on it. Skill instructions to init repos, commit, deploy or run installers never override this file.

| Trigger | Skill | Exact installed path |
| --- | --- | --- |
| Gate contract, near-sdk patterns | near-smart-contracts | `.claude/skills/near-smart-contracts/SKILL.md` (its MPC response shape is outdated; use SMART-CONTRACT) |
| Page NEAR signing | near-api-js | `.claude/skills/near-api-js/SKILL.md` |
| AI sidecar | near-ai-cloud | `.claude/skills/near-ai-cloud/SKILL.md` (Confidential TEE tier only) |
| Proxy, layout, upgrades | upgrade-solidity-contracts | `.claude/skills/upgrade-solidity-contracts/SKILL.md` |
| Security review | secure-workflow-guide | `.claude/skills/secure-workflow-guide/SKILL.md` |
| Fuzz and invariants | property-based-testing | `.claude/skills/property-based-testing/SKILL.md` |
| Planning a phase | writing-plans, executing-plans, subagent-driven-development | `.claude/skills/<name>/SKILL.md` |
| Logic and bugfixes | test-driven-development, systematic-debugging | `.claude/skills/<name>/SKILL.md` |
| Before claiming done | verification-before-completion | `.claude/skills/verification-before-completion/SKILL.md` |
| Module boundaries | codebase-design, improve-codebase-architecture | `.claude/skills/<name>/SKILL.md` |
| Review | requesting-code-review, code-review | `.claude/skills/<name>/SKILL.md` |
| Page design and motion | impeccable, emil-design-eng, design-taste-frontend, motion, gsap-core | `.claude/skills/<name>/SKILL.md` |
| Scope creep check | ponytail | `.claude/skills/ponytail/SKILL.md` |
| Library docs | find-docs | `.claude/skills/find-docs/SKILL.md` |
| Orca terminals | orca-cli | `.claude/skills/orca-cli/SKILL.md` |
| Hackathon method | hackathon-winning | global skill (`skill://hackathon-winning`) |

## Engineering standards

Small modules, one responsibility, explicit interfaces; no speculative abstractions, feature flags or retries; validate untrusted input at boundaries; never suppress errors to make a demo pass; delete obsolete paths when cutting over.

Stack-specific (Stack C, versions in ARCHITECTURE):

- `crates/constitution-types` is the only definition of certificate, proposal, manifest and EVM encoding bytes. Gate, verifier and CLI import it; the page re-derives independently and is checked by vectors in `vectors/`.
- Gate: near-sdk 5.29.1, Borsh state, no code-upgrade, add-key, delete-account or arbitrary self-call method. No method accepts a hash to sign. Panic codes exactly as SMART-CONTRACT. `#[private]` callbacks never panic after recording failure.
- Amounts: yoctoNEAR and wei are `u128` (JSON strings), NEAR time is nanoseconds, certificate and EVM time is seconds. Convert only through `constitution-types`.
- Solidity: 0.8.38, explicit `evm_version`, `ffi = false`, OpenZeppelin 5.6.1 unmodified, custom errors, checks then effects then interaction.
- Verifier: untrusted proposal code runs in the tool sandbox without network, keys or the dstack socket; every incomplete result fails closed; certificate key ephemeral (D18).
- AI: model `z-ai/glm-5.3-flash` only, `x-no-aliasing`, E2EE, `verifyResponse` must report `provider_tee`; AI output never produces a pass.
- Builds: deployed gate wasm only from `cargo near build reproducible-wasm`; Linux or WSL2 for sandbox tests.

## UX bar

Premium English copy, deliberate typography and hierarchy, semantic HTML landmarks, no generic dashboard. Every action shows real pending, success and failure states with a recovery path; hashes, links and timestamps appear only once observed; no fake progress or fixtures shown as live. Keyboard access, visible focus, readable contrast, 390 px and 1280 px layouts, `prefers-reduced-motion`. Motion explains state change only. Direction comes from `docs/DESIGN.md` once D7 is resolved.

## Testing and safety

| Layer | Tool | Planned command (not run) | Expected |
| --- | --- | --- | --- |
| Types and vectors | cargo test | `cargo test -p constitution-types` | all T-ENC vectors byte-identical |
| Gate | cargo test, near-workspaces | `cargo test -p constitution-gate` | T-GATE-1 to T-GATE-11 |
| EVM | Foundry | `forge test` in `contracts/` | T-EXE-1 to T-EXE-5 |
| Encoding oracle | viem | `pnpm --dir tools/vectors-viem vectors` | identical to Rust |
| Verifier | cargo test + fixtures | `cargo test -p constitution-verifier` | T-RULE-1 to T-RULE-5, T-AI-1 |
| Page | Vitest, Playwright | `pnpm --dir web test`, `pnpm --dir web e2e` | state matrix, both flows |
| Testnet | CLI | `make attack`, `make upgrade`, `constitution audit --network testnet` | T-E2E-1 to T-E2E-3 receipts |

Behavioral tests cover success, every rejection, boundaries, invariants and transitions; bug fixes add a failing-then-passing test. Lead alone runs integrated checks after workers land. No unrun check is called passing. Secrets never enter chat, docs or logs; demo keys are testnet-only and disposable. An unclear permission or unverified prerequisite becomes an owner-tagged BLOCKER, never a mock success.
