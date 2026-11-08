import type { BaseClient } from "../../lib/base.ts";
import { replaceValue, text, timestamp } from "./runtime.ts";

for (const root of document.querySelectorAll<HTMLElement>("[data-spike]")) {
  const button = root.querySelector<HTMLButtonElement>("[data-check]");
  const result = root.querySelector<HTMLOutputElement>("[data-result]");
  const observations = root.querySelector<HTMLElement>("[data-observations]");
  const observedTime = root.querySelector<HTMLElement>("[data-observed-time]");
  const implementation = root.querySelector<HTMLElement>("[data-implementation]");
  const link = root.querySelector<HTMLAnchorElement>("[data-explorer-link]");
  const explorerNote = root.querySelector<HTMLElement>("[data-explorer-note]");
  const { rpc, tx, proxy: proxyAddress, expected, explorer } = root.dataset;

  if (
    !button ||
    !result ||
    !observations ||
    !observedTime ||
    !implementation ||
    !link ||
    !explorerNote ||
    !rpc ||
    !tx ||
    !proxyAddress ||
    !expected ||
    !explorer
  ) {
    continue;
  }

  let client: { base: BaseClient; checksum: (address: string) => string } | undefined;
  let busy = false;

  button.addEventListener("click", () => {
    if (busy) return;

    busy = true;
    button.setAttribute("aria-disabled", "true");
    button.textContent = "Checking";
    result.textContent = "Checking the transaction and proxy implementation on Base Sepolia.";

    void (async () => {
      try {
        if (client === undefined) {
          // Static imports would load the RPC/viem bundle before the visitor requests a re-check.
          const [{ createBaseClient }, { getAddress }] = await Promise.all([
            import("../../lib/base.ts"),
            import("viem"),
          ]);
          client = { base: createBaseClient({ rpcUrl: rpc }), checksum: getAddress };
        }

        const [transaction, proxy] = await Promise.all([
          client.base.readTransaction(tx),
          client.base.readImplementation(proxyAddress),
        ]);

        observedTime.replaceChildren(timestamp(Date.now()));
        text(
          root,
          "[data-inclusion]",
          transaction.status === "included"
            ? `Included, status 1 · block ${transaction.block}`
            : transaction.status === "reverted"
              ? `Reverted, status 0 · block ${transaction.block}`
              : transaction.status === "pending"
                ? "Pending inclusion"
                : "Transaction not found by this RPC",
        );
        replaceValue(
          implementation,
          client.checksum(proxy.implementation),
          "Current spike proxy implementation",
          "address",
        );
        text(
          root,
          "[data-slot-block]",
          `Implementation observed at Base Sepolia block ${proxy.block}.`,
        );
        text(
          root,
          "[data-comparison]",
          proxy.implementation.toLowerCase() === expected.toLowerCase()
            ? "Matches the recorded VaultV2Honest implementation."
            : "The spike proxy changed after the recording. This read does not say why.",
        );

        link.hidden = transaction.status === "not_found";
        if (!link.hidden) {
          link.href = `${explorer}/tx/${tx}`;
        }
        explorerNote.hidden = link.hidden;
        observations.hidden = false;
        delete observations.dataset["stale"];
        result.textContent =
          "Base Sepolia read complete. The recorded receipt above has not changed.";
        button.textContent = "Re-check on Base Sepolia";
      } catch {
        if (!observations.hidden) {
          observations.dataset["stale"] = "true";
          const time = observedTime.querySelector("time");
          if (time?.textContent) {
            time.textContent = time.textContent.replace(/^Observed /, "Last observed ");
          }
        }
        result.textContent =
          "Cannot reach Base Sepolia. Nothing here is live until the connection returns.";
        button.textContent = "Try again";
      } finally {
        busy = false;
        button.removeAttribute("aria-disabled");
      }
    })();
  });
}
