import { remaining } from "../../lib/display.ts";
import type { View } from "../../lib/gate.ts";
import type { PendingChange } from "../../lib/gate-types.ts";
import type { Source } from "../../lib/state.ts";
import { hashValue, lastObserved, observe, pollVisible, timestamp } from "./runtime.ts";

async function initializeQueue(root: HTMLElement, account: string, nearRpc: string): Promise<void> {
  const list = root.querySelector<HTMLOListElement>("[data-changes]");
  const output = root.querySelector<HTMLOutputElement>("[data-queue-status]");
  const observed = root.querySelector<HTMLElement>("[data-queue-observed]");
  const retry = root.querySelector<HTMLButtonElement>("[data-queue-retry]");
  if (!list || !output || !observed || !retry) return;

  // This reader must not download at all until the root has an activated gate account.
  const { createGateClient } = await import("../../lib/gate.ts");
  const client = createGateClient({ rpcUrl: nearRpc, gate: account });
  let source: Source<View<PendingChange[]>> = { status: "idle" };
  let fingerprint = "";
  let busy = false;
  let stale = false;
  let previousNotice = "";

  const updateRemaining = (): void => {
    if (document.hidden) return;
    for (const row of list.querySelectorAll<HTMLElement>("[data-eta-ms]")) {
      const eta = Number(row.dataset["etaMs"]);
      if (eta <= Date.now()) {
        if (row.dataset["matured"] !== "true") {
          row.replaceChildren(timestamp(eta, "Applicable since"), ". Anyone may apply it.");
          row.dataset["matured"] = "true";
        }
      } else {
        row.textContent = `Remaining: ${remaining(eta - Date.now())}`;
        delete row.dataset["matured"];
      }
    }
  };
  const render = (changes: PendingChange[]): void => {
    const signature = JSON.stringify(changes, (_, v: unknown) =>
      typeof v === "bigint" ? v.toString() : v,
    );
    if (signature === fingerprint) {
      updateRemaining();
      return;
    }
    fingerprint = signature;
    list.replaceChildren();
    for (const change of [...changes].sort((a, b) => a.id - b.id)) {
      const item = document.createElement("li");
      const title = document.createElement("h4");
      title.textContent = `${change.kind.kind} · change ${change.id}`;
      const data = document.createElement("dl");
      const add = (label: string, value: Node | string): void => {
        const row = document.createElement("div");
        row.className = "live-row";
        const dt = document.createElement("dt");
        dt.textContent = label;
        const dd = document.createElement("dd");
        dd.append(value);
        row.append(dt, dd);
        data.append(row);
      };
      add("Scheduled by", hashValue(change.scheduled_by, "Change scheduler", "near"));
      add("Scheduled", timestamp(Number(change.scheduled_at_ns / 1_000_000n), "Scheduled"));
      const eta = Number(change.eta_ns / 1_000_000n);
      const applicable = timestamp(eta, "Applicable from");
      add("Applicable from", applicable);
      const status = document.createElement("p");
      status.className = "change-status";
      if (change.status === "Pending") {
        status.dataset["etaMs"] = String(eta);
      } else {
        status.textContent =
          change.status === "Applied"
            ? "Applied — observed on the gate. The view does not provide an application time or transaction receipt."
            : "Cancelled — observed on the gate. The view does not provide a cancellation time or actor.";
      }
      item.append(title, data, status);
      list.append(item);
    }
    updateRemaining();
  };
  const refresh = async (): Promise<void> => {
    if (busy || document.hidden) return;
    busy = true;
    try {
      source = await observe(source, () => client.listChanges(0, 50));
      const last = lastObserved(source);
      const wasStale = stale;
      stale = source.status === "unavailable" && source.failures >= 3;
      list.dataset["stale"] = String(stale);
      if (last) {
        render(last.value.value);
        observed.replaceChildren(
          timestamp(
            last.observedAtMs,
            source.status === "unavailable" ? "Last observed" : "Observed",
          ),
          ` · NEAR block ${last.value.blockHeight}`,
        );
      }
      const message =
        source.status === "unavailable"
          ? `${stale ? "Not live. " : ""}Cannot read NEAR testnet list_changes. ${last ? "Showing the last observed queue. " : ""}Nothing here is live until the connection returns.`
          : last?.value.value.length === 0
            ? "No change queued. Rules, the verifier allowlist and authority change only after the public governance delay."
            : wasStale
              ? "Connection restored. Live queue data resumed."
              : "Timelock queue observed on NEAR testnet.";
      output.setAttribute("aria-live", stale && !wasStale ? "assertive" : "polite");
      if (previousNotice !== message) {
        output.textContent = message;
        previousNotice = message;
      }
    } finally {
      busy = false;
    }
  };
  const stop = pollVisible(refresh);
  const minute = window.setInterval(updateRemaining, 60_000);
  retry.addEventListener("click", () => {
    void refresh();
  });
  window.addEventListener(
    "pagehide",
    () => {
      stop();
      window.clearInterval(minute);
    },
    { once: true },
  );
}

for (const root of document.querySelectorAll<HTMLElement>("[data-timelock]")) {
  const { gate, nearRpc } = root.dataset;
  if (!gate || !nearRpc) continue;

  void initializeQueue(root, gate, nearRpc).catch(() => {
    const output = root.querySelector<HTMLOutputElement>("[data-queue-status]");
    if (output)
      output.textContent =
        "Cannot load the NEAR reader. Reload this page to try again. No live queue has been observed.";
  });
}
