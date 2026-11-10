import type { ProxyRead } from "../../lib/base.ts";
import type { View } from "../../lib/gate.ts";
import type { Config, Constitution, ListedVerifier, PendingChange } from "../../lib/gate-types.ts";
import type { Source } from "../../lib/state.ts";
import { hashValue, lastObserved, observe, pollVisible, timestamp } from "./runtime.ts";

async function initializeLedger(
  root: HTMLElement,
  gateAccount: string,
  nearRpc: string,
  baseRpc: string,
): Promise<void> {
  const status = root.querySelector<HTMLOutputElement>("[data-ledger-status]");
  const retry = root.querySelector<HTMLButtonElement>("[data-ledger-retry]");
  if (!status || !retry) return;

  // Static imports would download the RPC/viem clients even on the unconfigured static page.
  const [{ createGateClient }, { createBaseClient }, { getAddress }] = await Promise.all([
    import("../../lib/gate.ts"),
    import("../../lib/base.ts"),
    import("viem"),
  ]);
  const gate = createGateClient({ rpcUrl: nearRpc, gate: gateAccount });
  const base = createBaseClient({ rpcUrl: baseRpc });
  let config: Source<View<Config>> = { status: "idle" };
  let rules: Source<View<Constitution>> = { status: "idle" };
  let verifiers: Source<View<ListedVerifier[]>> = { status: "idle" };
  let changes: Source<View<PendingChange[]>> = { status: "idle" };
  let proxy: Source<ProxyRead> = { status: "idle" };
  let busy = false;
  let lastNotice = "";
  let wasOutage = false;

  function row<T>(
    key: string,
    source: Source<T>,
    sourceName: string,
    value: (v: T) => Node | string,
    block: (v: T) => string,
  ): void {
    const item = root.querySelector<HTMLElement>(`[data-ledger-row="${key}"]`);
    if (!item) return;

    const target = item.querySelector<HTMLElement>("[data-value]");
    const meta = item.querySelector<HTMLElement>("[data-row-time]");
    const error = item.querySelector<HTMLElement>("[data-row-error]");
    if (!target || !meta || !error) return;

    const observed = lastObserved(source);
    const stale = source.status === "unavailable" && source.failures >= 3;
    item.dataset["stale"] = String(stale);
    if (observed) {
      // Keep the existing disclosure/copy controls and keyboard focus when the value did not change.
      const node = value(observed.value);
      const fingerprint = typeof node === "string" ? node : (node.textContent ?? "");
      if (target.dataset["rendered"] !== fingerprint) {
        target.replaceChildren(node);
        target.dataset["rendered"] = fingerprint;
      }
      meta.replaceChildren(
        timestamp(
          observed.observedAtMs,
          source.status === "unavailable" ? "Last observed" : "Observed",
        ),
        ` · ${block(observed.value)}`,
      );
    }
    error.textContent =
      source.status === "unavailable"
        ? `${stale ? "Not live. " : ""}Cannot read ${sourceName}.`
        : "";
  }
  const refresh = async (): Promise<void> => {
    if (busy || document.hidden) return;
    busy = true;
    try {
      [config, rules, verifiers, changes] = await Promise.all([
        observe(config, () => gate.getConfig()),
        observe(rules, () => gate.getConstitution()),
        observe(verifiers, () => gate.listVerifiers(0, 50)),
        observe(changes, () => gate.listChanges(0, 50)),
      ]);
      if (config.status === "ok") {
        const days = config.value.value.governance_delay_days;
        const timelock = document.querySelector<HTMLElement>("#timelock");
        if (timelock) timelock.dataset["governanceDelayDays"] = String(days);
        document.dispatchEvent(new CustomEvent("gate-config-observed", {
          detail: { governance_delay_days: days },
        }));
      }
      const cfg = lastObserved(config)?.value.value;
      if (cfg)
        proxy = await observe(proxy, () => {
          if (cfg.chain_id !== 84532) throw new Error("Gate is not configured for Base Sepolia");
          return base.readImplementation(cfg.proxy);
        });
      else
        proxy = await observe(proxy, async () => {
          throw new Error("Gate configuration has not been observed; the proxy address is unknown");
        });
      row(
        "gate",
        config,
        "NEAR testnet get_config",
        () => hashValue(gateAccount, "Gate account", "near"),
        (v) => `NEAR block ${v.blockHeight}`,
      );
      row(
        "rules",
        rules,
        "NEAR testnet get_constitution",
        (v) => hashValue(v.value.rule_set_hash, "Active rule set hash"),
        (v) => `NEAR block ${v.blockHeight}`,
      );
      row(
        "verifiers",
        verifiers,
        "NEAR testnet list_verifiers",
        (v) => {
          if (v.value.length === 0) return "No verifier allowlisted.";
          const list = document.createElement("ul");
          for (const verifier of v.value) {
            const li = document.createElement("li");
            li.append(
              hashValue(verifier.pubkey_b64, "Allowlisted verifier key"),
              hashValue(verifier.record.release_hash, "Verifier release hash"),
            );
            list.append(li);
          }
          return list;
        },
        (v) => `NEAR block ${v.blockHeight}`,
      );
      row(
        "changes",
        changes,
        "NEAR testnet list_changes",
        (v) =>
          `${v.value.filter((c) => c.status === "Pending").length} pending in the first ${v.value.length} returned changes.`,
        (v) => `NEAR block ${v.blockHeight}`,
      );
      row(
        "proxy",
        proxy,
        "Base Sepolia proxy implementation",
        (v) => hashValue(getAddress(v.implementation), "Protected proxy implementation", "address"),
        (v) => `Base Sepolia block ${v.block}`,
      );
      const sources: Source<unknown>[] = [config, rules, verifiers, changes, proxy];
      const outage = sources.some((s) => s.status === "unavailable" && s.failures >= 3);
      const failure = sources.some((s) => s.status === "unavailable");
      const notice = outage
        ? "Not live. Last observed values are retained. Nothing in a failed row is live until its connection returns."
        : failure
          ? "Some sources could not be read. Last observed values are retained; see each row."
          : wasOutage
            ? "Connection restored. Live data resumed."
            : "Gate ledger reads complete.";
      status.setAttribute("aria-live", outage && !wasOutage ? "assertive" : "polite");
      if (notice !== lastNotice) {
        status.textContent = notice;
        lastNotice = notice;
      }
      wasOutage = outage;
    } finally {
      busy = false;
    }
  };
  const stop = pollVisible(refresh);
  retry.addEventListener("click", () => {
    void refresh();
  });
  window.addEventListener("pagehide", stop, { once: true });
}

for (const root of document.querySelectorAll<HTMLElement>("[data-ledger]")) {
  const { gate, nearRpc, baseRpc } = root.dataset;
  if (!gate || !nearRpc || !baseRpc) continue;

  void initializeLedger(root, gate, nearRpc, baseRpc).catch(() => {
    const status = root.querySelector<HTMLOutputElement>("[data-ledger-status]");
    if (status)
      status.textContent =
        "Cannot load the chain readers. Reload this page to try again. No live data has been observed.";
  });
}
