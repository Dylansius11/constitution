# CONSTITUTION: design direction and interaction spec for the public page

English only. Direction rewritten 2026-11-07 after founder decision D7; sections 6 to 11 (state matrix, receipts, accessibility, copy, data honesty) predate it and still bind.

Labels: [V] verified with a dated source, [I] design inference, [U] unverified.

## TL;DR

- **D7 resolved 2026-11-07 (founder):** follow the brand and the founder's reference video (`asset/reference.mp4`, gitignored), then elevate it. The October directions A, B and C, the old reference board and the "no purple" anti-goal are retired.
- **World:** warm paper ground, near-black ink, one lavender used as a flat fill, thin lavender line art that explains the mechanism. Large tight grotesk headlines, small calm body, mono only for data, tags and commands.
- **Signature moment:** the pinned story. One scroll walks a drain disguised as a fee update to a refusal on the night ground (`PROPOSED · CHECKED · REFUSED`), then the ground turns lavender for an honest upgrade to execution (`CERTIFIED · SIGNED · EXECUTED`). It is labeled an illustration of the flow, never chain data.
- **Entrance (founder request 2026-11-07):** once per browser session the page opens on a short night curtain: the mark assembles, the wordmark rises, the certified line crosses and splits the ground onto the hero. It is skippable, bounded and never a loader (section 4).
- **Honesty rules hold everywhere:** pending items never show fake links or fake progress; hashes, links and timestamps appear only once observed; local, spike and rehearsal evidence is always labeled; stale data never looks live; the AI is shown as veto-only; no claim beyond PRD guarantees G1 to G4.
- **Today's data reality (2026-11-07):** no production gate is deployed and the CVM is off. The page therefore renders every live surface in `Awaiting activation`, shows recorded spike evidence with its label and date, and re-reads the spike's real Base Sepolia transaction on request. Live surfaces switch on from `PUBLIC_*` configuration once an activation record exists.

## 1. Thesis

A judge must get *what it is, why it matters, how to verify it* in ten seconds, and feel *calm under attack*: when the drain is refused the page does not flash or shake; it states the rule, "no signature was requested", and "implementation unchanged". Calm is the evidence.

10-second test at 1280x800 with no scrolling: (1) what it is, (2) testnet and unaudited, (3) where to see the attack, (4) where to try it. Fail any and the hero is wrong.

Anti-goals: glowing gradients, gradient text, glass, mascots, logo walls, vanity metrics, loading screens that fake progress or hold the page, autoplay media, fake dashboards, counters that imply progress, scroll that traps the keyboard. The one entrance curtain (section 4) obeys every limit there.

## 2. Brand and reference

**Brand (founder, `asset/`, gitignored):** the mark is a geometric asterisk of nine bars around a negative-space triangle with a T stem at the base; the wordmark is `Constitution` in a round geometric sans. The page redraws the mark as inline SVG (`web/src/components/ui/Mark.astro`, three filled paths traced from `logo-only-black.png`, `currentColor`), so it themes and animates. The brand's lavender streak background is not reproduced; its lavender becomes the accent.

**Reference (founder, Alia site scroll-through, frames read 2026-11-07 at 1 s, 4 s, 9 s, 14 s, 20 s, 25 s, 31 s, 42 s) [V]:**

| Reference pattern | CONSTITUTION use |
| --- | --- |
| Off-white ground, ink text, lavender flat CTA fill, outlined secondary | Same tokens; `See the attack` filled, `Read the rules` outlined |
| Hero: thin lavender lines weave, converge through a circle node, exit as one line; headline bottom-left, pitch and CTAs bottom-right | Authority lines (admin, governor, deployer, relayer, verifier operator) run toward the proxy and are stopped at the gate node; only the certified path passes through |
| Full-bleed lavender section, pinned, split headline with a product card between, mono step indicator | The story: split headline per act, product-like cards (proposal, stage ladder, verdict, signature, execution) between, `PROPOSED · CHECKED · REFUSED` / `CERTIFIED · SIGNED · EXECUTED` |
| "How it works": vertical line and node, lavender mono number chip, product card | Six numbered steps on one rail; the number chips carry the real order of the pipeline |
| Lines that branch into parallel tracks and rejoin | The verdict fork: every proposal enters one line, splits into certify and refuse tracks; the AI track joins only the refuse side |
| Dark footer with a large logo | The wordmark runs the full screen width, the mark in lavender, letters rise once |
| Solana "Agent Skills" code block with copy and a Human/Agent toggle | `Try it`: real CLI commands for humans; `/llms.txt` for agents |

## 3. Tokens

One file: `web/src/styles/tokens.css`. Component CSS never uses raw colors. Contrast computed 2026-11-07 with the WCAG relative-luminance formula.

| Token | Value | Use and measured contrast |
| --- | --- | --- |
| `--paper` | `#F2F1E8` | page ground |
| `--paper-2` | `#E6E4D3` | alternate band (how it works) |
| `--surface` | `#FBFAF5` | cards, receipts |
| `--ink` | `#17151F` | text; 15.92 on paper, 7.37 on lavender |
| `--ink-muted` | `#55525F` | secondary text; 6.72 on paper, 5.95 on paper-2. Never on lavender (3.11) |
| `--ink-on-lav` | `#2A1F4D` | secondary text on lavender; 6.12 |
| `--lav` | `#B794F6` | fills, line art, focus halo. Never text on paper (2.16) |
| `--violet` | `#5B3CC4` | links and focus ring on light grounds; 6.41 on paper |
| `--line` | `#D6D3C4` | decorative hairline |
| `--line-strong` | `#8A8676` | control borders; 3.22 on paper |
| `--night` | `#121019` | footer ground; `--night-text` `#EDEBF5` 15.98, `--night-muted` `#A9A5B8` 7.87, lavender 7.70 |
| `--pass` / `--refuse` / `--pending` | `#1E6B47` / `#B3261E` / `#7A5200` | verdict state only, always with a word and a marker shape; 5.70 / 5.77 / 6.10 on paper |

**Type:** Geist (variable, OFL 1.1) for everything, Geist Mono (variable, OFL 1.1) for hashes, addresses, commands, step tags and the story indicator. Self-hosted through Fontsource `@fontsource-variable/geist` and `@fontsource-variable/geist-mono` 5.3.0 (licence field `OFL-1.1`, npm registry read 2026-11-07). Display: weight 600, tracking -0.035em, line-height 0.98; h1 clamps 2.6 rem to 5.2 rem. Body 1.0625 rem / 1.55, measure 62ch. Data: `tabular-nums slashed-zero`.

**Shape:** radius 4 px for buttons, chips and tags (the reference is near-square); 10 px for cards. Elevation is a border or one soft tinted shadow, never both.

## 4. Motion

Library choice: **GSAP 3.15.1 with ScrollTrigger and MotionPathPlugin** (standard no-charge licence, both plugins ship in the `gsap` package, npm registry read 2026-11-07) for the entrance, the hero, and the scroll scenes. Component micro-interactions and scroll reveals are CSS transitions. Motion (motion.dev) is not shipped; one library, one scroll model.

| Scene | What moves and what it explains | File |
| --- | --- | --- |
| Entrance | Night curtain, once per session: the mark's three parts lock together inside a drawn ring, `Constitution` rises letter by letter, the certified line crosses the screen and the ground splits open onto the hero (about 2.9 s). Any key, click or tap ends it; a 5 s fallback always releases the page. Never with reduced motion, without JavaScript, on a deep link (`#hash`) or a second time in the session. No percentage, bar or "loading" copy: it is a curtain, not progress. | `motion/Intro.astro`, `intro.ts` |
| Hero | After the entrance: headline words rise, `cannot` gets a lavender highlight wipe, the lines draw into the gate and the mark assembles. Then, only while the hero is on screen, a loop: an attempt travels down each stolen-key line and dies at its tick (the tick turns refuse red, a small ring fades); the certified attempt passes behind the mark and lands on the proxy, which fills lavender. | `motion/hero.ts`, `HeroLines.astro` |
| Story | Pinned by CSS `position: sticky`; GSAP reads progress only. Act one on the night ground, act two on lavender (700 ms ground swap); a 3 px rail shows scroll position through the illustration and turns refuse red on `REFUSED`; a large step numeral sits behind the card; act headlines rise from a 6 px blur. | `motion/story.ts`, `sections/Story.astro` |
| Rules | Each rule row plays its schematic once when it scrolls in: R1 appends a slot, R2 lights the deposit/withdraw sequence and grows the held and owed bars, R3 scans and strikes the forbidden opcodes, R4 matches the committed selector, R5 pairs old and new outputs. Every figure is labeled `Illustration`. | `motion/RuleFigure.astro`, `rules.ts` |
| Timelock | While the section scrolls past the sticky dial the seven days fill in order and a hand walks the ring; the count reads `Day N of 7`, then `Apply opens`. Scrubbed by scroll, an illustration of the fixed delay, never the clock of a real change. | `motion/timelock.ts` |
| Reveals | Blocks marked `data-reveal` rise 28 px and fade in once, staggered 80 ms. Armed by an inline script only with motion allowed; a 2.5 s fallback drops the start state if the observer never loads. | `motion/reveal.ts`, `global.css` |
| Terminal, footer | Command lines print in with a 70 ms stagger the first time the block scrolls in; the footer wordmark letters rise once. | `ui/CodeBlock.astro`, `motion/footer.ts` |

- Native scrolling is never hijacked, smoothed or snapped, so keyboard, find-in-page and screen readers work unchanged.
- Animate `transform`, `opacity`, `clip-path`, `filter` and SVG `stroke-dashoffset` only. No layout properties.
- Easing: `--ease-out: cubic-bezier(0.23, 1, 0.32, 1)` for entrances and state swaps, `--ease-in-out: cubic-bezier(0.77, 0, 0.175, 1)` for on-screen moves and the curtain split. Buttons press to `scale(0.97)` over 160 ms.
- What never animates: hashes, addresses, recorded figures (no scramble, no count-up), status text, the timelock remaining time of a real change, system progress (there is none). The story rail and the dial count describe the illustration being scrolled, not the system.
- **Static first.** Without JavaScript, or with `prefers-reduced-motion: reduce`, every scene renders its final, complete content: no entrance, the hero lines drawn with no moving attempts, the story as an ordered list of six steps each with its card, every rule figure finished, the dial full, all blocks visible. GSAP runs inside `gsap.matchMedia()` under `(prefers-reduced-motion: no-preference)` only.

## 5. Information architecture (single page)

One page, one `h1`, static HTML as the baseline, live reads as enhancement. Landmarks: `header` with skip link and `nav aria-label="Sections"`, `main`, `footer`. Every section is `section aria-labelledby` with an `h2`.

| # | Section (id) | Contents | Live data |
| --- | --- | --- | --- |
| 0 | Header | Mark and wordmark, nav (How it works, Rules, Timelock, Evidence, Limits), scope chip `Testnet · Not audited`, filled `Try it` | none |
| 1 | Hero (`top`) | h1 one-liner, authority line art, pitch of at most 20 words, `See the attack`, `Read the rules` | none |
| 2 | Story (`story`) | Pinned lavender scroll story, two acts of three steps, labeled illustration | none |
| 3 | How it works (`how`) | Six numbered steps on a rail: propose, verify, red-team, gate, Chain Signature, execute; the verdict fork | none |
| 4 | Rules (`rules`) | R1 to R5 rows: large rule id, what it checks, where it stops, and a labeled schematic figure per rule; the AI veto note; committed manifest facts read at build from `rules/manifest.json` | gate `get_constitution` once configured |
| 5 | Timelock (`timelock`) | 7-day clock, roles (governor schedules, guardian cancels, nobody applies early), `E53_TIMELOCK_NOT_ELAPSED`, the queue | gate `list_changes` once configured |
| 6 | Evidence (`evidence`) | Four recorded figures, each with its source label (spike signature, its gas, local gate tests, committed rules); status ledger per component; recorded spike receipt with a live re-check of the Base Sepolia transaction and proxy slot; proposal watcher | Base Sepolia RPC now; gate and verifier once configured |
| 7 | Try it (`try`) | In the browser: `Run the attack` and `Run the honest upgrade` cards (PRD F10) with the stages a run reports; from a checkout: a terminal window with the Human/Agent commands that wrap instead of scrolling sideways, and `/llms.txt` | gate, verifier and Base Sepolia once configured |
| 8 | Limits (`limits`) | G1 to G4, trust assumptions, not claimed | none |
| 9 | Footer | links, scope, build time, then the full-width mark and wordmark; repo and X links only when configured | none |

**Activation-gated surfaces.** The in-browser Attack and Honest upgrade runs, the QR "take it" key (5.1) and the audit panel (6.18) need the activated gate, the CVM verifier and the disposable demo key. Until the run configuration is complete, both run cards render locked: `Awaiting activation` badge, a disabled `Opens at activation` button that says why, and a collapsed list of the stages a run will report; nothing on a locked card claims to have happened. With configuration, the same cards run `web/src/lib/run-flow.ts` (seal, `propose` with the demo key, verifier run, gate and Base observation, relay of a Signed transaction, slot readback) and render the state matrix below; a reload resumes observing and never resubmits. The QR card and the audit panel are not rendered before activation. Their specs below stay binding.

**Configuration (all optional, read at build):** `PUBLIC_NEAR_RPC`, `PUBLIC_GATE_ACCOUNT`, `PUBLIC_VERIFIER_URL`, `PUBLIC_BASE_RPC` (default `https://sepolia.base.org` for reads), `PUBLIC_REPO_URL`, `PUBLIC_X_POST_URL`. The run cards additionally need `PUBLIC_DEMO_ACCOUNT_ID`, `PUBLIC_DEMO_SECRET_KEY` (the disposable D17 function-call key, public on purpose) and `PUBLIC_{ATTACK,HONEST}_{IMPL,CODEHASH,CURRENT_IMPL,INIT_CALLDATA}`; any missing or malformed value keeps them locked. A missing gate account means `Awaiting activation`, never a sample value.

### 5.1 QR "take it" card (activation-gated)

- `figure` containing the QR as inline SVG with `role="img"` and a text alternative: `QR code. Encodes a disposable, testnet-only NEAR key that holds the demo governor role. Not a production key.` The key itself is not put in `alt` (too long).
- `figcaption` states: `This key is public on purpose. It can only act on the demo. Never reuse it.`
- Text alternative for people who cannot scan: `button` "Copy key" and a `details` "Show key as text" that reveals the key in `code`. A keyboard and screen-reader user has full parity.
- Size: at least 280 px square at 1280, 220 px at 390 [I], dark modules on light quiet zone in every direction.
- Never shown: any production key, any full-access key of the gate, the MPC-derived EVM key or any founder wallet (PRD never-cut demo, step 1).
- The key never persists: no localStorage, no cookies.

### 5.2 Section order rationale

Claim, then the story of one attack and one honest upgrade, then the mechanism, then what is committed (rules, timelock), then the paper (evidence), then how to check it yourself, then the limits. A judge who stops after the story already knows what, why and where to verify.

### 5.3 Navigation

Single line at 1280: `How it works, Rules, Timelock, Evidence, Limits` plus the filled `Try it`. At 390 the links collapse into a native `details` disclosure; the filled `Try it` stays visible. The header is sticky with a paper background; `scroll-padding-top` equals its height so focused elements and anchors are never hidden.

## 6. State matrix

### 6.0 Machine and sources

Attack and Honest upgrade share one machine. The page never presumes the outcome: the Attack surface can end in a verdict other than Rejected if the verifier does not reproduce a counterexample, and the copy must stay outcome-neutral until a verdict is observed.

Mapping from observed data to UI state (sources from ARCHITECTURE.md and SMART-CONTRACT.md):

| UI state | Observed source |
| --- | --- |
| idle | no active run for this surface |
| submitting | `propose` sent with the demo key, no proposal id observed yet |
| verifying | verifier run status `GET /v1/runs/{run_id}` stage in `queued`, `unsealing`, `compiling`, `r1_layout`, `r2_solvency`, `r3_filter`, `r4_initializer`, `r5_replay` |
| ai review | stage `ai_review` or `reproducing` |
| certified | verdict `Pass` recorded on the gate |
| signing | gate state `SignRequested` |
| signing (retry) | gate state `SignFailed` |
| relaying | gate state `Signed`, EVM status `signed` or `pending` |
| executed | EVM status `included` and success, and ERC-1967 implementation slot after equals the expected new implementation |
| rejected | verdict `Reject {rule_id, reason_code}`, or gate state `Rejected` |
| expired | gate state `Expired` |
| review incomplete | AI review incomplete (timeout, error, wrong signature kind) |
| reverted | EVM status `reverted` |
| error | a call failed with a cause that is not one of the above |
| network unavailable | RPC, verifier or receipts fetch failing |

Verifier stages `signing`, `submitting` and `done` are the verifier's own tail (signing the verdict, recording it on NEAR). In the UI they are shown inside the verifying state as `Signing the verdict` and `Recording the verdict on NEAR`. The UI word **Signing** is reserved for the Chain Signature (MPC) step to avoid confusion.

### 6.1 Cross-cutting rules (apply to every state)

1. **No fake progress.** No percentage, no progress bar, no ETA, no `Step n of m` (the AI stage is skipped when a deterministic rule refuses, so the count is not fixed). Progress is shown as an ordered stage list where each stage has its own observed status.
2. **Liveness without spinners.** A static marker plus `Last checked 14:02:11 UTC` that updates on each poll. If the last check is older than 3 polls, the label becomes `Last checked {time} UTC. Retrying.`
3. **Observed only.** A timestamp, hash or link renders only after it was observed. Before that the field reads `Pending` (or `Not reached` if the path ended earlier). `Pending` never links anywhere.
4. **Explorer links** appear only after the page itself has read the item from the chain (section 7.4).
5. **Stage marker shapes** (in addition to text): Pending: hollow ring and the word `Pending`. In progress: filled ring and `In progress`. Done: check and `Done`. Failed: cross and `Refused` or `Failed`. Skipped: bar and `Not run` or `Not reached`. One icon family, drawn as inline SVG [family choice deferred to build, U].
6. **Timestamps** are UTC, `2026-11-03 14:02:11 UTC`, in `time datetime`. Relative text is secondary and visible, never hover-only.
7. **Focus:** after a terminal state caused by a user submission, focus moves to the receipt heading (`tabindex="-1"`) only if focus is still inside that surface; otherwise a polite message offers a `Jump to receipt` link. Focus is never stolen from another surface.
8. **Controls:** while in flight, the submit button is `aria-disabled="true"` with the text `Submitted` (not removed from the tab order, so focus is not lost).

### 6.2 Idle

- **Trigger:** no active run.
- **Visual:** quiet panel, fixture description, one primary button, empty stage list in `Pending` state, recorded receipt link if one exists.
- **Copy, Attack:** title `Submit VaultV2Drain`. Body `Looks like a fee update. Contains a hidden path that lets one address withdraw the whole vault. Submit it with the demo key and watch the constitution decide.` Button `Submit VaultV2Drain`. Helper `Uses the public demo key. Nothing here can touch anything but the demo vault.`
- **Copy, Upgrade:** title `Submit VaultV2Honest`. Body `Adds a feature with append-only storage. It is built to pass every committed rule. Submit it and watch what the constitution decides.` Button `Submit VaultV2Honest`. Helper as above.
- **Live:** none.
- **Recovery:** if a previous run for this surface exists, link `View the last receipt` (anchor to `#receipt-{id}`).

### 6.3 Submitting

- **Trigger:** `propose` broadcast, proposal id not yet observed. If the page seals the source bundle in the browser, the first sub-stage is `Sealing the source bundle` (see Q1).
- **Visual:** stage ladder with the first stage `In progress`; button `aria-disabled` reading `Submitted`.
- **Copy:** title `Registering the proposal`. Body `Signing with the demo key and sending it to NEAR testnet. Waiting for the gate to record a proposal id.`
- **Live:** polite, once on entry.
- **Recovery:** none while in flight (cannot cancel a broadcast). After 60 s without observation: `Still waiting for NEAR testnet to report this proposal. The transaction may still land.` plus the transaction hash as plain text (link only after NEAR reports it). Action `Check again`.

### 6.4 Verifying (stage names)

- **Trigger:** verifier stage in the deterministic set.
- **Visual:** the ladder, one stage `In progress` at a time, finished stages show `Done` or `Refused` plus their own observed detail, later stages `Pending`. `Started 14:02:11 UTC` shown once.
- **Copy:** title `Verifying`. Body `The attested verifier is checking {fixture} against the committed rules. Source stays inside the enclave.` Stage labels:

| Verifier stage | UI label | One line shown under the label |
| --- | --- | --- |
| `queued` | Queued | Waiting for the verifier to start. |
| `unsealing` | Opening the sealed bundle | Unsealed inside the attested verifier. The hash must equal the commitment. |
| `compiling` | Compiling | Pinned compiler. The runtime code hash must equal the proposal. |
| `r1_layout` | R1 Storage layout | The new layout must extend the current one, append only. |
| `r2_solvency` | R2 Solvency | No sequence within the bound may let an account withdraw more than its balance. |
| `r3_filter` | R3 Opcode filter | No reachable `SELFDESTRUCT`, `CALLCODE` or `DELEGATECALL` to a non-constant target. |
| `r4_initializer` | R4 Initializer allowlist | Init calldata must be empty or match a committed selector and schema. |
| `r5_replay` | R5 Differential replay | A committed corpus replays against old and new code. Outputs must match except declared changes. |
| `signing` | Signing the verdict | The verifier signs its verdict inside the enclave. |
| `submitting` | Recording the verdict on NEAR | The signed verdict is sent to the gate. |

- **Live:** polite. Announce each stage start with `{stage label}, in progress`. Coalesce so no more than one announcement per 2 seconds.
- **Recovery:** none needed; if no stage change for 90 s, show `The verifier has not reported progress since {time} UTC. It may be busy. Your proposal is already recorded.` and `Check again`.

### 6.5 AI review

- **Trigger:** stage `ai_review` or `reproducing`. If a deterministic rule already refused, the AI stage shows `Not run` with the line `A deterministic rule refused first.`
- **Visual:** two stage rows: `Private red-team review` and `Reproducing candidate`. A boxed line below: `The AI can only refuse. It cannot approve.`
- **Copy:** title `Private red-team review`. Body `A private AI is searching for a counterexample inside the enclave. Anything it proposes must be reproduced deterministically before it can refuse anything.`
- **Live:** polite on entry and on `Reproducing candidate`.
- **Recovery:** none. If it finishes with nothing reproduced: row reads `No candidate counterexample reproduced.`

### 6.6 Certified

- **Trigger:** verdict `Pass` recorded on the gate (`AiReview::Clean`).
- **Visual:** ladder up to the verdict `Done`, `--pass` verdict marker and word `Certified`. The receipt article appears with `Pending` in the signing and EVM fields.
- **Copy:** title `Certified`. Body `All {n} rules passed under rule set {hash short}. No candidate counterexample reproduced. Next, the gate asks the MPC network to sign this exact transaction.`
- **Live:** polite.
- **Recovery:** none.

### 6.7 Signing (Chain Signature)

- **Trigger:** gate `SignRequested`.
- **Visual:** ladder row `Chain Signature` `In progress`; EVM fields `Pending`.
- **Copy:** title `Signing`. Body `The gate asked the NEAR MPC network to sign the exact upgrade transaction. Waiting for the signature.`
- **Live:** polite.
- **Recovery (signing failed):** gate `SignFailed`. Title `Signature request failed`. Body `The proposal is still certified. Retrying signs the identical transaction with the same nonce.` Button `Retry signing` [needs Q6]. Live: assertive once (`role="alert"` on first appearance), then polite.

### 6.8 Relaying

- **Trigger:** gate `Signed`; EVM status `signed` or `pending`.
- **Visual:** the signed transaction hash is shown as plain monospace text with copy. No link. EVM block field `Pending`.
- **Copy:** title `Signed, waiting for Base Sepolia`. Body `The gate holds a signed transaction. Anyone can broadcast it. It is not on Base Sepolia yet.` Helper `A relayer will send it. You can also run constitution relay {proposal id} yourself.` (command is copyable).
- **Live:** polite.
- **Recovery:** if EVM status stays `signed` for 3 minutes: `Not broadcast yet. A relayer has not sent this transaction. Run constitution relay {proposal id}.` If `pending` for 3 minutes: `Broadcast but not yet included. The gate can re-sign the same transaction with a higher fee up to the committed cap.` (the fee bump is a gate capability, F3).

### 6.9 Executed

- **Trigger:** EVM `included` with success, and the page read implementation after equal to the expected implementation.
- **Visual:** `--pass` verdict marker and word `Executed`. `before` and `after` implementation lines set in the same mono columns with the differing characters highlighted. Explorer links appear (7.4).
- **Copy:** title `Executed on Base Sepolia`. Body `Block {n}. The proxy implementation changed from {before short} to {after short}. This upgrade passed through the gate, was signed by the MPC network, and executed through the constitution executor.`
- **Live:** polite.
- **Recovery:** link `View the receipt`, `Run the attack`.

### 6.10 Rejected

- **Trigger:** verdict `Reject {rule_id, reason_code}` or gate `Rejected`.
- **Visual:** verdict word `Refused` in `--refuse` with the rule id (`R2`). Ladder rows after the verdict show `Not reached`. EVM rows read `Not requested`, never `Pending`. Proxy line: `before` and `after` equal, set character for character in the same mono columns with `=` between them, `unchanged, observed at block {n}`, rendered only after two observed reads agree.
- **Copy:** title `Refused under {rule_id} {rule name}`. Body `{plain reason from the evidence, for example: The verifier reproduced a sequence in which address {addr} withdraws {pct}% of the vault.} No signature was requested. The proxy implementation is unchanged.` If evidence detail is not available: `The verifier reproduced a sequence that violates {rule name}. No signature was requested. The proxy implementation is unchanged.` The two last sentences each render only when their facts are observed: `No signature was requested` requires gate state `Rejected` and no `sign_requested` event; `unchanged` requires the two slot reads.
- **Live:** polite (a verdict is information, not an emergency).
- **Recovery:** `View the evidence hash` (copy), `Run the upgrade`, and the line `The reviewer can only refuse, never approve.`

### 6.11 Expired

- **Trigger:** gate `Expired` (from `Pending` or `Certified`).
- **Visual:** neutral marker (`--pending`), word `Expired`, later rows `Not reached`.
- **Copy:** title `Expired`. Body `This proposal was not completed before its deadline and can no longer be certified. Nothing was signed. The proxy implementation is unchanged.` (`unchanged` follows the same observed rule.)
- **Live:** polite.
- **Recovery:** `Submit again` creates a new proposal (new id).

### 6.12 Review incomplete

- **Trigger:** AI timeout, error, malformed output or a signature kind other than the required one (F6, T-AI-1).
- **Visual:** `--pending` marker, word `Incomplete`, AI row `Incomplete`. Explicitly not styled as pass or refuse.
- **Copy:** title `Review incomplete`. Body `The private AI review did not finish, so nothing was certified. This is not a pass and not a refusal. It can be retried until {deadline} UTC.`
- **Live:** polite.
- **Recovery:** button `Retry review` while before the deadline [needs Q6]; after the deadline the state becomes `Expired`.

### 6.13 Reverted (EVM)

- **Trigger:** EVM status `reverted`.
- **Visual:** `--refuse` marker, word `Reverted`.
- **Copy:** title `Reverted on Base Sepolia`. Body `The executor refused this transaction. The proxy implementation is unchanged. Run constitution receipt {id} for the revert reason.`
- **Live:** assertive once, then polite.
- **Recovery:** explorer link to the transaction (observed), copy command.

### 6.14 Error

- **Trigger:** any call failure not covered above. Each error states cause and next step in one place.
- **Visual:** `--refuse` outline panel, cross marker, cause in plain words, technical code in mono below.
- **Copy:** title `Could not {action}`. Body `{cause in plain words}. {next step}.` Variants:

| Cause | Body | Recovery |
| --- | --- | --- |
| Gate refused the proposal at registration (F1): wrong target, wrong chain, rule set changed mid-proposal, duplicate active proposal, rate limit or credit, malformed lengths | `The gate refused this proposal: {plain reason}. Nothing was recorded.` For duplicate: `This exact implementation and calldata already has an active proposal.` For credit or rate limit: `The demo account has reached its proposal limit. Use the CLI with your own account, or read the receipts below.` | `Close`, link to the CLI instructions in the README |
| Verifier unreachable | `The verifier did not respond. Your proposal {id} is recorded on NEAR. The verdict appears when the verifier returns.` | `Check again` |
| Broadcast failed | `The transaction could not be broadcast: {reason}. It is still signed and held by the gate.` | `Copy relay command` |
| Receipt does not match schema `constitution.receipt/1` | `This receipt does not match the expected format, so it is shown as raw JSON. Treat its contents as unverified.` | `Download raw JSON` |

- **Live:** assertive (`role="alert"`) once.
- **Recovery:** always one visible action, never a dead end.

### 6.15 Network unavailable

- **Trigger:** three consecutive failed fetches to NEAR RPC, Base RPC, the verifier API or the receipts JSON.
- **Visual:** a boxed banner at the top of the affected surface (and the ledger strip), text `Not live`. Last observed values stay visible but each gets `Last observed {time} UTC` and a hatched or dashed treatment; live styling (verdict colors) is removed from stale data.
- **Copy:** title `Cannot reach {Base Sepolia | NEAR testnet | the verifier}`. Body `Showing the last observed state from {time} UTC. Nothing here is live until the connection returns.`
- **Live:** assertive once on first failure, polite afterwards and on recovery (`Connection restored. Live data resumed.`).
- **Recovery:** `Retry now` (also retries automatically with backoff [I]: 2 s, 4 s, 8 s up to 30 s; stop polling when the tab is hidden).

### 6.16 Receipts surface (mini matrix)

| State | Treatment and copy | Live | Recovery |
| --- | --- | --- | --- |
| Loading | Static placeholder rows with the text `Loading receipts`. No shimmer, no skeleton animation. | polite once | none |
| Empty | `No receipts yet. Submit the attack or the upgrade above and its receipt appears here.` | none | anchor links to `#attack`, `#upgrade` |
| Loaded | List newest first, each `article` with permalink `#receipt-{id}`. | none | n/a |
| Stale | Each card gets `Last observed {time} UTC` and the `Not live` badge. | polite | `Retry now` |
| Recorded fallback | Badge `Recorded rehearsal {date}`: `This is a real receipt from an earlier testnet run, not a live run.` | none | none |
| Invalid | Section 6.14 schema row. | assertive once | download raw |

### 6.17 Timelock queue surface (mini matrix, inside The constitution)

| State | Treatment and copy | Live |
| --- | --- | --- |
| No change queued | `No change queued. Rules, the verifier allowlist and authority change only after a public 7-day delay.` | none |
| Queued | Row: kind, scheduled by, `Scheduled {time} UTC`, `Applicable from {eta} UTC`, remaining time in days, hours and minutes. Updates in place every minute, no animation. | none (a countdown must not be announced) |
| Early apply refused | Receipt (type governance) with `Refused: TimelockNotElapsed` and mono code `E53_TIMELOCK_NOT_ELAPSED`. Body `This change cannot be applied before {eta} UTC. No role can shorten the delay.` | polite |
| Matured, not applied | `Applicable since {eta} UTC. Anyone may apply it.` | none |
| Applied | Only shown if observed on the gate: `Applied {time} UTC` with its receipt. Never inferred. | polite |
| Cancelled | `Cancelled {time} UTC by {role}` | none |
| Local time-travel test | Only in receipts, badge `Local test, not chain evidence`, never in the queue. | none |

### 6.18 Audit panel (mini matrix)

Checks (from F7): (1) gate account has zero full-access keys; (2) gate has no self-upgrade path outside the timelocked queue; (3) derived EOA equals the executor's authorized caller; (4) executor owns ProxyAdmin; (5) ProxyAdmin is the proxy's ERC-1967 admin; (6) active rule-set hash and verifier allowlist match the committed manifest.

| State | Treatment and copy |
| --- | --- |
| Not run | Each check `Not run`. Button `Run audit`. Helper `Reads public chain data in your browser. Nothing is sent anywhere.` |
| Checking | The check in progress shows `Checking`. No percentage. |
| Pass | `Pass` plus the observed value (for example the two equal addresses). |
| Fail | `Failed: observed {a}, expected {b}.` Panel header turns to refusal styling with `The claim that only the constitution can upgrade this vault does not hold while this check fails.` Assertive, once. |
| Cannot read | `Cannot read {source}. Not counted as a pass.` |
| CLI only | For any check the browser cannot compute (for example check 2 if it needs code analysis [U]): `Run constitution audit to check this.` Never shown as pass. |
| Network unavailable | Section 6.15. |

Panel summary line is `{n} of 6 checks passed, {m} not run` and appears only after a run. A green summary is impossible while any check is not run or failed.

## 7. Receipt anatomy

### 7.1 Structure

One `article aria-labelledby` per receipt. Heading `Receipt {proposal id}: {fixture or change kind}`. Inside: header block, then six `dl` groups in the order of PRD F8, then a footer with the canonical JSON.

### 7.2 Header block

- Title and permalink anchor (`#receipt-{id}`).
- **Evidence label** (a text badge with an icon, never color only):
  - `Testnet, observed`: data read live by this page. Solid border.
  - `Testnet, recorded {date}`: a real testnet receipt saved earlier (the rehearsal fallback). Solid border, the word `Recorded`. Never replayed as live (PRD never-cut demo).
  - `Local test, not chain evidence`: dashed border and hatch fill, no explorer links. Maps to receipt `labels.network = "local"`.
  - `Spike, recorded {date}`: real chain evidence from the minimal spike gate (no verifier in the loop). Solid border, the word `Spike`. Never presented as a production run.
- Verdict word and marker, plus the before and after implementation lines.
- Schema line in mono: `constitution.receipt/1`.

### 7.3 Field order and sources

Order matches PRD F8 and the receipt JSON in ARCHITECTURE.md.

| # | Group and field | JSON source | Format | Until observed |
| --- | --- | --- | --- | --- |
| 1 | Proposal: id | `proposal.id` | integer | n/a |
| 1 | Target proxy, new implementation, `new_impl_codehash`, `init_calldata`, `source_commitment`, expected current implementation | `proposal.*` (ProposalView) [U exact keys] | addresses EIP-55 checksummed and never lower-cased; hashes `0x` plus hex; init calldata `Empty` or length plus short hex | n/a (proposal exists) |
| 2 | Rule set hash | `rule_set_hash` | hash with copy | n/a |
| 2 | Rule results R1 to R5 | evidence manifest `r1..r5` `status`, `detail_code`, `artifact_sha256` | `table`: rule, result (`Pass`, `Fail`, `Incomplete`, `Not run`), reason code, evidence hash | `Pending` while verifying; `Not run` if a stage stopped earlier |
| 3 | AI review outcome | `ai.signature_kind`, `ai.reproduced_candidates`, outcome `Clean`, `Vetoed`, `NotRun`, `Incomplete` | text plus count `Candidates reproduced: {n}` | `Pending` |
| 4 | Verdict: certificate or rejection signature | `verdict.kind`, `verdict.rule_id`, `verdict.reason_code`, verifier pubkey, `verifier.release_hash`, `evidence_root` | `Certificate` or `Rejection`, signature hash, verifier key short, release hash short | `Pending` |
| 5 | NEAR transactions | `near.propose_tx`, `near.verdict_tx`, `near.sign_tx?` | hash with copy; link once observed by NEAR RPC | `Pending`; `sign_tx` reads `Not requested` on a refusal |
| 6 | EVM: nonce, signed transaction hash | `evm.nonce?`, `evm.tx_hash?` | nonce integer, hash with copy | `Pending`; `Not requested` on a refusal |
| 6 | EVM: inclusion status and block | `evm.status` (`not_signed`, `signed`, `pending`, `included`, `reverted`), `evm.block?` | text label (`Not signed`, `Signed, not broadcast`, `Pending inclusion`, `Included`, `Reverted`), block integer | `Pending` |
| 6 | Explorer link | `evm.explorer?` | link text `View on Base Sepolia explorer` | absent until observed |
| 7 | Proxy implementation before and after | `proxy.implementation_before`, `proxy.implementation_after`, `proxy.observed_at_block` | two hashes, `observed at block {n}` | `after` shows `Pending` until read; `unchanged` only after two equal reads |

Footer: `Download receipt JSON` (the exact bytes rendered, `download` attribute) and `Copy receipt JSON`.

### 7.4 Explorer link rules

1. A link to a transaction, address or block is rendered only after the page has itself retrieved that object from the corresponding RPC (found by hash). A signed hash from the gate event, before broadcast, is **text only**.
2. Link text states the destination: `View on Base Sepolia explorer` or `View on NEAR testnet explorer`. Never a bare arrow.
3. Links open in the same tab by default with `rel="noopener"`; external indicator in text (`opens the explorer`), not only an icon.
4. Explorer base URLs are configuration, not constants. Reachability observed this session: the NEAR testnet explorer page at https://testnet.nearblocks.io/ loaded [V]; https://sepolia.basescan.org/ returned a bot verification page to headless Chromium and was not bypassed [V], so its transaction URL pattern is [U] until the lead confirms the Base Sepolia explorer in SMART-CONTRACT or INTEGRATIONS (Q2).
5. Local test receipts have no explorer links at all.

### 7.5 Hash and address display

| Item | 390 px | 1280 px | Notes |
| --- | --- | --- | --- |
| Hash (66 chars) | `0x3fa9c2…e21e` (first 6 hex, last 4) | `0x3fa9c21b8d…c7e4a6e2` (first 10 hex, last 8) | Mono, tabular, `overflow-wrap: anywhere` when expanded |
| Address (42 chars) | `0x4200…0006` (first 4 hex, last 4), checksum casing preserved | `0x420000…000006` (first 6 hex, last 6) | Never lower-case a checksummed address |
| NEAR account id | full | full | Truncate only above 32 characters with a middle ellipsis |
| Expand | inline `button aria-expanded` that shows the full value in place | same | Not hover-only; no tooltip dependency |
| Copy | `button` `Copy` next to every hash; copies the full value | same | Confirms in a polite live region: `Copied transaction hash` |
| Screen reader name | `Transaction hash, starts 0x3fa9, ends e21e` as the accessible name of the code element, full value available via Expand | same | Avoid reading 66 characters aloud by default |

### 7.6 Receipt types

- **Upgrade receipt** (Attack and Upgrade): all groups above.
- **Governance receipt** (early-apply refusal, and an applied rule change only if it really matured): replaces groups 1 to 3 with `Change id`, `Kind` (`SetRuleSet` and the other kinds), `Scheduled by`, `Scheduled at`, `Applicable from (eta)`, `Apply attempted at`, `Result` with the human title `TimelockNotElapsed` and the code `E53_TIMELOCK_NOT_ELAPSED`. Groups 5 to 7 follow the same rules.
- **Audit report** is not a receipt; it is the Audit panel.

## 8. Accessibility and responsiveness checklist

Targets: WCAG 2.2 AA minimum, AAA contrast for body text where the palette allows (section 3 measures ink at 15.92:1 on paper).

**Keyboard and focus**
- [ ] Tab order follows DOM order: skip link, header nav, hero links, story (no controls), section links and copy buttons, evidence re-check, try-it toggle and copy, footer links. After activation: QR copy and disclosure, Attack and Upgrade submit, receipt expand and copy, audit run.
- [ ] Focus ring: 2 px solid `--violet` with 3 px offset on light grounds (6.41:1 on paper), `--ink` on lavender (7.37:1), `--lav` on night (7.70:1). Never removed.
- [ ] No sticky element obscures focus (2.4.11): `scroll-padding-top` on `html` equals the sticky header height.
- [ ] Every control reachable and operable with keyboard only; no hover-only information anywhere (tooltips are replaced by visible text or Expand buttons).
- [ ] Disclosures use native `details` and `button aria-expanded`.

**Semantics**
- [ ] One `h1`; headings strictly nested; each section `aria-labelledby`.
- [ ] `table` with `caption`, `th scope`; `ol` for timelines and queues; `dl` for receipt fields; `article` for receipts; `time datetime` for every timestamp.
- [ ] One `output` (implicit status role) per interactive surface, plus explicit `aria-live` where needed. Live regions exist in the DOM before content changes. Text in them is full sentences. Countdowns are never in a live region.
- [ ] `lang="en"`; descriptive `title`; every link text states its destination.
- [ ] QR has a text alternative and a text-key fallback (section 5.1).
- [ ] No letter-by-letter DOM splitting of headings; split headlines in the story are two real text nodes inside one heading.

**Color and contrast**
- [ ] Text at least 4.5:1, large text at least 3:1, UI component boundaries and focus indicators at least 3:1.
- [ ] State is never color alone: marker shape plus text label on every state (section 6.1).
- [ ] Respect `prefers-contrast: more` (raise `--line` to `--line-strong`) and `forced-colors: active` (markers and borders keep meaning through borders and text) [I].
- [ ] Stale data loses verdict colors (section 6.15).

**Targets and zoom**
- [ ] Interactive targets at least 44x44 CSS px [I, WCAG 2.5.8 requires 24].
- [ ] Reflow with no horizontal scroll at 320 CSS px and at 400 percent zoom, except the `pre`-like receipt JSON, which scrolls inside a focusable region with a label.
- [ ] Text resizes to 200 percent without loss.

**Layouts at the two required widths** (visual check at both, per F10 acceptance)

| Section | 390 px | 1280 px |
| --- | --- | --- |
| Hero | Line art on top at reduced height, h1 at about 42 px in 4 to 5 lines, pitch, CTAs stacked full width | Line art across the upper viewport, h1 bottom-left in 2 to 3 lines, pitch and two CTAs bottom-right |
| Story | Not pinned; six steps stacked, each with its card | Pinned split headline with the card between; mono indicator at the bottom |
| How it works | Rail on the left edge, steps stacked | Rail in the centre column, text left, card right |
| Rules | Each rule a stacked row with its limit below | Ruled table, id, rule, what it checks, limit |
| Evidence | Status rows stacked; hashes truncated to the 390 rule | Status ledger as ruled rows; spike receipt as a two-column `dl` |
| Footer | Stacked | Large mark left, links in a row |

**Performance and robustness [I]**
- [ ] Static HTML contains all non-live content and recorded receipts; page is understandable with JavaScript disabled.
- [ ] Fonts self-hosted, `font-display: swap`, subset to Latin; preload display face only.
- [ ] Polling pauses when the tab is hidden; cadence while a run is active 2 s, idle 15 s; backoff on errors [public RPC quotas are not published, U].
- [ ] Page performs no writes except the demo `propose` and optional gate calls; no cookies, no analytics.

## 9. Motion

Section 4 sets the library, the scroll model and the static-first rule. Component timings:

| Use | Duration | Easing |
| --- | --- | --- |
| Button press (`:active` scale 0.97) | 160 ms | `--ease-out` |
| Stage marker change (color and shape swap) | 120 ms | `--ease-out` |
| Live field commit (opacity 0 to 1, translateY 4 px to 0) | 200 ms | `--ease-out` |
| Story card swap (opacity, 28 px rise and 0.97 scale; outgoing card fades only) | 360 ms opacity, 600 ms transform | `--ease-out` |
| Story act headline (opacity, 0.5 em rise, 6 px blur to none; right half 80 ms later) | 500 to 800 ms | `--ease-out` |
| Story ground swap night to lavender | 700 ms | `--ease-in-out` |
| Entrance curtain (mark, wordmark, line, split) | about 2.9 s total, split 850 ms | `expo.out`, split `expo.inOut` |
| Hero entrance (words, highlight wipe, line draw, mark assembly) | 1.1 to 1.4 s, words staggered 45 ms | `expo.out`, `power3.out` |
| Hero attempt loop | 1.6 s per attempt, certified 2.6 s, 0.9 s rest, paused off screen | `power2.in`, linear |
| Rule figure plays, scroll reveals | 300 to 900 ms, staggered 80 to 220 ms | `--ease-out` |
| Story step changes, timelock dial | scrubbed by scroll | direct progress |
| Disclosure open and close, Human/Agent toggle | instant | n/a |

No `transition: all`. Never animate from `scale(0)`. Interruptible CSS transitions for anything that can retrigger.

**`prefers-reduced-motion: reduce`:** no GSAP setup at all, no transitions, no transforms; the story is the static ordered list; anchors jump; `:active` feedback is a background change. No information depends on motion.

## 10. Copy voice

**Voice rules.** Precise, calm, third person about the system, past tense for observed facts, present tense for rules. Exact numbers. Scope every claim to this vault and this network. No exclamation marks. No hype lexicon: unhackable, unbreakable, bulletproof, trustless, military-grade, revolutionary, seamless, AI-powered, "secure by design", "fully verified". Never claim: fully formal verification, prevention of Bybit-style incidents, protection of any contract other than the configured proxy, protection against bugs the declared rules do not cover. "We" appears only in the Limits section ("We do not claim").

**Example lines.**

1. Hero h1 (the one-liner, fixed): `Steal every admin key. You still cannot violate the upgrade constitution.`
2. Hero subtext (19 words): `One vault on Base Sepolia upgrades only through a NEAR gate that signs only what the committed rules certify.`
3. Scope chip: `Testnet demo. Not audited. One vault.`
4. Refusal: `Refused under R2 Solvency. The verifier reproduced a sequence in which address 0x4a7d31…91c0 withdraws 100% of the vault. No signature was requested. The proxy implementation is unchanged.` (Address and percentage are placeholders; they render only when present in the evidence, otherwise the generic sentence in 6.10 is used.)
5. AI limit: `The private AI reviewer can only refuse. It cannot approve.`
6. Success: `Executed on Base Sepolia, block 14,206,331. The proxy implementation changed from 0x91c0be…be22 to 0x4a7d31…09f1 through the gate, the MPC signature and the executor.` (All values are placeholders for format only, not data.)
7. Timelock refusal: `Refused (E53_TIMELOCK_NOT_ELAPSED). This change cannot be applied before 2026-11-09 14:02 UTC. No role can shorten the delay.` (Date is a format placeholder; the real `eta` renders from the gate.)
8. Review incomplete: `Review incomplete. The private AI review did not finish, so nothing was certified. It can be retried until 2026-11-03 14:32 UTC.`
9. Pending fields: `Pending. Not yet observed.` and for a path that ended earlier: `Not reached.`
10. Limits: `Rules protect what they declare. A flaw outside R1 to R5, or beyond their bounds, can pass. The opcode filter is a filter, not a proof.`
11. Network: `Cannot reach Base Sepolia. Showing the last observed state from 14:02:11 UTC. Nothing here is live until the connection returns.`
12. Local test: `Local test, not chain evidence. This receipt was produced on a local fork and has no explorer link.`

**Limits section content (from PRD, wording to stay close to it):**
- Guarantees, if the stated trust assumptions hold: G1 to G4 as written in the PRD "Risks and guarantees".
- Trust assumptions: MPC network honesty and availability (not post-quantum per the PRD citation); TEE vendor and attestation chain, with the PRD sentence that attestation proves which code ran on which input, not that the conclusion is correct; rules protect only what they declare; opcode and selector filters are filters; bounded checks are bounded; AI review is nondeterministic and its find rate is reported from rehearsal runs, not promised; liveness outages delay upgrades but never cause a signature without certification.
- Not claimed: full formal verification; prevention of Bybit-style incidents; protection of other contracts; protection against bugs the declared rules do not cover; "first ever"; absence of competitors.

## 11. Page-level data and trust notes for the builder

- The page signs only the demo `propose` (function-call key, ARCHITECTURE.md). A compromised page host cannot produce certificates or signatures (ARCHITECTURE.md risk list). This sentence appears in Limits.
- Hashes the page displays are re-derived by the page where feasible (ARCHITECTURE import boundary: `web` re-derives hashes independently); where it only relays a value from the receipts JSON the field carries `Recorded` or `As reported by the receipt`, not `Verified`.
- Colors never imply more certainty than the data: a stale or recorded value never carries `Observed` styling.

## 12. Founder and lead decisions

**D7 (founder): visual direction. RESOLVED 2026-11-07.** Follow the founder's brand (`asset/` logos) and reference video, elevated for CONSTITUTION; sections 2 to 4 record the result. Open licences only: Geist and Geist Mono (OFL 1.1, self-hosted); GSAP under its standard no-charge licence. No paid face, no spend.

**Lead answers to the builder questions (2026-10-26; contracts updated in ARCHITECTURE, SMART-CONTRACT and OPEN-DECISIONS D17).**
- **Q1. Sealing.** The page seals the fixture bundle itself with `libsodium-wrappers` 0.8.4 `crypto_box_seal` to the `transport_pubkey` from `GET /v1/identity` (npm metadata checked 2026-10-26). Interop with the Rust `crypto_box` 0.9.1 sealed box is proven by vector T-ENC-3 before use. `Sealing the source bundle` is a visible sub-stage.
- **Q2. Explorer.** Canonical: `https://sepolia.basescan.org` (docs.base.org, read 2026-10-26). Fallback: `https://base-sepolia.blockscout.com`. Both are configuration; links appear only for observed hashes.
- **Q3. Verifier status from the browser.** Yes. The verifier serves CORS for the configured page origin on `GET /v1/identity`, `GET /v1/runs/{id}`, `GET /v1/runs/{id}/evidence` and the rate-limited `POST /v1/runs`.
- **Q4. Receipts.** Two sources, labeled distinctly: `Observed` receipts assembled live in the browser from gate views, verifier run status and Base Sepolia reads; `Recorded` receipts from committed `receipts/*.json` (rehearsal and submission runs) bundled at build time.
- **Q5. Governance on the page.** One preset action, "Try to change the rules now": schedules a harmless preset `SetParams` change with the demo key and immediately calls `apply_change`, showing the real `E53_TIMELOCK_NOT_ELAPSED` refusal. The queue is otherwise read-only.
- **Q6. Writes allowed.** The demo key may call `propose`, `schedule_change` and `apply_change` only. The page may also broadcast an already signed raw transaction to Base Sepolia (permissionless) and re-post a run to the verifier. `retry_sign` and `bump_fee` are shown as copyable CLI commands, not buttons.
- **Q7. Footer links.** Repo link from `PUBLIC_REPO_URL`, hidden until the founder publishes the repository (B6). The X post link comes from `PUBLIC_X_POST_URL` and stays hidden until the founder publishes.

**Status of this document for D8 (hosting, undecided).** Nothing here depends on server features. Headers such as CSP and caching depend on the host; if the host cannot set headers, use a `meta` CSP [U].

## 13. Sources

- Founder brand files `asset/full-logo-white.png`, `asset/icon-logo-black.png`, `asset/logo-only-black.png` and reference video `asset/reference.mp4` (gitignored), read 2026-11-07.
- npm registry metadata read 2026-11-07: `gsap` 3.15.1 (standard no-charge licence, https://gsap.com/standard-license), `@fontsource-variable/geist` 5.3.0 and `@fontsource-variable/geist-mono` 5.3.0 (`OFL-1.1`).
- Project documents: `docs/PRD.md` (guarantees G1 to G4, limits, never-cut demo), `docs/technical/ARCHITECTURE.md` (verifier stages, receipt JSON, CLI), `docs/technical/SMART-CONTRACT.md` (states, panic codes), `rules/manifest.json`, `spikes/gate-min/base-sepolia-evidence.json`.
- Skills read for guidance only: impeccable (craft floor), emil-design-eng, design-taste-frontend, gsap-core, ponytail.
- The 2026-10-26 reference research (Awwwards, save.design and 20 reference sites, screenshots in `scratch/design-refs/`) informed the retired directions and stays in git history.
