# CONSTITUTION: design direction and interaction spec for the public page

Planning document. No application code, no installs. English only. Written 2026-10-26.

Labels: [V] opened this session (URL in Sources, date 2026-10-26), [I] design inference, [U] unverified.

## TL;DR and status

- Status: **visual direction is founder decision D7 and is OPEN.** This document gives three candidate directions and a recommendation [I]. Everything that is not a pure look (information architecture, semantics, state machine, microcopy, receipt anatomy, motion rules, accessibility) is **direction-neutral** and can be built now. Only token values, type families and the one signature moment change with D7.
- Candidates: **A. Constitutional document** (light, editorial serif, rules as articles, verdicts as seals), **B. Forensic ledger** (dark, monospace receipts), **C. Broadside** (monochrome typographic poster; replaces the earlier "Swiss minimal" because the research found a stronger, more ownable third axis, see section 3C).
- Recommendation [I]: **A, with B's ruled monospace discipline inside the receipts.** A document that contains ledgers. Reasons in section 4.
- Hard design rules that hold in every direction: pending items never show fake links or fake progress; timestamps and hashes appear only once observed; local test evidence never looks like testnet evidence; stale data never looks live; the AI is shown as veto-only; no claim beyond PRD guarantees G1 to G4.
- Net new product questions surfaced by this work (need lead answers, none blocks the spec): section 12, Q1 to Q7.
- References: 20+ concrete pages opened, 28 screenshots in `scratch/design-refs/` (gitignored). Index in section 2.

## 1. Design thesis

**What the page must make a judge feel in 10 seconds:** *authority, proof, calm under attack.*

- **Authority.** The page reads like an instrument of law, not a startup. Type is large, spacing is even, nothing blinks, nothing sells. The one-liner is stated, not shouted: "Steal every admin key. You still cannot violate the upgrade constitution."
- **Proof.** The first screen already contains observed facts, not claims: the current proxy implementation hash with the Base Sepolia block it was read at, the active rule-set hash, the timelock queue length. A judge who sees a hash with a block number believes the page is wired to a chain before reading a sentence.
- **Calm under attack.** When the drain proposal is refused, the page does not flash red or shake. It states the rule, the reproduced counterexample, "no signature was requested", and "implementation unchanged" as two equal hashes. Calm is the evidence: the system was not surprised.

**Why this serves the product.** CONSTITUTION's claim is scoped and falsifiable (PRD TL;DR): for one protected proxy, no key we control can run an uncertified implementation or shorten the 7-day timelock. A flashy page would read as marketing for a security product. A page that is precise, scoped, and makes every statement traceable to a receipt matches the Design criterion evidence in the PRD ("Public page with real states, receipts a non-expert can read") and gives Claude Code and Codex judges a DOM they can read without running JavaScript (section 5, progressive enhancement).

**10-second test (use at review):** within one 1280x800 viewport and with no scrolling, a first-time viewer can say (1) what it is, (2) that it is testnet and unaudited, (3) that live chain data is shown, (4) where to take the key. Fail any and the hero is wrong.

**Anti-goals:** AI-purple gradients, mascot illustration, dot-matrix art, "trusted by" logo walls (we have no customers, PRD), vanity volume stats (we have no metrics), intro gates, autoplay media, counters that imply progress.

## 2. Reference board

Every row was opened this session (2026-10-26). Screenshots are in `scratch/design-refs/` (gitignored). "Computed" values below were read from the live DOM with `getComputedStyle` in headless Chromium, so they are measurements, not impressions. Screenshot width 1280 unless named 390.

| # | Reference [V] | File | Borrow | Avoid |
| --- | --- | --- | --- | --- |
| R1 | Awwwards home, Site of the Day 2026-10-25 "Forge Automotive", score 7.24 (https://www.awwwards.com/) | `01-awwwards-home-1280.png` | Name-as-headline at 112 px Inter Tight (computed), credits strip under it, restraint of one big title. Used to select references from the SOTD list. | Nothing specific; it is a directory. |
| R2 | Awwwards Sites of the Day listing (https://www.awwwards.com/websites/sites_of_the_day/) | `03-awwwards-sotd-list-1280.png` | Source of current winners: Forge Automotive, ESR Bespoke, Emotion Agency, Cartier "Le Choeur des Pierres", FORMAT, Signal-A, Belgrade Arbor, OFFFForum. | n/a |
| R3 | save.design home and explore (https://save.design/, https://save.design/explore) | `02-savedesign-home-1280.png`, `04-savedesign-explore-1280.png` | Dark near-black `rgb(9,9,11)` with Geist and Geist Mono and a pixel face for headings (computed); filter by Industry including **cybersecurity** and finance; per-site tech stack metadata. | Their landscape hero render is decoration with no information. |
| R4 | save.design detail, railgun.org (https://save.design/website/railgun-org) | `20-savedesign-detail-railgun-1280.png` | Shows how a reference is documented: framework React, animation GSAP/Three.js/Lenis/Rive, design system Radix UI, font Onest. Template for the "borrow" notes here. | n/a |
| R5 | Forge Automotive, SOTD 2026-10-25 (https://forgeautomotive.co.uk/) | `05-forge-automotive-1280.png` | Full-black first screen with one statement in widely tracked caps and a serif display face (family named "editorial", 76.8 to 80 px, computed). Proof that big quiet type reads premium. | "ENTER" gate and cookie line before content: a judge has 10 seconds. Never gate the page. |
| R6 | ESR Bespoke, on the Awwwards recent Sites of the Day list (https://www.esrbespoke.au/) | `06-esr-bespoke-1280.png` | Multi-zone header (name, descriptor, nav) with the headline left and a small caps paragraph far right. 62 px grotesk headline (computed). | Autoplay video with "UNMUTE". |
| R7 | Heron AI, Awwwards entry by Bearplus (https://heronaiapp.com/) | `07-heron-ai-1280.png` | Hairline frame lines across a pale field; spec list in mono as `TYPE : ... FOUNDED : ... FOCUS : ...` and an index `001 INIT` (page text). Direct pattern for the receipt header `dl`. Fonts BT Grotesk and Geist Mono (computed). | Full-screen intro illustration that delays content. |
| R8 | MilleDollars, Awwwards entry (https://milledollars.fr/) | `08-milledollars-1280.png` | Index rows: ordinal, title, party, year (`01 | THE DAZZLING DIVISION | RICHARD MILLE | 2026`, page text). Pattern for the receipts list. ABC Diatype and Migra (computed). | Media-heavy rounded image stack; nothing to learn for data. |
| R9 | Signal-A Studio (https://signal-a.studio/) | `09-signal-a-1280.png` | Live local clock and city in the header (`03:09 CET`), hairline-underlined nav cells, GT America Mono for UI (computed). Clock pattern informs the timelock clock. | Letter-spaced justified display lines, and per-letter DOM splitting: the extracted page text came back as single letters, which is hostile to screen readers and to AI judges reading the DOM. |
| R10 | RAILGUN (https://railgun.org/) | `10-railgun-1280.png`, `24-railgun-mobile-390.png` | Hairline-boxed 2x2 stat cells with large numerals and a label below; at 390 px the same 2x2 grid holds. Pattern for the ledger strip. Onest at 80 px (computed). | Dot-matrix mountain art, volume stats (we have no metrics), purple CTA. |
| R11 | Cyera (https://cyera.com/) | `11-cyera-1280.png` | Two-weight headline: bold first line, light second line. Haffer Uprights (computed). | Purple gradient and mascot illustration: the AI-security cliche. |
| R12 | Cotool (https://cotool.ai/) | `12-cotool-1280.png` | Small mono eyebrow above a 64 px grotesk headline with one emphasized word set in a serif inside a highlight box (Suisse Intl, Geist Mono, a serif; computed). | Dot-matrix wave and "Loved by" logos. Note: the skill guidance warns that mixed-family emphasis reads amateur, so only copy the box, not the family swap. |
| R13 | HackFirst (https://hackfirst.io/) | `13-hackfirst-1280.png`, `26-hackfirst-scroll-1280.png` | Full-width wordmark; hairline-ruled stat rows with an index tag `DOC. [00-02]`, a 60 px numeral and a mono caption. Closest existing pattern to a ledger row. Neue Haas Display and Feature Mono (computed). | The loader shows a `0%` counter: this is exactly the fake-progress pattern we forbid. Noise video. |
| R14 | Dicta: Legal Poetry (https://dicta.icaad.ngo/) | `14-dicta-1280.png`, `27-dicta-after-begin-1280.png` (`28-dicta-scroll-1280.png` is a blank frame, unused) | Legal-record subject matter handled with plain big type. A single phrase sits in a solid box whose fill inverts between frames (black field with white box, white field with black box). Reusable as the "verdict word" treatment. Neurial Grotesk 90 px (computed). | Sponsor logo wall and the multicolor stripe. |
| R15 | Burkhalter Rechtsanwaelte, law firm, Awwwards nominee 2026-03-10 (https://drpb.ch/, https://drpb.ch/en) | `15-drpb-1280.png`, `22-drpb-scroll-1280.png` | Closest "legal authority" web reference: light display serif (GT Super Display, 90 px) over a Unica77 sans, body text in a single deep ink blue `rgb(1,30,147)` on white (computed). Ink-blue-on-white is the model for direction A. | Stock photo of lawyers, blurred glass header. |
| R16 | Orchid Security, Awwwards nominee 2026-09-19 (https://www.orchid.security/) | `16-orchid-1280.png` | Shows what the category default looks like: centered headline, purple glow, mock dashboard. | All of it. This is the pattern to differ from; a div-built fake dashboard would also contradict "no fake progress". |
| R17 | FORMAT by Obys, SOTD 2026-10-19, score 7.59 (https://format.obys.agency/) | `17-format-obys-1280.png` | Clause numbering `1`, `1.1 Usage`, `1.2 Geometry` on white sheets over grey `#222` (page text). Direct model for numbering rules and article clauses. | 10.7 px body text (computed): unreadable. |
| R18 | OFFFForum, SOTD 2026-10-16, score 7.33 (https://www.offforum.com/) | `18-offforum-1280.png`, `23-offforum-scroll-1280.png`, `25-offforum-mobile-390.png` | A hairline-boxed two-column key/value table in the hero that survives unchanged at 390 px. Pattern for the receipt header and the ledger strip. Forvm serif caps with Baikal sans (computed). | Cookie modal covering the first screen. |
| R19 | The Explora Journal, Awwwards honorable mention 2026-09-12 (https://explorajourneys.com/int/en/explora-journal) | `19-explora-journal-1280.png` | Calm light serif title with an italic descriptor line under it; visible accessibility affordance ("Press Alt+1 for screen-reader mode", page text). Pattern: italic subtitle, visible accessibility statement. | Warm beige ground `rgb(244,242,239)` (computed): the skill guidance flags warm cream as an AI default, so direction A uses a cool neutral instead. |
| R20 | Constitute, US constitution text (https://www.constituteproject.org/constitution/United_States_of_America_1992) | `21-constitute-us-1280.png` | The real reading pattern for a constitution: topic labels in a left marginal column, permalink icon, "Preamble / Article I / Section 1" headings, text column about 700 px. Lato at 28 px for h1 (computed). Pattern for the rules section: marginalia plus anchors. | The pale grey marginal labels look low contrast [I]; keep marginalia at 4.5:1 or better. |
| R21 | Motion `animate()` docs (https://motion.dev/docs/animate) | n/a | Page text states: "It comes in two sizes, mini (3.2kb) and hybrid (22kb)." Basis for the motion library proposal in section 9. | n/a |

Directory pages also opened for selection (no screenshots): Awwwards typography, finance, editorial, legal, law, blockchain, security, magazine, fintech and banking listings, and the Awwwards entry pages for Forge Automotive, ESR Bespoke, Heron AI, MilleDollars, Signal-A Studio, Dicta, Swiss Precision in Law, Orchid Security, The Explora Journal, History of Blockchain, Cartier "Le Choeur des Pierres", FORMAT, Belgrade Arbor and OFFFForum (URLs in Sources).

**Patterns that survive from the research into every direction:**

1. Hairline rules and boxed key/value tables as the dominant structure (R7, R10, R13, R18). It is also the honest way to show data.
2. A live clock or timestamp as a quiet header element (R9, R13). Liveness without spinners.
3. Numbered clauses (R17, R20). The constitution is literally clauses.
4. One very large typographic gesture per viewport and everything else small (R5, R11, R13).
5. A boxed word that inverts (R14) as the verdict word.

## 3. Three candidate directions

All three use the same token names (section 3.0), so swapping direction is a token and type change, not a rebuild.

### 3.0 Direction-neutral token contract

Semantic names only; each direction supplies values. Component CSS never uses raw colors.

```css
:root {
  /* surfaces and text */
  --bg: ;            /* page ground */
  --surface: ;       /* raised panel, receipt body */
  --text: ;          /* primary text */
  --text-muted: ;    /* secondary text, still >= 4.5:1 */
  --line: ;          /* decorative hairline, may be < 3:1, never the only cue */
  --line-strong: ;   /* control borders and meaningful dividers, >= 3:1 */
  --accent: ;        /* links, focus ring, rule numbers. The single accent. */
  /* semantic verdict colors, used ONLY for verdict state, never for decoration */
  --pass: ;
  --refuse: ;
  --pending: ;
  /* typography */
  --font-display: ;
  --font-text: ;
  --font-ui: ;
  --font-data: ;     /* hashes, addresses, receipts */
  /* motion, identical across directions */
  --ease-out: cubic-bezier(0.23, 1, 0.32, 1);
  --ease-in-out: cubic-bezier(0.77, 0, 0.175, 1);
  --dur-fast: 120ms;
  --dur-base: 200ms;
  --dur-seal: 280ms;
}
```

Every state is conveyed by a text label plus a marker shape, never by color alone (section 8).

---

### 3A. Constitutional document (light, editorial serif)

**Concept.** The page is a published instrument. Rules are articles with clause numbers. Receipts are exhibits attached to the instrument, set in ruled monospace. Verdicts are impressed as seals. Ground is a cool neutral, ink is near-black, and the single accent is an ink blue like a notary stamp.

**References [V]:** R15 (ink blue on white, display serif), R20 (marginalia, permalinks, clause headings), R17 (clause numbering), R19 (italic descriptor), R7 and R18 (boxed key/value for receipt headers), R14 (boxed verdict word).

**Type pairing (proposal).**

| Role | Family | Licence status |
| --- | --- | --- |
| Display and text | Newsreader (variable) | [V] SIL OFL 1.1, shipped as a variable font, per its repository README. Chosen because it was designed "for continuous on-screen reading", fits long rule text, and is not a banned default display serif. |
| UI labels, buttons, table heads | Geist Sans | [V] SIL OFL 1.1 per the Geist repository. |
| Hashes, addresses, receipts, code | Geist Mono | [V] same repository and licence. |
| Paid upgrade path (not recommended, needs founder approval) | GT Super Display with a grotesk, the pairing seen on R15 | [U] commercial licence, price and web licence terms not checked. No spending is authorized. |

Scale [I], 1280 / 390: h1 64 / 34 px, line-height 1.05, weight 400. h2 40 / 28. h3 24 / 20. Body 18 / 17 px at 1.6, measure 62 to 68 characters. Data (mono) 14 / 13 px at 1.5, `font-variant-numeric: tabular-nums slashed-zero`. Section marginal label: sentence case 13 px, not uppercase-tracked (avoids the eyebrow-on-everything tell).

**Color tokens.** Contrast ratios were computed this session with the WCAG relative-luminance formula.

```css
:root {                      /* direction A */
  --bg: #F5F6F8;
  --surface: #FFFFFF;
  --text: #10131A;           /* on bg 17.18, on surface 18.58 */
  --text-muted: #4A5163;     /* on bg 7.33, on surface 7.93 */
  --line: #C9CED8;           /* on bg 1.46: decorative only */
  --line-strong: #7A8296;    /* on bg 3.56: control borders pass 3:1 */
  --accent: #1D2FA6;         /* on bg 9.66, on surface 10.45; white on accent 9.66 */
  --pass: #0F6B4A;           /* on bg 6.03; white on pass 6.52 */
  --refuse: #B3261E;         /* on bg 6.04; white on refuse 6.54 */
  --pending: #7A5200;        /* on bg 6.40 */
}
```

Notes: the cool grey ground avoids the warm cream and brass family that the skill guidance flags as an AI default. Pass and refuse appear only on verdict markers, seals and verdict words. QR codes always render dark on a light quiet zone (`#10131A` on `#FFFFFF`, quiet zone at least 4 modules) [I].

**Layout grid.**

| Width | Grid | Detail |
| --- | --- | --- |
| 390 | 4 columns, 20 px margins, 16 px gutters | One column. Marginal labels (the `§` number and topic) sit above their block. Receipt `dl`: term above value. Rules table keeps real table semantics but shows ID and Rule only, with the check detail in a per-row `details`. |
| 1280 | 12 columns, 1200 px max container, 40 px margins, 24 px gutters | Marginalia in columns 1 to 3, text in 4 to 10 (R20 pattern). Hero: headline columns 1 to 8, QR "take it" card columns 9 to 12. Receipt: term in 1 to 3, value in 4 to 12. Ledger strip is a 4-cell boxed row (R10, R18). |

**Signature moment: the Seal.** When a receipt reaches a terminal verdict, a circular seal is impressed on the receipt header: circle text reads `CERTIFIED` or `REFUSED`, the rule id (`R2`), and the short rule-set hash. It appears only after the verdict signature is observed. Motion: opacity 0 to 1 and scale 1.06 to 1 over `--dur-seal` with `--ease-out`, once. Under reduced motion it simply appears. The seal is an SVG with an `aria-label` that repeats the text verdict; all seal information also exists as plain text next to it. Local-test receipts get a different, hatched, unsealed outline marked `LOCAL TEST` so they can never be mistaken for chain evidence. The Seal is the memorable, screenshot-able artifact for the X post.

**Risks.** (1) Reads as a law-firm brochure if the hero type scale is timid: mitigate with the 64 px display line, the live ledger strip and the seal. (2) Serif at small size in receipts: keep receipts in Geist Mono. (3) Light page under studio lighting in the demo video can glare: keep `--bg` off pure white (it is `#F5F6F8`). (4) Seal SVG text on a circle can be fussy: build it from a fixed-length string, test with the longest rule id.

---

### 3B. Forensic ledger (dark, monospace receipts)

**Concept.** A chain-of-custody ledger. Near-black ground, tabular monospace everywhere data appears, ruled rows with a document index tag, one amber accent. The page feels like an evidence terminal with the volume turned down.

**References [V]:** R13 (HackFirst: hairline rows, `DOC. [00-02]` tag, big numerals, mono caption), R3 and R4 (save.design: near-black with Geist and Geist Mono), R10 (boxed stat cells), R7 (mono key/value spec list), R18 (boxed table).

**Type pairing (proposal).**

| Role | Family | Licence status |
| --- | --- | --- |
| Display and UI | Geist Sans | [V] OFL 1.1 (Geist repository). |
| Data, receipts, labels | Geist Mono, alternative JetBrains Mono | [V] OFL 1.1 for both (Geist and JetBrains Mono repositories). |
| Paid upgrade path | Neue Haas Display with Feature Mono (the pairing computed on R13) | [U] commercial. Not authorized. |

Scale [I], 1280 / 390: h1 56 / 32 px Geist Sans 500, h2 32 / 24, body 17 / 16, data 14 / 13 mono. Ledger numerals up to 96 px at 1280 for the timelock and implementation counts.

```css
:root {                      /* direction B */
  --bg: #0A0C0F;
  --surface: #12151A;
  --text: #E8EEF2;           /* on bg 16.73, on surface 15.63 */
  --text-muted: #9AA6B2;     /* on bg 7.90, on surface 7.38 */
  --line: #2A313A;           /* on bg 1.49: decorative only */
  --line-strong: #55616E;    /* on bg 3.10 */
  --accent: #E8B04B;         /* on bg 10.02; dark text on accent 10.02 */
  --pass: #5FD39A;           /* on bg 10.51 */
  --refuse: #FF7A6B;         /* on bg 7.69 */
  --pending: #E8B04B;        /* on surface 9.36: same hue as accent by design */
}
```

Notes: single accent is amber (evidence tape), not neon green or purple. Pending and accent share a hue, which is acceptable because pending is a state label with text, never a standalone cue.

**Layout grid.**

| Width | Grid | Detail |
| --- | --- | --- |
| 390 | 4 columns, 16 px margins | Rows are stacked: index tag (`[00-02]`), label, value, each on its own line. Ledger strip is a 2x2 boxed grid (R10 at 390 holds this). |
| 1280 | 12 columns, 1200 px, 40 px margins | Rows are `index (cols 1-2) | label (3-5) | value (6-12)` as in R13. Hairline between every row. Full-width wordmark is optional, not required. |

**Signature moment: Before equals after.** On the refusal receipt, the proxy implementation line shows the `before` and `after` hashes set in the same monospace columns, character for character, with an `=` between them and the text `unchanged, observed at block {n}`. On the executed receipt the same two lines appear with the differing characters highlighted. The point is visual: a judge sees two identical strings and understands "nothing happened". Rows commit one by one as they are observed (fade and 4 px rise, 200 ms), never typed or scrambled. This is the most data-honest signature of the three.

**Risks.** (1) Dark neon-adjacent look is the crypto cliche and the Forge, HackFirst and ESR pages already cover dark; the page needs the amber discipline and generous spacing to feel like authority rather than a hacker theme. (2) Mono at long prose lengths is tiring: restrict mono to data, not explanation. (3) Dark mode screenshots for X compress poorly on some feeds [U]. (4) Amber on black plus red refusal needs the text labels, not color, to separate states.

---

### 3C. Broadside (monochrome typographic poster)

*Replaces "Swiss minimal".* Plain Swiss minimal in a grid of whitespace is safe but anonymous and does not give the founder the "amaze me" moment. The research (R6, R9, R13, R17, R18) shows a stronger version: a poster-scale grotesk wordmark, hairline boxed tables, clause numbers and one signal color on pure black and white.

**Concept.** The page is a printed broadside nailed to a wall. A full-width `CONSTITUTION` wordmark, black ink on white, and one signal red-orange used only for the thing that is refused. Everything else is boxed hairlines and small type.

**References [V]:** R13 (full-width wordmark, ruled rows), R17 (clause numbers, FORMAT), R18 (boxed table at 390), R9 (live clock, hairline nav), R6 (grotesk headline with a small caps paragraph far right), R5 (calm statement).

**Type pairing (proposal).** Geist Sans for wordmark, headings and body, Geist Mono for data. [V] both OFL 1.1. Paid upgrade path: GT America or Neue Haas Display [U] commercial, not authorized. Wordmark sized with viewport units so the single word fills the container: roughly 146 px at 1280 and 43 px at 390 [I, measure in build].

```css
:root {                      /* direction C */
  --bg: #FFFFFF;
  --surface: #FFFFFF;
  --text: #0A0A0A;           /* 19.80 on bg */
  --text-muted: #4D4D4D;     /* 8.45 on bg */
  --line: #D9D9D9;
  --line-strong: #767676;    /* 4.54 on bg */
  --accent: #0A0A0A;         /* the accent IS ink; signal is semantic only */
  --pass: #0B6B3A;           /* 6.61 on bg */
  --refuse: #C62D10;         /* 5.55 on bg; white on refuse 5.55 */
  --pending: #4D4D4D;
}
```

Note: `#E5391A` measured 4.26 on white, which fails 4.5:1 for text, so the signal is darkened to `#C62D10`.

**Layout grid.**

| Width | Grid | Detail |
| --- | --- | --- |
| 390 | 4 columns, 16 px margins | Wordmark full width, headline below, ledger strip 2x2, everything else single column with hairline rules. |
| 1280 | 12 columns, 1200 px | Wordmark spans all 12, one-liner in columns 1 to 7, small paragraph and QR in 9 to 12. Rules as a numbered ruled list (`1.1`, `1.2`) in columns 1 to 12 with big numerals. |

**Signature moment: The clock.** A very large numeral block for the timelock: `6 d 23 h 41 m` from the real `eta_ns` of a real queued change, updated in place each minute, no animation. Empty state is `No change queued` at the same size. The page says the same thing the product says: even the governor must wait, in 96 px type. Appears only from observed `eta`; never a decorative countdown.

**Risks.** (1) Highest agency-site risk: poster type can bury the proof. The ledger strip must stay above the fold. (2) The wordmark must be real text (not letter-split spans, see R9 avoid) so screen readers and AI judges read it. (3) Pure white and black maximize contrast but feel colder and less "constitutional" than A. (4) Easy to over-style; strict discipline needed on one signal color.

---

## 4. Recommendation [I]

**A. Constitutional document, with B's ruled monospace inside receipts.** D7 stays open; this is advice.

1. **Semantics match the metaphor.** The product's nouns are rules, articles, a rule-set hash, a timelock queue and receipts. A document maps them to the HTML that already exists for them: `table` for rules, `ol` for the proposal timeline and the queue, `dl` for receipt fields, `article` for receipts. The founder's "as semantic as possible" is satisfied literally and visually.
2. **Judges are three kinds of reader.** A human skims for 10 seconds, Claude Code and Codex read the DOM. A document with real text, clause numbers and a full static HTML baseline serves all three (section 5).
3. **Differentiation inside the reference set.** In the opened set, security and crypto pages skew dark, purple or dot-matrix (R10, R11, R12, R13, R16). The closest legal-authority reference (R15) is light with a display serif and ink blue. A restrained light instrument is rarer in this category [I].
4. **Calm is easier to show on light type.** A refused proposal in `--refuse` on a quiet ground reads as a decision, not an alarm. On dark, red reads as alarm.
5. **Screenshot and print friendly.** Receipts as clean document pages survive in an X post and in `@media print`.
6. **Cost.** Newsreader, Geist Sans and Geist Mono are all OFL [V], no spend.
7. **Why not B or C as the base.** B is the best pure proof aesthetic but the most generic category look; C is the most striking but the highest risk of burying proof under type. Both remain valid if the founder wants to be louder (C) or more technical (B).

**How to avoid the "boring brochure" risk:** the 64 px display line, the live ledger strip in the first viewport, the seal, and the "before equals after" line from B on the refusal receipt.

## 5. Information architecture (single page)

One page, one `h1`, static HTML as the baseline, live data as enhancement.

**Progressive enhancement rule [I].** The build embeds a snapshot: rules, limits, how-it-works text, and the latest recorded receipts rendered into HTML, each labeled `Recorded {date}`. Islands (Astro) upgrade only four things with live reads: ledger strip, Attack and Upgrade surfaces, Receipts, Audit. A client that never runs JavaScript still sees every claim and the recorded receipts. Live values replace the snapshot only after observation, and the label changes from `Recorded` to `Observed`.

Page landmarks: `header` (banner) with skip link and `nav aria-label="Sections"`; `main`; `footer` (contentinfo). Every section is `section aria-labelledby="{id}-title"` with an `h2`. Single status region per interactive surface.

| # | Section (id) | Elements and semantics | Contents | Live data |
| --- | --- | --- | --- | --- |
| 0 | Header | `header`, `nav`, skip link as first focusable | Wordmark text `CONSTITUTION`, anchor links, a UTC clock (`time`) | Clock is client-only; static page shows no clock |
| 1 | Hero (`hero`) | `section` with the page `h1`; `figure` for the QR | One-liner h1. Subtext of at most 20 words. One primary link `See the attack` to `#attack`. Scope chip `Testnet demo. Not audited.` QR "take it" card (section 5.1). | none |
| 1b | Ledger strip | `dl` inside a boxed row, `aria-labelledby` | Proxy implementation (hash, `Base Sepolia block {n}`), active rule-set hash, verifier allowlist count, timelock queue count. Each cell: `Observed {time} UTC` or `Recorded {date}` or `Not yet observed`. | Base RPC via viem, NEAR view |
| 2 | The constitution (`constitution`) | `section`; rules in `table` with `caption`, `th scope="col"` and `th scope="row"` for rule ids; allowlist in `table`; queue in `ol` with `time` | Active rules R1 to R5 (id, name, what it checks, stated limit). Rule-set hash with copy. Verifier allowlist (pubkey short, release hash short, attestation link when it exists). Timelock queue: pending changes with `eta`, kind, status; fixed text "Delay: 7 days. No role can shorten it." and "Guardian can only cancel or reduce authority". | NEAR views |
| 3 | Attack (`attack`) | `section`, `form` with one submit button, `output` for status, `ol` for stages | Fixture description for `VaultV2Drain`. Submit with the demo key. Stage ladder, verdict, proxy line. | NEAR, verifier status, Base RPC |
| 4 | Honest upgrade (`upgrade`) | same as 3 | `VaultV2Honest`. Same state machine. | same |
| 5 | Receipts (`receipts`) | `section` with `ol` of `article`s, each `article aria-labelledby`, `dl` for fields, `table` for rule results | One receipt per proposal and per governance refusal. Section 7. | receipts JSON, RPCs |
| 6 | Audit panel (`audit`) | `section`, `button` Run audit, `output`, `ul` of checks | F7 checks, per-check status (section 6.9) | NEAR RPC (access keys), Base RPC |
| 7 | How it works (`how`) | `ol` of 6 steps; optional decorative SVG with `aria-hidden` | Proposal, attested verifier, private red-team AI veto, gate, Chain Signature, executor on Base Sepolia. Text first. | none |
| 8 | Limits and honesty (`limits`) | `section`, `dl` or `ul` | PRD guarantees G1 to G4, trust assumptions, "Not claimed" list (section 10) | none |
| 9 | Footer | `footer` | Repo link, X link, build id and time, licence note, "Testnet. Not audited." | none |

### 5.1 QR "take it" card

- `figure` containing the QR as inline SVG with `role="img"` and a text alternative: `QR code. Encodes a disposable, testnet-only NEAR key that holds the demo governor role. Not a production key.` The key itself is not put in `alt` (too long).
- `figcaption` states: `This key is public on purpose. It can only act on the demo. Never reuse it.`
- Text alternative for people who cannot scan: `button` "Copy key" and a `details` "Show key as text" that reveals the key in `code`. A keyboard and screen-reader user has full parity.
- Size: at least 280 px square at 1280, 220 px at 390 [I], dark modules on light quiet zone in every direction.
- Never shown: any production key, any full-access key of the gate, the MPC-derived EVM key or any founder wallet (PRD never-cut demo, step 1).
- The key never persists: no localStorage, no cookies.

### 5.2 Section order rationale

Hero (what and where to take the key) then Constitution (what is committed) then Attack then Upgrade (the two proofs) then Receipts (evidence) then Audit (the claim's preconditions) then How it works then Limits. The order mirrors the demo: state the law, break it, obey it, show the paper.

### 5.3 Navigation

Single line at 1280: `Constitution, Attack, Upgrade, Receipts, Audit, How it works, Limits` (seven items, no secondary nav). At 390 a `button aria-expanded` disclosure opens the same list. Header is not sticky at 390; at 1280 it may be sticky with `scroll-padding-top` equal to header height so focused elements are never hidden.

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
- **Visual:** ladder up to the verdict `Done`, `--pass` verdict marker and word `Certified`. Direction A: the seal is not impressed yet; it waits for execution (see 6.9) and a refusal seal is impressed at the verdict (see 6.10). The receipt article appears with `Pending` in the signing and EVM fields.
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
- **Visual:** `--pass` verdict marker and word `Executed`. Direction A: the **seal is impressed now** (`CERTIFIED`, rule-set short hash) because the full chain of evidence is observed. Direction B: `before` and `after` lines with differing characters highlighted. Explorer links appear (7.4).
- **Copy:** title `Executed on Base Sepolia`. Body `Block {n}. The proxy implementation changed from {before short} to {after short}. This upgrade passed through the gate, was signed by the MPC network, and executed through the constitution executor.`
- **Live:** polite.
- **Recovery:** link `View the receipt`, `Run the attack`.

### 6.10 Rejected

- **Trigger:** verdict `Reject {rule_id, reason_code}` or gate `Rejected`.
- **Visual:** verdict word `Refused` in `--refuse` with the rule id (`R2`). Direction A: seal impressed with `REFUSED` and the rule id. Ladder rows after the verdict show `Not reached`. EVM rows read `Not requested`, never `Pending`. Proxy line: `before` and `after` equal, `unchanged, observed at block {n}`, rendered only after two observed reads agree.
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
- **Visual:** a boxed banner at the top of the affected surface (and the ledger strip), text `Not live`. Last observed values stay visible but each gets `Last observed {time} UTC` and a hatched or dashed treatment; live styling (verdict colors, seal) is removed from stale data.
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
  - `Local test, not chain evidence`: dashed border and hatch fill, no explorer links, no seal. Maps to receipt `labels.network = "local"`.
- Verdict word and marker, plus the Seal (direction A) or before and after (direction B).
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

Targets: WCAG 2.2 AA minimum, AAA contrast for body text where the palette allows (all three directions already measure above 7:1 for body text, section 3).

**Keyboard and focus**
- [ ] Tab order follows DOM order: skip link, header nav, hero link, QR copy button, QR text disclosure, constitution disclosures and copy buttons, Attack submit, Upgrade submit, receipts (each hash expand and copy), audit run, footer links.
- [ ] Focus ring: 2 px solid `--accent`, 2 px offset, never removed. Measured against ground: A 9.66:1, B 10.02:1, C 19.80:1.
- [ ] No sticky element obscures focus (2.4.11): `scroll-padding-top` on `html`, header not sticky at 390.
- [ ] Every control reachable and operable with keyboard only; no hover-only information anywhere (tooltips are replaced by visible text or Expand buttons).
- [ ] Disclosures use native `details` and `button aria-expanded`.

**Semantics**
- [ ] One `h1`; headings strictly nested; each section `aria-labelledby`.
- [ ] `table` with `caption`, `th scope`; `ol` for timelines and queues; `dl` for receipt fields; `article` for receipts; `time datetime` for every timestamp.
- [ ] One `output` (implicit status role) per interactive surface, plus explicit `aria-live` where needed. Live regions exist in the DOM before content changes. Text in them is full sentences. Countdowns are never in a live region.
- [ ] `lang="en"`; descriptive `title`; every link text states its destination.
- [ ] QR has a text alternative and a text-key fallback (section 5.1).
- [ ] No letter-by-letter DOM splitting of headings (R9 avoid).

**Color and contrast**
- [ ] Text at least 4.5:1, large text at least 3:1, UI component boundaries and focus indicators at least 3:1.
- [ ] State is never color alone: marker shape plus text label on every state (section 6.1).
- [ ] Respect `prefers-contrast: more` (raise `--line` to `--line-strong`) and `forced-colors: active` (seal, markers and borders keep meaning through borders and text) [I].
- [ ] Stale data loses verdict colors (section 6.15).

**Targets and zoom**
- [ ] Interactive targets at least 44x44 CSS px [I, WCAG 2.5.8 requires 24].
- [ ] Reflow with no horizontal scroll at 320 CSS px and at 400 percent zoom, except the `pre`-like receipt JSON, which scrolls inside a focusable region with a label.
- [ ] Text resizes to 200 percent without loss.

**Layouts at the two required widths** (visual check at both, per F10 acceptance)

| Section | 390 px | 1280 px |
| --- | --- | --- |
| Hero | One column: h1 (3 to 4 lines at about 34 px), subtext, link, scope chip, QR card, then ledger strip 2x2 | h1 in 2 lines at 64 px in columns 1 to 8, QR card columns 9 to 12, ledger strip 4 cells in one row inside the first viewport |
| Constitution | Marginal label above; rules table with ID and Rule, detail in `details`; queue as stacked rows | Marginalia in columns 1 to 3, table columns 4 to 12, queue as a ruled table |
| Attack and Upgrade | Panel full width; stage ladder vertical; button full width | Two panels side by side only if both fit without cramped ladders, otherwise stacked; ladder left, receipt preview right |
| Receipts | `dl` term above value; hashes truncated to the 390 rule | `dl` as term and value columns; hashes at the 1280 rule |
| Audit | Checks stacked | Checks as ruled rows with the observed value right-aligned |
| Footer | Stacked links | One row |

**Performance and robustness [I]**
- [ ] Static HTML contains all non-live content and recorded receipts; page is understandable with JavaScript disabled.
- [ ] Fonts self-hosted, `font-display: swap`, subset to Latin; preload display face only.
- [ ] Polling pauses when the tab is hidden; cadence while a run is active 2 s, idle 15 s; backoff on errors [public RPC quotas are not published, U].
- [ ] Page performs no writes except the demo `propose` and optional gate calls; no cookies, no analytics.

## 9. Motion

**Principles [I, informed by the Emil Kowalski skill guidance].** Motion explains a state change or confirms a press. It is never ambient. Frequent actions get no animation. Nothing moves that the user did not cause or that was not caused by a real observed event. No looping animation. All motion is an enhancement inside `@media (prefers-reduced-motion: no-preference)`.

**Durations and easings (proposal).**

| Use | Duration | Easing |
| --- | --- | --- |
| Button press feedback (`:active` scale 0.98) | 120 ms | `--ease-out` |
| Stage marker change (color and shape swap) | 120 ms | `--ease-out` |
| New receipt row or field commit on live update (opacity 0 to 1, translateY 4 px to 0) | 200 ms | `--ease-out` |
| Disclosure open and close | 0 ms (instant) | n/a |
| Seal impression (direction A): opacity 0 to 1, scale 1.06 to 1 | 280 ms | `--ease-out` |
| Hero entrance (optional, once on load): headline, subtext, link, 40 ms stagger, total under 400 ms | 200 ms each | `--ease-out` |

No `transition: all`. Never animate from `scale(0)`. Interruptible CSS transitions, not keyframes, for anything that can retrigger.

**What animates:** stage markers when a stage is observed; receipt rows arriving by live update (not on initial load); the seal once; button press; the hero on first load.

**What never animates:** hashes and addresses (no scramble, decode, typing or count effects); numbers (no count-up; the timelock countdown updates in place); status text (instant swap); progress (there is none to animate); the QR; the proxy before and after lines; scroll position (no smooth-scroll hijack, no parallax, no scroll-triggered reveals, so a judge scrolling fast or a screenshot tool never sees empty space); backgrounds; cursor effects; marquees; autoplay media.

**Library proposal [I].** CSS only: transitions, `@starting-style` and `prefers-reduced-motion` media query cover everything above [browser support of `@starting-style` and scroll-driven animation not checked, U]. If an engineer wants imperative sequencing for the seal or the receipt commit, the `motion` package's `animate()` has a mini build of 3.2 kb [V, motion.dev/docs/animate]. GSAP is not needed. Neither is an approved dependency; the lead and founder approve.

**`prefers-reduced-motion: reduce` behavior (full).**
- All transitions and animations are off: no fade, no translate, no scale, no stagger.
- Stage markers, receipt rows and the seal appear in their final state instantly.
- `:active` feedback becomes a background color change with no transform.
- `scroll-behavior: auto`; anchors jump.
- The hero is static.
- No information depends on motion: every state change is also announced in text and in the live region.

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

**D7 (founder): visual direction. OPEN.** Options A, B, C above; recommendation A with B's receipts. Decision needed before frontend build starts; not blocking spec work.

**Font licensing to confirm (founder).**
- Recommended set is Newsreader, Geist Sans, Geist Mono, optionally JetBrains Mono. All are SIL OFL 1.1 per their repositories [V]. Confirm that open fonts are acceptable.
- Any commercial face (GT Super Display, Neue Haas Display, GT America, Feature Mono, Suisse) is [U] on price and web licence and requires founder approval and spend, which is not authorized.
- Self-hosting plan (Astro's own font handling versus manual `@font-face`) is [U] for Astro 7.4.2; engineering decides.

**Lead answers to the builder questions (2026-10-26; contracts updated in ARCHITECTURE, SMART-CONTRACT and OPEN-DECISIONS D17).**
- **Q1. Sealing.** The page seals the fixture bundle itself with `libsodium-wrappers` 0.8.4 `crypto_box_seal` to the `transport_pubkey` from `GET /v1/identity` (npm metadata checked 2026-10-26). Interop with the Rust `crypto_box` 0.9.1 sealed box is proven by vector T-ENC-3 before use. `Sealing the source bundle` is a visible sub-stage.
- **Q2. Explorer.** Canonical: `https://sepolia.basescan.org` (docs.base.org, read 2026-10-26). Fallback: `https://base-sepolia.blockscout.com`. Both are configuration; links appear only for observed hashes.
- **Q3. Verifier status from the browser.** Yes. The verifier serves CORS for the configured page origin on `GET /v1/identity`, `GET /v1/runs/{id}`, `GET /v1/runs/{id}/evidence` and the rate-limited `POST /v1/runs`.
- **Q4. Receipts.** Two sources, labeled distinctly: `Observed` receipts assembled live in the browser from gate views, verifier run status and Base Sepolia reads; `Recorded` receipts from committed `receipts/*.json` (rehearsal and submission runs) bundled at build time.
- **Q5. Governance on the page.** One preset action, "Try to change the rules now": schedules a harmless preset `SetParams` change with the demo key and immediately calls `apply_change`, showing the real `E53_TIMELOCK_NOT_ELAPSED` refusal. The queue is otherwise read-only.
- **Q6. Writes allowed.** The demo key may call `propose`, `schedule_change` and `apply_change` only. The page may also broadcast an already signed raw transaction to Base Sepolia (permissionless) and re-post a run to the verifier. `retry_sign` and `bump_fee` are shown as copyable CLI commands, not buttons.
- **Q7. Footer links.** Repo `https://github.com/Dylansius11/constitution` (D9; exists only after 2026-11-01). The X post link comes from `PUBLIC_X_POST_URL` and stays hidden until the founder publishes.

**Status of this document for D8 (hosting, undecided).** Nothing here depends on server features. Headers such as CSP and caching depend on the host; if the host cannot set headers, use a `meta` CSP [U].

## 13. Sources

Opened this session, 2026-10-26 (managed headless Chromium, isolated profile; no logins, no form submissions; screenshots saved only under `scratch/design-refs/`):

Inspiration hubs
- https://www.awwwards.com/
- https://www.awwwards.com/websites/sites_of_the_day/
- https://www.awwwards.com/websites/typography/
- https://www.awwwards.com/websites/finance/
- https://www.awwwards.com/websites/editorial/
- https://www.awwwards.com/websites/legal/
- https://www.awwwards.com/websites/law/
- https://www.awwwards.com/websites/blockchain/
- https://www.awwwards.com/websites/security/
- https://www.awwwards.com/websites/magazine/
- https://www.awwwards.com/websites/fintech/
- https://www.awwwards.com/websites/banking/
- https://save.design/
- https://save.design/explore (Industry filter, cybersecurity)
- https://save.design/website/railgun-org

Awwwards entry pages (used for dates, scores, credits and the external URL)
- https://www.awwwards.com/sites/forge-automotive
- https://www.awwwards.com/sites/esr-bespoke
- https://www.awwwards.com/sites/heron-ai
- https://www.awwwards.com/sites/milledollars
- https://www.awwwards.com/sites/signal-a-studio
- https://www.awwwards.com/sites/dicta-legal-poetry-by-h-ahuja
- https://www.awwwards.com/sites/swiss-precision-in-law
- https://www.awwwards.com/sites/orchid-security-2
- https://www.awwwards.com/sites/the-explora-journal
- https://www.awwwards.com/sites/history-of-blockchain
- https://www.awwwards.com/sites/cartier-le-choeur-des-pierres
- https://www.awwwards.com/sites/format
- https://www.awwwards.com/sites/belgrade-arbor
- https://www.awwwards.com/sites/offfforum

Reference sites
- https://forgeautomotive.co.uk/
- https://www.esrbespoke.au/
- https://heronaiapp.com/
- https://milledollars.fr/
- https://signal-a.studio/
- https://railgun.org/
- https://cyera.com/
- https://cotool.ai/
- https://hackfirst.io/
- https://dicta.icaad.ngo/
- https://drpb.ch/ and https://drpb.ch/en
- https://www.orchid.security/
- https://format.obys.agency/
- https://www.offforum.com/
- https://explorajourneys.com/int/en/explora-journal
- https://www.constituteproject.org/constitution/United_States_of_America_1992

Fonts and libraries
- https://github.com/productiontype/Newsreader (OFL 1.1, variable)
- https://github.com/vercel/geist-font (OFL 1.1, Geist Sans, Mono, Pixel)
- https://github.com/JetBrains/JetBrainsMono (OFL 1.1)
- https://github.com/adobe-fonts/source-serif (OFL 1.1, considered and not selected; listed as a fallback serif)
- https://motion.dev/docs/animate

Explorers
- https://testnet.nearblocks.io/ (loaded)
- https://sepolia.basescan.org/ (bot verification page; not bypassed)

Project documents read: `docs/PRD.md`, `docs/technical/ARCHITECTURE.md` (receipt JSON, verifier stages, page responsibilities), `docs/technical/SMART-CONTRACT.md` (proposal and change states). Installed skills read for guidance only: impeccable, design-taste-frontend, emil-design-eng, motion. No skill scripts were run.

Screenshots: `scratch/design-refs/` holds 28 PNG files; 27 are cited in section 2 and `28-dicta-scroll-1280.png` is an unused blank frame.
