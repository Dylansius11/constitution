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
import { renderLadder } from "../live/ladder.ts";
import { replaceValue, timestamp } from "../live/runtime.ts";

/**
 * /app controller. All chain facts are re-read from the gate and the account; the wallet's own
 * answers are never taken as proof. One run at a time (the gate allows one active proposal per
 * account), and nothing proposes without a click.
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

const dataNode = document.getElementById("app-data");
const root = document.getElementById("account");
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

function start(data: AppData, account: HTMLElement): void {
  const gate: GateClient = createGateClient({ rpcUrl: data.nearRpc, gate: data.gate });
  const stepEl = (name: string): HTMLElement => must(account, `[data-step="${name}"]`);
  const connectStep = stepEl("connect");
  const enableStep = stepEl("enable");
  const runStep = stepEl("run");
  const connectBtn = must<HTMLButtonElement>(account, "[data-connect]");
  const connectedBox = must<HTMLElement>(account, "[data-connected]");
  const accountIdEl = must<HTMLElement>(account, "[data-account-id]");
  const disconnectBtn = must<HTMLButtonElement>(account, "[data-disconnect]");
  const walletMessage = must<HTMLElement>(account, "[data-wallet-message]");
  const connectHelp = must<HTMLElement>(account, "[data-connect-help]");
  const enableBtn = must<HTMLButtonElement>(account, "[data-enable]");
  const attemptsSelect = must<HTMLSelectElement>(account, "[data-attempts]");
  const depositNote = must<HTMLElement>(account, "[data-deposit-note]");
  const manage = must<HTMLElement>(account, "[data-manage]");
  const withdrawBtn = must<HTMLButtonElement>(account, "[data-withdraw]");
  const removeKeyBtn = must<HTMLButtonElement>(account, "[data-remove-key]");
  const refreshBtn = must<HTMLButtonElement>(account, "[data-refresh]");
  const setupMessage = must<HTMLElement>(account, "[data-setup-message]");
  const runSummary = must<HTMLElement>(account, "[data-run-summary]");
  const activePanel = must<HTMLElement>(account, "[data-active-panel]");
  const activeText = must<HTMLElement>(account, "[data-active-text]");
  const expireBtn = must<HTMLButtonElement>(account, "[data-expire]");
  const activeMessage = must<HTMLElement>(account, "[data-active-message]");
  const fact = (name: string): HTMLElement => must(account, `[data-fact="${name}"]`);
  const verifierBox = document.querySelector<HTMLElement>("[data-verifier]");
  const cards = new Map<string, HTMLElement>();
  for (const attack of data.attacks) {
    const card = document.querySelector<HTMLElement>(`[data-attack="${attack.id}"]`);
    if (card) cards.set(attack.id, card);
  }

  const state = {
    accountId: null as string | null,
    wallet: "idle" as "idle" | "restoring" | "connecting" | "connected",
    standing: null as Standing | null,
    standingLoading: false,
    standingFailed: false,
    verifier: "checking" as "checking" | "online" | "offline",
    setupBusy: false,
    walletBusy: false,
    busyAttack: null as string | null,
    ownProposal: null as number | null,
    activeExpiryMs: null as number | null,
    attempts: Number(attemptsSelect.value),
  };
  const flows = new Map<string, RunFlow>();
  const lastUpdate = new Map<string, RunFlowUpdate>();
  let wallet: WalletPort | null = null;

  const runKey = (id: string): string => `constitution:run:${data.gate}:${state.accountId}:${id}`;
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

  function setStep(step: HTMLElement, mode: "done" | "current" | "locked", status: string): void {
    step.dataset["state"] = mode;
    must(step, "[data-step-status]").textContent = status;
  }

  function render(): void {
    const connected = state.accountId !== null;
    const standing = state.standing;
    const ready = standing === null ? null : readiness(standing);

    // Step 1
    connectBtn.hidden = connected;
    connectedBox.hidden = !connected;
    connectHelp.hidden = connected;
    accountIdEl.textContent = state.accountId ?? "";
    const connecting = state.wallet === "connecting" || state.wallet === "restoring";
    connectBtn.setAttribute("aria-disabled", String(connecting));
    connectBtn.toggleAttribute("aria-busy", connecting);
    connectBtn.textContent = state.wallet === "restoring" ? "Reconnecting…" : connecting ? "Waiting for the wallet…" : "Connect wallet";
    setStep(connectStep, connected ? "done" : "current", connected ? "Connected" : connecting ? "Connecting" : "Not connected");

    // Step 2
    const keyReady = standing?.key === "active";
    const enableDone = ready?.kind === "ready" && keyReady;
    setStep(
      enableStep,
      !connected ? "locked" : enableDone ? "done" : "current",
      !connected ? "Locked" : standing === null ? (state.standingFailed ? "Unreadable" : "Reading") : enableDone ? "Enabled" : ready?.kind === "ready" ? "Wallet per run" : "Needs credit",
    );
    if (standing) {
      fact("credit").textContent = `${formatNear(standing.credit)} NEAR`;
      fact("runs").textContent = String(attemptsAffordable(standing));
      fact("key").textContent = KEY_LABEL[standing.key];
      fact("fee").textContent = `${formatNear(standing.fee)} NEAR`;
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
      depositNote.textContent = connected ? "Reading the gate's fees." : "Connect a wallet first.";
      enableBtn.hidden = false;
      enableBtn.textContent = "Enable live runs";
      manage.hidden = true;
    }
    enableBtn.setAttribute("aria-disabled", String(!connected || standing === null || state.setupBusy || state.walletBusy));
    enableBtn.toggleAttribute("aria-busy", state.setupBusy);
    refreshBtn.setAttribute("aria-disabled", String(state.standingLoading));

    // Step 3
    const runnable = ready?.kind === "ready";
    setStep(runStep, runnable ? "current" : "locked", runnable ? "Ready" : "Locked");
    runSummary.textContent = !connected
      ? "Connect a wallet and enable live runs to unlock the attacks."
      : runnable && standing
        ? `Credit covers ${attemptsAffordable(standing)} attack${attemptsAffordable(standing) === 1 ? "" : "s"}. ${
            standing.key === "active" ? "Your session key signs each one, with no pop-ups." : "Your wallet will ask you to confirm each one."
          }`
        : "Add credit to unlock the attacks.";
    const own = state.ownProposal !== null && standing?.active === state.ownProposal;
    const showActive = standing !== null && standing.active !== null && !own;
    activePanel.hidden = !showActive;
    if (showActive && standing && standing.active !== null) {
      const expired = state.activeExpiryMs !== null && Date.now() >= state.activeExpiryMs;
      activeText.textContent =
        state.activeExpiryMs === null
          ? `Proposal ${standing.active} is in flight on your account. Watch it below, or let it finish.`
          : expired
            ? `Proposal ${standing.active} passed its deadline. Releasing it returns your reservation so you can run again.`
            : `Proposal ${standing.active} is in flight on your account. It can be released after ${new Date(state.activeExpiryMs).toUTCString()}.`;
      expireBtn.hidden = !expired;
      expireBtn.setAttribute("aria-disabled", String(state.walletBusy));
    }

    // Cards
    const decision = gateFor();
    for (const [id, card] of cards) {
      const button = must<HTMLButtonElement>(card, "[data-run]");
      const hint = must<HTMLElement>(card, "[data-run-hint]");
      const finished = card.dataset["state"] === "done" || card.dataset["state"] === "failed";
      const running = state.busyAttack === id;
      let label = finished ? "Run again" : "Run live";
      let disabled = false;
      let text = "";
      if (running) {
        label = "Running…";
        disabled = true;
      } else if (!decision.ok) {
        text = decision.reason;
        if (decision.action === "connect") label = "Connect wallet to run";
        else if (decision.action === "enable") label = "Enable live runs";
        else disabled = true;
      } else {
        text = decision.signing === "session" ? "Signed by your session key. No pop-up." : "Your wallet will ask you to confirm.";
      }
      button.textContent = label;
      button.setAttribute("aria-disabled", String(disabled));
      button.toggleAttribute("aria-busy", running);
      hint.textContent = running ? "" : text;
    }

    // Verifier chip
    if (verifierBox) {
      verifierBox.dataset["verifierState"] = state.verifier;
      must(verifierBox, "[data-verifier-label]").textContent =
        state.verifier === "online" ? "Demo verifier online" : state.verifier === "offline" ? "Demo verifier offline" : "Checking the demo verifier";
      must<HTMLElement>(verifierBox, "[data-verifier-retry]").hidden = state.verifier !== "offline";
    }
  }

  /* ---------- wallet ---------- */

  async function onAccount(accountId: string): Promise<void> {
    state.accountId = accountId;
    state.wallet = "connected";
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
    render();
    try {
      const accountId = await (await ensureWallet()).connect();
      await onAccount(accountId);
    } catch (error) {
      state.wallet = "idle";
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
    state.accountId = null;
    state.wallet = "idle";
    state.standing = null;
    state.busyAttack = null;
    state.ownProposal = null;
    for (const card of cards.values()) {
      card.dataset["state"] = "idle";
      card.dataset["tone"] = "neutral";
      must<HTMLElement>(card, "[data-live]").hidden = true;
    }
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

  function renderCard(id: string, update: RunFlowUpdate): void {
    const card = cards.get(id);
    if (!card) return;
    const kind = update.state.kind;
    const terminal = TERMINAL[kind] === true;
    const live = must<HTMLElement>(card, "[data-live]");
    live.hidden = false;
    card.dataset["tone"] = update.view.tone;
    card.dataset["state"] = !terminal ? "running" : kind === "error" ? "failed" : "done";
    must(card, "[data-run-title]").textContent = update.view.title;
    must(card, "[data-run-body]").textContent = update.view.body;
    renderLadder(card, update.view.stages);
    const wait = must<HTMLElement>(card, "[data-run-wait]");
    wait.hidden = update.waiting === null;
    if (update.waiting !== null) {
      const seconds = Math.max(1, Math.ceil((update.waiting.retryAtMs - Date.now()) / 1000));
      wait.textContent = `The verifier is busy with another run. Your proposal is recorded; trying again in about ${seconds} s.`;
    }
    const near = must<HTMLAnchorElement>(card, "[data-run-near]");
    near.hidden = update.nearTxHash === null;
    if (update.nearTxHash !== null) near.href = `${data.explorer}/txns/${update.nearTxHash}`;
    const watch = must<HTMLAnchorElement>(card, "[data-run-watch]");
    watch.hidden = update.proposalId === null;
    if (update.proposalId !== null) watch.dataset["proposal"] = String(update.proposalId);

    const result = must<HTMLElement>(card, "[data-result]");
    result.hidden = update.rejection === null || kind !== "rejected";
    if (update.rejection !== null && kind === "rejected") {
      const expectedRule = Number(card.dataset["expectedRule"]);
      const expectedReason = Number(card.dataset["expectedReason"]);
      const match = update.rejection.ruleId === expectedRule && update.rejection.reasonCode === expectedReason;
      result.dataset["match"] = String(match);
      must(result, "[data-result-title]").textContent = `Refused under ${ruleLabel(update.rejection.ruleId)}`;
      must(result, "[data-result-reason]").textContent =
        reasonText(update.rejection.reasonCode) ?? `Reason code ${update.rejection.reasonCode}.`;
      must(result, "[data-result-match]").textContent = match
        ? "This is the rule the attack was built to hit."
        : `The gate refused it, but under a different rule or reason than this card expects (${ruleLabel(expectedRule)}, reason ${expectedReason}).`;
    }
    lastUpdate.set(id, update);
    state.ownProposal = terminal && state.busyAttack === id ? null : (update.proposalId ?? state.ownProposal);
    if (!terminal) state.busyAttack = id;
    else if (state.busyAttack === id) {
      state.busyAttack = null;
      announce(`${update.view.title}.`);
      void refreshStanding();
    }
    render();
  }

  function beginRun(attack: AppAttack, resume: boolean): void {
    const accountId = state.accountId;
    if (accountId === null) return;
    flows.get(attack.id)?.dispose();
    if (!resume) localStorage.removeItem(runKey(attack.id));
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
      (update) => renderCard(attack.id, update),
      { propose: (args) => proposeSigner()(args) },
    );
    flows.set(attack.id, flow);
    if (!resume) state.busyAttack = attack.id;
    render();
    flow.start().catch(() => {
      if (state.busyAttack === attack.id) state.busyAttack = null;
      render();
    });
  }

  function resumeRuns(): void {
    for (const attack of data.attacks)
      if (state.accountId !== null && localStorage.getItem(runKey(attack.id)) !== null) beginRun(attack, true);
  }

  async function onRunClick(attack: AppAttack): Promise<void> {
    const decision = gateFor();
    if (!decision.ok) {
      if (decision.action === "connect") await connect();
      else if (decision.action === "enable") {
        document.getElementById("account")?.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
        enableBtn.focus({ preventScroll: true });
      }
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
  verifierBox?.querySelector("[data-verifier-retry]")?.addEventListener("click", () => void checkVerifier());
  for (const attack of data.attacks) {
    const card = cards.get(attack.id);
    card?.querySelector("[data-run]")?.addEventListener("click", (event) => {
      if (!disabled(event.currentTarget as HTMLElement)) void onRunClick(attack);
    });
    card?.querySelector("[data-run-watch]")?.addEventListener("click", (event) => {
      const id = (event.currentTarget as HTMLElement).dataset["proposal"];
      const input = document.querySelector<HTMLInputElement>("#demo-watch-input");
      const form = input?.closest("form");
      if (id !== undefined && input && form) {
        input.value = id;
        form.requestSubmit();
      }
    });
  }

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
