import { createRunFlow, type FlowKind, type RunFlow, readFlowConfig } from "../../lib/run-flow.ts";
import { renderLadder } from "./ladder.ts";

/** States after which the flow stops observing; the card then offers a fresh run. */
const TERMINAL: Record<string, true> = {
  executed: true,
  rejected: true,
  expired: true,
  reverted: true,
  error: true,
};

const config = readFlowConfig(stringEnv(import.meta.env));
const nearExplorer = "https://testnet.nearblocks.io";

/** Only the string-valued build variables; Vite's boolean flags are not configuration. */
function stringEnv(env: object): Record<string, string> {
  return Object.fromEntries(
    Object.entries(env).filter((entry): entry is [string, string] => typeof entry[1] === "string"),
  );
}

function isFlowKind(value: string | undefined): value is FlowKind {
  return value === "attack" || value === "honest";
}

for (const card of document.querySelectorAll<HTMLElement>("[data-flow]")) {
  const kind = card.dataset["flow"];
  const button = card.querySelector<HTMLButtonElement>("[data-run]");
  const title = card.querySelector<HTMLElement>("[data-run-title]");
  const body = card.querySelector<HTMLElement>("[data-run-body]");
  const near = card.querySelector<HTMLAnchorElement>("[data-run-near]");
  if (!config || !button || !title || !body || !near || !isFlowKind(kind)) continue;

  const storageKey = `constitution:run:${config.gateAccount}:${kind}`;
  let flow: RunFlow | null = null;

  const begin = (): void => {
    flow?.dispose();
    button.setAttribute("aria-disabled", "true");
    button.textContent = "Running";
    flow = createRunFlow(kind, config, (update) => {
      renderLadder(card, update.view.stages);
      title.textContent = update.view.title;
      body.textContent = update.view.body;
      card.dataset["tone"] = update.view.tone;
      if (update.nearTxHash) {
        near.href = `${nearExplorer}/txns/${update.nearTxHash}`;
        near.hidden = false;
      }
      if (TERMINAL[update.state.kind]) {
        button.removeAttribute("aria-disabled");
        button.textContent = "Start a new run";
      }
    });
    flow.start().catch((error: unknown) => {
      title.textContent = "This run stopped";
      body.textContent = `${error instanceof Error ? error.message : String(error)} Nothing was retried.`;
      card.dataset["tone"] = "neutral";
      button.removeAttribute("aria-disabled");
      button.textContent = "Start a new run";
    });
  };

  button.addEventListener("click", () => {
    if (button.getAttribute("aria-disabled") === "true") return;
    // A new run is an explicit choice: it forgets the earlier run on this tab, never resubmits it.
    if (button.textContent === "Start a new run") sessionStorage.removeItem(storageKey);
    begin();
  });

  // A reload resumes observing a run this tab already submitted; it never submits again.
  if (sessionStorage.getItem(storageKey) !== null) begin();
}
