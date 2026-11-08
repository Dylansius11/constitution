import { createRunFlow, type FlowKind, type RunFlow, readFlowConfig } from "../../lib/run-flow.ts";
import { renderLadder } from "./ladder.ts";

/** Only observed terminal outcomes permit forgetting recovery and starting another proposal. */
const TERMINAL: Record<string, true> = {
  executed: true,
  rejected: true,
  expired: true,
  reverted: true,
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
  let canStartNew = false;

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
      canStartNew = TERMINAL[update.state.kind] === true;
      if (canStartNew || update.state.kind === "error") {
        button.removeAttribute("aria-disabled");
        button.textContent = canStartNew ? "Start a new run" : "Check again";
      }
    });
    flow.start().catch((error: unknown) => {
      canStartNew = false;
      title.textContent = "This run stopped";
      body.textContent = `${error instanceof Error ? error.message : String(error)} Nothing was retried. Inspect the demo account with the CLI before submitting again.`;
      card.dataset["tone"] = "neutral";
      button.removeAttribute("aria-disabled");
      button.textContent = "Check again";
    });
  };

  button.addEventListener("click", () => {
    if (button.getAttribute("aria-disabled") === "true") return;
    // Errors retain accepted/uncertain submission metadata; another observation never reproposes.
    if (canStartNew) sessionStorage.removeItem(storageKey);
    begin();
  });

  // A reload resumes observing a run this tab already submitted; it never submits again.
  if (sessionStorage.getItem(storageKey) !== null) begin();
}
