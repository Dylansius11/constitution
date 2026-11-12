import { attackBundle, type DemoAttackConfig } from "../../lib/demo-config.ts";
import { shortValue } from "../../lib/display.ts";
import { createGateClient, type GateClient } from "../../lib/gate.ts";
import {
  GateWriteError,
  parseProposeOutcome,
  type ProposeSigner,
  proposeWithSessionKey,
} from "../../lib/near-write.ts";
import { reasonText, ruleLabel } from "../../lib/reasons.ts";
import { createRunFlow, type RunFlow, type RunFlowUpdate } from "../../lib/run-flow.ts";
import {
  advanceClock,
  buildSteps,
  formatElapsed,
  newClock,
  PIPELINE,
  type RunClock,
  runFinished,
  runSeconds,
  type Step,
  stageSeconds,
  type WalletSteps,
} from "../../lib/run-steps.ts";
import {
  forgetSessionKey,
  generateSessionKey,
  loadSessionKey,
  type SessionKey,
  saveSessionKey,
} from "../../lib/session-key.ts";
import { planSetup, runSetup } from "../../lib/session-setup.ts";
import {
  attemptsAffordable,
  depositFor,
  formatNear,
  type KeyState,
  type Readiness,
  readiness,
  readStanding,
  type Standing,
} from "../../lib/standing.ts";
import {
  createNearConnectWallet,
  WalletError,
  type WalletPort,
  WalletRejectedError,
} from "../../lib/wallet.ts";
import {
  expireProposalTx,
  proposeTx,
  removeSessionKeyTx,
  withdrawCreditTx,
} from "../../lib/wallet-actions.ts";
import { replaceValue, timestamp } from "../live/runtime.ts";

/**
 * /app controller. All chain facts are re-read from the gate and the account; the wallet's own
 * answers are never taken as proof. One run at a time (the gate allows one active proposal per
 * account), and nothing proposes without a click. One panel shows the selected attack; the
 * stepper, timers and result are painted from that attack's last run update.
 */

interface AppAttack extends Pick<DemoAttackConfig, "id" | "title" | "contract" | "impl" | "codehash" | "initCalldata" | "expectedRule" | "expectedReason" | "files"> {}

interface AppData {
  gate: string;
  proxy: string;
  verifierUrl: string;
  nearRpc: string;
  baseRpc: string;
  explorer: string;
  attacks: AppAttack[];
}

const WALLET_HINT = "constitution:app:wallet";
const TERMINAL: Record<string, true> = { executed: true, rejected: true, expired: true, reverted: true, error: true };
const RULE_STEP = ["r1_layout", "r2_solvency", "r3_filter", "r4_initializer", "r5_replay"] as const;

const dataNode = document.getElementById("app-data");
const root = document.getElementById("run");
if (dataNode?.textContent && root) start(JSON.parse(dataNode.textContent) as AppData, root);

function must<T extends Element>(scope: ParentNode, selector: string): T {
  const found = scope.querySelector<T>(selector);
  if (found === null) throw new Error(`Missing ${selector}`);
  return found;
}

function announce(text: string): void {
  const region = document.getElementById("announcer");
  if (!region) return;
  region.textContent = "";
  requestAnimationFrame(() => {
    region.textContent = text;
  });
}

function setMessage(node: HTMLElement, text: string | null, tone: "ok" | "error" | "pending" = "pending"): void {
  node.hidden = text === null;
  node.dataset["tone"] = tone;
  if (text !== null) node.textContent = text;
}

const KEY_LABEL: Record<KeyState, string> = {
  none: "None yet",
  not_on_chain: "Not on your account",
  low_allowance: "Nearly out of allowance",
  active: "Active",
};

const IDLE_TITLE = "Ready";
const IDLE_BODY = "Starting proposes this upgrade to the demo gate. The verifier then checks it against the rules.";

function start(data: AppData, panel: HTMLElement): void {
  const gate: GateClient = createGateClient({ rpcUrl: data.nearRpc, gate: data.gate });
  const pane = (name: string): HTMLElement => must(panel, `[data-pane="${name}"]`);
  const connectPane = pane("connect");
  const enablePane = pane("enable");
  const runPane = pane("run");
  const connectBtn = must<HTMLButtonElement>(panel, "[data-connect]");
  const accountDetails = must<HTMLDetailsElement>(panel, "[data-account-details]");
  const accountSummary = must<HTMLElement>(panel, "[data-account-summary]");
  const accountIdEl = must<HTMLElement>(panel, "[data-account-id]");
  const disconnectBtn = must<HTMLButtonElement>(panel, "[data-disconnect]");
  const walletMessage = must<HTMLElement>(panel, "[data-wallet-message]");
  const enableBtn = must<HTMLButtonElement>(panel, "[data-enable]");
  const attemptsSelect = must<HTMLSelectElement>(panel, "[data-attempts]");
  const depositNote = must<HTMLElement>(panel, "[data-deposit-note]");
  const manage = must<HTMLElement>(panel, "[data-manage]");
  const withdrawBtn = must<HTMLButtonElement>(panel, "[data-withdraw]");
  const removeKeyBtn = must<HTMLButtonElement>(panel, "[data-remove-key]");
  const refreshBtn = must<HTMLButtonElement>(panel, "[data-refresh]");
  const setupMessage = must<HTMLElement>(panel, "[data-setup-message]");
  const activePanel = must<HTMLElement>(panel, "[data-active-panel]");
  const activeText = must<HTMLElement>(panel, "[data-active-text]");
  const expireBtn = must<HTMLButtonElement>(panel, "[data-expire]");
  const activeMessage = must<HTMLElement>(panel, "[data-active-message]");
  const runBtn = must<HTMLButtonElement>(panel, "[data-run]");
  const runHint = must<HTMLElement>(panel, "[data-run-hint]");
  const runTitle = must<HTMLElement>(panel, "[data-run-title]");
  const runBody = must<HTMLElement>(panel, "[data-run-body]");
  const runWait = must<HTMLElement>(panel, "[data-run-wait]");
  const runTimer = must<HTMLElement>(panel, "[data-run-timer]");
  const runNear = must<HTMLAnchorElement>(panel, "[data-run-near]");
  const runWatch = must<HTMLAnchorElement>(panel, "[data-run-watch]");
  const result = must<HTMLElement>(panel, "[data-result]");
  const stepEls = new Map<string, HTMLElement>();
  for (const li of panel.querySelectorAll<HTMLElement>("[data-step]")) stepEls.set(li.dataset["step"] ?? "", li);
  const radios = [...panel.querySelectorAll<HTMLInputElement>('input[name="attack"]')];
  const pickLock = must<HTMLElement>(panel, "[data-pick-lock]");
  const fact = (name: string): HTMLElement => must(panel, `[data-fact="${name}"]`);
  const verifierBox = document.querySelector<HTMLElement>("[data-verifier]");
  const views = new Map<string, HTMLElement>();
  for (const view of panel.querySelectorAll<HTMLElement>("[data-attack-view]")) views.set(view.dataset["attackView"] ?? "", view);
  const attackById = new Map(data.attacks.map((attack) => [attack.id, attack] as const));
  const firstAttack = data.attacks[0];
  if (firstAttack === undefined) throw new Error("The gallery has no attacks");

  const state = {
    accountId: null as string | null,
    wallet: "idle" as "idle" | "restoring" | "connecting" | "connected",
    connectFailed: false,
    standing: null as Standing | null,
    standingLoading: false,
    standingFailed: false,
    verifier: "checking" as "checking" | "online" | "offline",
    setupBusy: false,
    walletBusy: false,
    selected: firstAttack.id,
    busyAttack: null as string | null,
    ownProposal: null as number | null,
    activeExpiryMs: null as number | null,
    attempts: Number(attemptsSelect.value),
  };
  const flows = new Map<string, RunFlow>();
  const lastUpdate = new Map<string, RunFlowUpdate>();
  const lastSteps = new Map<string, Step[]>();
  const clocks = new Map<string, RunClock>();
  const resumed = new Set<string>();
  let ticker: ReturnType<typeof setInterval> | undefined;
  let wallet: WalletPort | null = null;

  const runKey = (id: string): string => `constitution:run:${data.gate}:${state.accountId}:${id}`;
  const startKey = (id: string): string => `${runKey(id)}:started`;
  const sessionKey = (): SessionKey | null =>
    state.accountId === null ? null : loadSessionKey(localStorage, data.gate, state.accountId);

  async function ensureWallet(): Promise<WalletPort> {
    if (wallet === null) {
      wallet = await createNearConnectWallet();
      wallet.onSignOut(() => {
        void signedOutByWallet();
      });
    }
    return wallet;
  }

  /* ---------- reads ---------- */

  async function refreshStanding(): Promise<Standing | null> {
    const accountId = state.accountId;
    if (accountId === null) return null;
    state.standingLoading = true;
    render();
    try {
      const key = loadSessionKey(localStorage, data.gate, accountId);
      const standing = await readStanding(gate, accountId, key?.publicKey ?? null);
      if (state.accountId !== accountId) return null;
      state.standing = standing;
      state.standingFailed = false;
      state.activeExpiryMs = null;
      if (standing.active !== null) {
        try {
          const proposal = await gate.getProposal(standing.active);
          state.activeExpiryMs = proposal.value ? Number(proposal.value.expires_at_ns / 1_000_000n) : null;
        } catch {
          // The expiry is a convenience; the active proposal itself was read.
        }
      }
      return standing;
    } catch {
      if (state.accountId === accountId) state.standingFailed = true;
      return null;
    } finally {
      state.standingLoading = false;
      render();
    }
  }

  async function checkVerifier(): Promise<void> {
    state.verifier = "checking";
    render();
    try {
      // Deliberately lazy: the verifier client is only needed once the page is open and a run may start.
      const { createVerifierClient } = await import("../../lib/verifier.ts");
      const identity = await createVerifierClient({ baseUrl: data.verifierUrl, timeoutMs: 8000 }).identity();
      state.verifier = "online";
      const detail =
        identity.label ??
        (identity.release_hash ? `Release ${shortValue(identity.release_hash, "hash", "narrow")}` : "No measured release");
      if (verifierBox) must(verifierBox, "[data-verifier-detail]").textContent = `${detail}. One run at a time.`;
    } catch {
      state.verifier = "offline";
      if (verifierBox)
        must(verifierBox, "[data-verifier-detail]").textContent =
          "It did not answer. Runs cannot start until it does.";
    }
    render();
  }

  async function readRuleSet(): Promise<void> {
    const hashCell = document.querySelector<HTMLElement>("[data-ruleset-hash]");
    const note = document.querySelector<HTMLElement>("[data-ruleset-note]");
    if (!hashCell || !note) return;
    try {
      const view = await gate.getConstitution();
      replaceValue(hashCell, view.value.rule_set_hash, "Demo rule set hash");
      note.replaceChildren(timestamp(Date.now()), ` · NEAR block ${view.blockHeight}`);
    } catch {
      hashCell.textContent = "Cannot read the demo gate";
    }
  }

  /* ---------- gating ---------- */

  type Gate =
    | { ok: true; signing: "session" | "wallet" }
    | { ok: false; reason: string; action?: "connect" | "enable" };

  function gateFor(): Gate {
    if (state.accountId === null) return { ok: false, reason: "Connect a wallet to run this attack.", action: "connect" };
    if (state.standing === null)
      return { ok: false, reason: state.standingFailed ? "Cannot read the demo gate. Use Refresh in your account." : "Reading your account on the demo gate." };
    const ready: Readiness = readiness(state.standing);
    if (ready.kind === "active_proposal")
      return { ok: false, reason: `Proposal ${ready.proposalId} is already in flight on your account. It must finish or expire first.` };
    if (ready.kind === "needs_credit")
      return { ok: false, reason: "Add credit to run attacks.", action: "enable" };
    if (state.verifier === "offline") return { ok: false, reason: "The demo verifier is offline. Check it again above." };
    if (state.verifier === "checking") return { ok: false, reason: "Checking the demo verifier." };
    if (state.busyAttack !== null) return { ok: false, reason: "Another attack is running. One run at a time." };
    return { ok: true, signing: ready.signing };
  }

  /* ---------- render ---------- */

  function walletSteps(): WalletSteps {
    const standing = state.standing;
    const connecting = state.wallet === "connecting" || state.wallet === "restoring";
    return {
      connected: state.accountId !== null,
      connecting,
      connectFailed: state.connectFailed,
      standing:
        standing === null
          ? state.standingFailed
            ? "unreadable"
            : "reading"
          : readiness(standing).kind === "needs_credit"
            ? "needs_credit"
            : "ready",
    };
  }

  function stepsFor(id: string): Step[] {
    const update = lastUpdate.get(id);
    return buildSteps(walletSteps(), update === undefined ? null : { state: update.state, stages: update.view.stages });
  }

  const now = (): number => Date.now();

  /** Stepper, run timer and per-step times for the selected attack. */
  function paintSteps(): void {
    const steps = stepsFor(state.selected);
    lastSteps.set(state.selected, steps);
    const clock = clocks.get(state.selected);
    const target = attackById.get(state.selected)?.expectedRule;
    for (const step of steps) {
      const li = stepEls.get(step.id);
      if (!li) continue;
      li.dataset["state"] = step.state;
      must(li, "[data-step-status]").textContent = step.status;
      const isTarget = target !== undefined && step.id === RULE_STEP[target - 1];
      must<HTMLElement>(li, "[data-step-target]").hidden = !isTarget;
      li.toggleAttribute("data-target", isTarget);
    }
    paintTimes(steps, clock);
  }

  function paintTimes(steps: readonly Step[], clock: RunClock | undefined): void {
    const t = now();
    PIPELINE.forEach((spec, i) => {
      const li = stepEls.get(spec.id);
      if (!li) return;
      const seconds = clock === undefined ? null : stageSeconds(clock, i, t);
      must(li, "[data-step-time]").textContent = seconds === null ? "" : `${seconds} s`;
    });
    const total = clock === undefined ? null : runSeconds(clock, t);
    const finished = clock !== undefined && (clock.endMs !== null || runFinished(steps));
    runTimer.textContent = total === null ? (clock === undefined ? "0:00" : "—") : formatElapsed(total * 1000);
    runTimer.dataset["running"] = String(clock !== undefined && !finished);
  }

  function ensureTicker(): void {
    const live = [...clocks.values()].some((clock) => clock.endMs === null && clock.known);
    if (live && ticker === undefined) {
      ticker = setInterval(() => {
        const clock = clocks.get(state.selected);
        const steps = lastSteps.get(state.selected);
        if (steps) paintTimes(steps, clock);
        if (![...clocks.values()].some((c) => c.endMs === null && c.known)) {
          clearInterval(ticker);
          ticker = undefined;
        }
      }, 1000);
    }
  }

  /** The run area of the selected attack: status, wait notice, links and the result card. */
  function paintRun(): void {
    const id = state.selected;
    const update = lastUpdate.get(id);
    const card = attackById.get(id);
    const kind = update?.state.kind;
    panel.dataset["tone"] = update?.view.tone ?? "neutral";
    runTitle.textContent = update?.view.title ?? IDLE_TITLE;
    runBody.textContent = update?.view.body ?? IDLE_BODY;
    const waiting = update?.waiting ?? null;
    runWait.hidden = waiting === null;
    if (waiting !== null) {
      const seconds = Math.max(1, Math.ceil((waiting.retryAtMs - now()) / 1000));
      runWait.textContent = `The verifier is busy with another run. Your proposal is recorded and waiting in line; trying again in about ${seconds} s.`;
    }
    runNear.hidden = update?.nearTxHash == null;
    if (update?.nearTxHash != null) runNear.href = `${data.explorer}/txns/${update.nearTxHash}`;
    runWatch.hidden = update?.proposalId == null;
    if (update?.proposalId != null) runWatch.dataset["proposal"] = String(update.proposalId);

    const rejection = update?.rejection ?? null;
    result.hidden = rejection === null || kind !== "rejected";
    if (rejection !== null && kind === "rejected" && card !== undefined) {
      const match = rejection.ruleId === card.expectedRule && rejection.reasonCode === card.expectedReason;
      result.dataset["match"] = String(match);
      must(result, "[data-result-title]").textContent = `The gate held. Refused under ${ruleLabel(rejection.ruleId)}, reason ${rejection.reasonCode}.`;
      must(result, "[data-result-reason]").textContent = reasonText(rejection.reasonCode) ?? `Reason code ${rejection.reasonCode}.`;
      must(result, "[data-result-match]").textContent = match
        ? "This is the rule the attack was built to hit. No signature was requested and the proxy was not touched."
        : `The gate refused it, but under a different rule or reason than expected (${ruleLabel(card.expectedRule)}, reason ${card.expectedReason}).`;
    }
  }

  function pickStatus(id: string): string {
    const update = lastUpdate.get(id);
    if (update === undefined) return "Not run yet";
    const kind = update.state.kind;
    if (kind === "rejected") return `Refused · ${ruleLabel(update.state.ruleId)}`;
    if (kind === "executed") return "Executed";
    if (TERMINAL[kind] === true) return kind === "error" ? "Needs a retry" : "Ended";
    return "Running";
  }

  function render(): void {
    const connected = state.accountId !== null;
    const standing = state.standing;
    const ready = standing === null ? null : readiness(standing);
    const runnable = ready?.kind === "ready";
    const keyReady = standing?.key === "active";
    const selectedHasRun = lastUpdate.has(state.selected);

    // Which controls belong to the current step.
    connectPane.hidden = connected;
    enablePane.hidden = !connected || (ready !== null && ready.kind === "active_proposal") || (runnable && keyReady);
    runPane.hidden = !connected || (!runnable && !selectedHasRun);
    accountDetails.hidden = !connected;

    // Wallet
    accountIdEl.textContent = state.accountId ?? "";
    const connecting = state.wallet === "connecting" || state.wallet === "restoring";
    connectBtn.setAttribute("aria-disabled", String(connecting));
    connectBtn.toggleAttribute("aria-busy", connecting);
    connectBtn.textContent = state.wallet === "restoring" ? "Reconnecting…" : connecting ? "Waiting for the wallet…" : "Connect wallet";

    // Credit and key
    if (standing) {
      fact("credit").textContent = `${formatNear(standing.credit)} NEAR`;
      fact("runs").textContent = String(attemptsAffordable(standing));
      fact("key").textContent = KEY_LABEL[standing.key];
      fact("fee").textContent = `${formatNear(standing.fee)} NEAR`;
      accountSummary.textContent = `${formatNear(standing.credit)} NEAR credit · covers ${attemptsAffordable(standing)} attack${attemptsAffordable(standing) === 1 ? "" : "s"}`;
      const deposit = depositFor(standing, state.attempts);
      depositNote.textContent =
        deposit === 0n
          ? "Your credit already covers that many attacks."
          : `Deposits ${formatNear(deposit)} NEAR. Each attack spends ${formatNear(standing.fee)} NEAR for good; the ${formatNear(standing.required)} NEAR reservation returns after every run, and you can withdraw the rest.`;
      const needsSomething = deposit > 0n || standing.key !== "active";
      enableBtn.hidden = !needsSomething;
      enableBtn.textContent =
        standing.key === "low_allowance" ? "Fresh session key needed" : deposit > 0n ? "Enable live runs" : "Add the session key";
      manage.hidden = standing.credit === 0n && standing.key === "none";
      withdrawBtn.setAttribute("aria-disabled", String(standing.credit === 0n || standing.active !== null || state.walletBusy));
      removeKeyBtn.hidden = standing.key === "none";
    } else {
      for (const name of ["credit", "runs", "key", "fee"]) fact(name).textContent = connected ? "Reading…" : "Not read";
      accountSummary.textContent = connected ? "Reading the gate" : "";
      depositNote.textContent = connected ? "Reading the gate's fees." : "Connect a wallet first.";
      enableBtn.hidden = false;
      enableBtn.textContent = "Enable live runs";
      manage.hidden = true;
    }
    enableBtn.setAttribute("aria-disabled", String(!connected || standing === null || state.setupBusy || state.walletBusy));
    enableBtn.toggleAttribute("aria-busy", state.setupBusy);
    refreshBtn.setAttribute("aria-disabled", String(state.standingLoading));

    // A proposal of this account that this page did not start (or that outlived a reload).
    const own = state.ownProposal !== null && standing?.active === state.ownProposal;
    const showActive = standing !== null && standing.active !== null && !own;
    activePanel.hidden = !showActive;
    if (showActive && standing && standing.active !== null) {
      const expired = state.activeExpiryMs !== null && now() >= state.activeExpiryMs;
      activeText.textContent =
        state.activeExpiryMs === null
          ? `Proposal ${standing.active} is in flight on your account. Look it up below, or let it finish.`
          : expired
            ? `Proposal ${standing.active} passed its deadline. Releasing it returns your reservation so you can run again.`
            : `Proposal ${standing.active} is in flight on your account. It can be released after ${new Date(state.activeExpiryMs).toUTCString()}.`;
      expireBtn.hidden = !expired;
      expireBtn.setAttribute("aria-disabled", String(state.walletBusy));
    }

    // The run button of the selected attack
    const decision = gateFor();
    const running = state.busyAttack === state.selected;
    const finished = selectedHasRun && !running && TERMINAL[lastUpdate.get(state.selected)?.state.kind ?? ""] === true;
    let label = finished ? "Run again" : "Run this attack live";
    let disabled = false;
    let hint = "";
    if (running) {
      label = "Running…";
      disabled = true;
    } else if (!decision.ok) {
      hint = decision.reason;
      if (decision.action === "enable") label = "Enable live runs first";
      disabled = decision.action !== "connect";
    } else {
      hint = decision.signing === "session" ? "Signed by your session key. No pop-up." : "Your wallet will ask you to confirm.";
    }
    runBtn.textContent = label;
    runBtn.setAttribute("aria-disabled", String(disabled));
    runBtn.toggleAttribute("aria-busy", running);
    runHint.textContent = running ? "" : hint;

    // Picker: free to browse, locked while a run is in flight
    const locked = state.busyAttack !== null;
    pickLock.hidden = !locked;
    for (const radio of radios) {
      radio.disabled = locked && radio.value !== state.selected;
      radio.checked = radio.value === state.selected;
      const status = panel.querySelector<HTMLElement>(`[data-pick-status="${radio.value}"]`);
      if (status) status.textContent = pickStatus(radio.value);
      const body = radio.closest<HTMLElement>(".rp-card");
      if (body) body.dataset["state"] = lastUpdate.get(radio.value)?.state.kind ?? "idle";
    }
    for (const [id, view] of views) view.hidden = id !== state.selected;

    paintSteps();
    paintRun();

    // Verifier chip
    if (verifierBox) {
      verifierBox.dataset["verifierState"] = state.verifier;
      must(verifierBox, "[data-verifier-label]").textContent =
        state.verifier === "online" ? "Demo verifier online" : state.verifier === "offline" ? "Demo verifier offline" : "Checking the demo verifier";
      must<HTMLElement>(verifierBox, "[data-verifier-retry]").hidden = state.verifier !== "offline";
    }
  }

  function select(id: string): void {
    if (!attackById.has(id) || state.selected === id) return;
    state.selected = id;
    render();
  }

  /* ---------- wallet ---------- */

  async function onAccount(accountId: string): Promise<void> {
    state.accountId = accountId;
    state.wallet = "connected";
    state.connectFailed = false;
    state.standing = null;
    state.standingFailed = false;
    localStorage.setItem(WALLET_HINT, "1");
    render();
    announce(`Connected as ${accountId}.`);
    resumeRuns();
    await refreshStanding();
  }

  async function connect(): Promise<void> {
    if (state.wallet === "connecting" || state.wallet === "restoring") return;
    setMessage(walletMessage, null);
    state.wallet = "connecting";
    state.connectFailed = false;
    render();
    try {
      const accountId = await (await ensureWallet()).connect();
      await onAccount(accountId);
    } catch (error) {
      state.wallet = "idle";
      state.connectFailed = !(error instanceof WalletRejectedError);
      render();
      setMessage(
        walletMessage,
        error instanceof WalletRejectedError
          ? "The wallet connection was cancelled. Connect again whenever you are ready."
          : "The wallet did not connect. Try again, or choose another wallet.",
        error instanceof WalletRejectedError ? "pending" : "error",
      );
    }
  }

  function clearSession(): void {
    for (const flow of flows.values()) flow.dispose();
    flows.clear();
    lastUpdate.clear();
    lastSteps.clear();
    clocks.clear();
    resumed.clear();
    state.accountId = null;
    state.wallet = "idle";
    state.standing = null;
    state.busyAttack = null;
    state.ownProposal = null;
    setMessage(setupMessage, null);
    setMessage(activeMessage, null);
    render();
  }

  async function disconnect(): Promise<void> {
    localStorage.removeItem(WALLET_HINT);
    try {
      await wallet?.disconnect();
    } finally {
      clearSession();
      announce("Disconnected.");
    }
  }

  async function signedOutByWallet(): Promise<void> {
    localStorage.removeItem(WALLET_HINT);
    clearSession();
  }

  /* ---------- credit and session key ---------- */

  async function enable(): Promise<void> {
    const standing = state.standing;
    const accountId = state.accountId;
    if (standing === null || accountId === null || state.setupBusy || state.walletBusy) return;
    if (standing.key === "low_allowance") {
      setMessage(setupMessage, "Your session key is nearly out of gas allowance. Remove it, then enable live runs again for a fresh one.", "pending");
      return;
    }
    state.setupBusy = true;
    setMessage(setupMessage, "Waiting for your wallet to confirm.", "pending");
    render();
    let generated: SessionKey | null = null;
    try {
      let key = loadSessionKey(localStorage, data.gate, accountId);
      if (key === null) {
        generated = await generateSessionKey();
        key = generated;
        saveSessionKey(localStorage, data.gate, accountId, key);
      }
      const current = await readStanding(gate, accountId, key.publicKey);
      const plan = planSetup({ gate: data.gate, accountId, standing: current, key, attempts: state.attempts });
      if (plan.transactions.length === 0) {
        state.standing = current;
        setMessage(setupMessage, "Live runs are already enabled.", "ok");
        return;
      }
      const usedKey = key;
      const result = await runSetup({
        wallet: await ensureWallet(),
        plan,
        attempts: state.attempts,
        read: () => readStanding(gate, accountId, usedKey.publicKey),
      });
      state.standing = result.standing;
      if (result.status === "done") {
        setMessage(setupMessage, `Live runs are enabled. Your credit covers ${attemptsAffordable(result.standing)} attacks.`, "ok");
        announce("Live runs are enabled.");
      } else {
        setMessage(
          setupMessage,
          `${
            result.missing.includes("session_key")
              ? "Your wallet did not add the session key, so it will ask you to confirm each run. "
              : ""
          }${result.missing.includes("credit") ? "The credit deposit is not visible on the gate yet; use Refresh in a moment. " : ""}`.trim(),
          "pending",
        );
      }
    } catch (error) {
      if (generated !== null) {
        // A key that never reached the account must not linger as a half-configured session.
        const reread = await readStanding(gate, accountId, generated.publicKey).catch(() => null);
        if (reread === null || reread.key !== "active") forgetSessionKey(localStorage, data.gate, accountId);
      }
      setMessage(
        setupMessage,
        error instanceof WalletRejectedError
          ? "You cancelled in the wallet. Nothing was changed."
          : "The wallet reported an error. Refresh to see what the gate actually holds before trying again.",
        error instanceof WalletRejectedError ? "pending" : "error",
      );
    } finally {
      state.setupBusy = false;
      render();
      void refreshStanding();
    }
  }

  async function walletAction(
    action: () => Promise<void>,
    message: HTMLElement,
    done: string,
  ): Promise<void> {
    if (state.walletBusy || state.setupBusy) return;
    state.walletBusy = true;
    setMessage(message, "Waiting for your wallet to confirm.", "pending");
    render();
    try {
      await action();
      setMessage(message, done, "ok");
    } catch (error) {
      setMessage(
        message,
        error instanceof WalletRejectedError
          ? "You cancelled in the wallet. Nothing was changed."
          : "The wallet reported an error. Refresh to see what actually changed.",
        error instanceof WalletRejectedError ? "pending" : "error",
      );
    } finally {
      state.walletBusy = false;
      render();
      void refreshStanding();
    }
  }

  async function withdraw(): Promise<void> {
    const standing = state.standing;
    if (standing === null || standing.credit === 0n || standing.active !== null) return;
    const amount = standing.credit;
    await walletAction(
      async () => void (await (await ensureWallet()).send([withdrawCreditTx(data.gate, amount)])),
      setupMessage,
      `Withdrew ${formatNear(amount)} NEAR of credit back to your account.`,
    );
  }

  async function removeKey(): Promise<void> {
    const accountId = state.accountId;
    const key = sessionKey();
    if (accountId === null || key === null) return;
    const onChain = state.standing?.key === "active" || state.standing?.key === "low_allowance";
    await walletAction(
      async () => {
        if (onChain) await (await ensureWallet()).send([removeSessionKeyTx(accountId, key.publicKey)]);
        forgetSessionKey(localStorage, data.gate, accountId);
      },
      setupMessage,
      "The session key is removed from your account and from this browser.",
    );
  }

  async function expire(): Promise<void> {
    const active = state.standing?.active;
    if (active === null || active === undefined) return;
    await walletAction(
      async () => void (await (await ensureWallet()).send([expireProposalTx(data.gate, active)])),
      activeMessage,
      `Proposal ${active} is released.`,
    );
  }

  /* ---------- runs ---------- */

  function proposeSigner(): ProposeSigner {
    const accountId = state.accountId;
    const key = sessionKey();
    if (accountId === null) throw new Error("No account");
    if (state.standing?.key === "active" && key !== null) {
      return (args) =>
        proposeWithSessionKey(
          { nearRpc: data.nearRpc, gateAccount: data.gate, accountId, secretKey: key.secretKey },
          args,
        );
    }
    return async (args) => {
      try {
        const [outcome] = await (await ensureWallet()).send([proposeTx(data.gate, args)]);
        return parseProposeOutcome(outcome, data.gate);
      } catch (error) {
        if (error instanceof WalletRejectedError)
          throw new GateWriteError("wallet_declined", "You cancelled the wallet confirmation. Nothing was proposed.");
        if (error instanceof WalletError)
          throw new GateWriteError("other", "The wallet reported an error before confirming. Check your account before trying again.");
        throw error;
      }
    };
  }

  /** Stores the update, advances that attack's clocks, and repaints when it is the selected attack. */
  function onUpdate(id: string, update: RunFlowUpdate): void {
    const kind = update.state.kind;
    const terminal = TERMINAL[kind] === true;
    const first = !lastUpdate.has(id);
    lastUpdate.set(id, update);

    let clock = clocks.get(id);
    if (clock === undefined) {
      const stored = Number(localStorage.getItem(startKey(id)));
      const wasResumed = resumed.has(id);
      // After a reload the end of a run that already finished is unknown, so no time is claimed.
      clock = Number.isFinite(stored) && stored > 0 ? newClock(stored, !(wasResumed && terminal)) : newClock(now(), !wasResumed);
      clocks.set(id, clock);
    }
    const steps = buildSteps(walletSteps(), { state: update.state, stages: update.view.stages });
    if (clock.known) advanceClock(clock, steps, now());
    else if (clock.endMs === null && runFinished(steps)) clock.endMs = now();

    state.ownProposal = terminal && state.busyAttack === id ? null : (update.proposalId ?? state.ownProposal);
    if (!terminal) {
      if (state.busyAttack !== id && first && state.selected !== id) state.selected = id;
      state.busyAttack = id;
    } else if (state.busyAttack === id) {
      state.busyAttack = null;
      announce(`${update.view.title}.`);
      void refreshStanding();
    }
    render();
    ensureTicker();
  }

  function beginRun(attack: AppAttack, resume: boolean): void {
    const accountId = state.accountId;
    if (accountId === null) return;
    flows.get(attack.id)?.dispose();
    if (resume) resumed.add(attack.id);
    else {
      localStorage.removeItem(runKey(attack.id));
      localStorage.setItem(startKey(attack.id), String(now()));
      clocks.set(attack.id, newClock(now()));
      lastUpdate.delete(attack.id);
      resumed.delete(attack.id);
    }
    const label = attack.contract.split(":")[1] ?? attack.contract;
    const flow = createRunFlow(
      {
        nearRpc: data.nearRpc,
        gateAccount: data.gate,
        verifierUrl: data.verifierUrl,
        baseRpc: data.baseRpc,
        label,
        storageKey: runKey(attack.id),
        proposal: {
          newImpl: attack.impl,
          newImplCodehash: attack.codehash,
          initCalldata: attack.initCalldata,
          bundle: attackBundle(attack),
        },
      },
      (update) => onUpdate(attack.id, update),
      { propose: (args) => proposeSigner()(args) },
    );
    flows.set(attack.id, flow);
    if (!resume) state.busyAttack = attack.id;
    render();
    ensureTicker();
    flow.start().catch(() => {
      if (state.busyAttack === attack.id) state.busyAttack = null;
      render();
    });
  }

  function resumeRuns(): void {
    for (const attack of data.attacks)
      if (state.accountId !== null && localStorage.getItem(runKey(attack.id)) !== null) beginRun(attack, true);
  }

  async function onRunClick(): Promise<void> {
    const attack = attackById.get(state.selected);
    if (attack === undefined) return;
    const decision = gateFor();
    if (!decision.ok) {
      if (decision.action === "connect") await connect();
      else if (decision.action === "enable") enableBtn.focus();
      return;
    }
    beginRun(attack, false);
  }

  /* ---------- wiring ---------- */

  const disabled = (button: HTMLElement): boolean => button.getAttribute("aria-disabled") === "true";
  connectBtn.addEventListener("click", () => {
    if (!disabled(connectBtn)) void connect();
  });
  disconnectBtn.addEventListener("click", () => void disconnect());
  enableBtn.addEventListener("click", () => {
    if (!disabled(enableBtn)) void enable();
  });
  withdrawBtn.addEventListener("click", () => {
    if (!disabled(withdrawBtn)) void withdraw();
  });
  removeKeyBtn.addEventListener("click", () => void removeKey());
  refreshBtn.addEventListener("click", () => {
    if (!disabled(refreshBtn)) void refreshStanding();
  });
  expireBtn.addEventListener("click", () => {
    if (!disabled(expireBtn)) void expire();
  });
  attemptsSelect.addEventListener("change", () => {
    state.attempts = Number(attemptsSelect.value);
    render();
  });
  runBtn.addEventListener("click", () => {
    if (!disabled(runBtn)) void onRunClick();
  });
  for (const radio of radios) radio.addEventListener("change", () => select(radio.value));
  verifierBox?.querySelector("[data-verifier-retry]")?.addEventListener("click", () => void checkVerifier());
  const lookup = (id: string | undefined): void => {
    const input = document.querySelector<HTMLInputElement>("#demo-watch-input");
    const form = input?.closest("form");
    if (id !== undefined && input && form) {
      input.value = id;
      form.requestSubmit();
    }
  };
  runWatch.addEventListener("click", () => lookup(runWatch.dataset["proposal"]));
  for (const button of document.querySelectorAll<HTMLElement>("[data-lookup-id]"))
    button.addEventListener("click", () => lookup(button.dataset["lookupId"]));

  render();
  void checkVerifier();
  void readRuleSet();
  if (localStorage.getItem(WALLET_HINT) !== null) {
    state.wallet = "restoring";
    render();
    ensureWallet()
      .then((w) => w.restore())
      .then((accountId) => {
        if (accountId === null) {
          localStorage.removeItem(WALLET_HINT);
          state.wallet = "idle";
          render();
          return;
        }
        return onAccount(accountId);
      })
      .catch(() => {
        state.wallet = "idle";
        render();
      });
  }
}
