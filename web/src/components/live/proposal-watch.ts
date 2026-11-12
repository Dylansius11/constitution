import { getAddress, keccak256 } from "viem";
import { createBaseClient, type ProxyReads, type TxObservation } from "../../lib/base.ts";
import { createGateClient, type View } from "../../lib/gate.ts";
import type { Config, Proposal } from "../../lib/gate-types.ts";
import {
  reduceSurface,
  type Source,
  type SurfaceInput,
  type SurfaceKind,
} from "../../lib/state.ts";
import { type SurfaceView, surfaceView } from "../../lib/surface-view.ts";
import { createVerifierClient, type VerifierRun } from "../../lib/verifier.ts";
import { markerPaths, renderLadder } from "./ladder.ts";
import { lastObserved, observe, pollVisible, replaceValue, text, timestamp } from "./runtime.ts";

function renderVerdictMarker(root: HTMLElement, tone: SurfaceView["tone"]): void {
  const marker = root.querySelector<SVGElement>(".live-verdict .marker");
  if (!marker) return;
  marker.classList.remove("marker--pending", "marker--done", "marker--refused", "marker--skipped");
  const state = (
    { pass: "done", refuse: "refused", pending: "pending", neutral: "skipped" } as const
  )[tone];
  marker.classList.add(`marker--${state}`);
  const shape = document.createElementNS(
    "http://www.w3.org/2000/svg",
    state === "pending" ? "circle" : "path",
  );
  if (state === "pending") {
    shape.setAttribute("cx", "8");
    shape.setAttribute("cy", "8");
    shape.setAttribute("r", "6");
  } else shape.setAttribute("d", markerPaths[state]);
  marker.replaceChildren(shape);
}

for (const root of document.querySelectorAll<HTMLElement>("[data-watch]")) {
  const { gate: gateAccount, nearRpc, baseRpc, verifier: verifierUrl } = root.dataset;
  const form = root.querySelector<HTMLFormElement>("[data-watch-form]");
  const input = form?.querySelector<HTMLInputElement>("input");
  const output = root.querySelector<HTMLOutputElement>("[data-watch-status]");
  const record = root.querySelector<HTMLElement>("[data-watch-record]");
  const fields = root.querySelector<HTMLElement>("[data-watch-fields]");
  const link = root.querySelector<HTMLAnchorElement>("[data-watch-explorer]");
  const explorerNote = root.querySelector<HTMLElement>("[data-watch-explorer-note]");
  const observedTime = root.querySelector<HTMLElement>("[data-watch-time]");
  const baseTime = root.querySelector<HTMLElement>("[data-watch-base-time]");
  if (
    !gateAccount ||
    !nearRpc ||
    !baseRpc ||
    !form ||
    !input ||
    !output ||
    !record ||
    !fields ||
    !link ||
    !explorerNote ||
    !observedTime ||
    !baseTime
  ) {
    continue;
  }

  const gate = createGateClient({ rpcUrl: nearRpc, gate: gateAccount });
  const base = createBaseClient({ rpcUrl: baseRpc });
  const verifier = verifierUrl ? createVerifierClient({ baseUrl: verifierUrl }) : null;
  let stop: (() => void) | undefined;
  let generation = 0;

  const watch = (id: number): void => {
    stop?.();
    const ownGeneration = ++generation;
    let proposal: Source<Proposal | null> = { status: "idle" };
    let run: Source<VerifierRun | null> = { status: "idle" };
    let evm: Source<TxObservation> = { status: "idle" };
    let proxy: Source<ProxyReads> = { status: "idle" };
    let config: Source<View<Config>> = { status: "idle" };
    let gateView: View<Proposal | null> | undefined;
    let previous: SurfaceKind | undefined;
    let lastAnnouncement = "";
    let lastAnnouncedAt = 0;
    let wasOutage = false;
    let observedTxHash: string | null = null;
    const runId =
      root.dataset["initialId"] === String(id) && root.dataset["runId"] !== undefined
        ? Number(root.dataset["runId"])
        : null;
    const canReadRun =
      verifier !== null && runId !== null && Number.isSafeInteger(runId) && runId >= 0;
    text(
      root,
      "[data-watch-run-note]",
      canReadRun
        ? `Reading the remembered verifier run ${runId}. Verifier-reported stages are not themselves a gate verdict.`
        : "Verifier stages: Not observed by this page. The verifier API needs a run id; it cannot look up a run by proposal id.",
    );
    output.textContent = `Watching proposal ${id}. Reading NEAR testnet.`;
    const waiting = surfaceView({ kind: "idle" });
    root.dataset["tone"] = "neutral";
    record.dataset["stale"] = "false";
    // The record stays hidden until a proposal is watched: an idle ladder of "Pending" rows says nothing.
    record.hidden = false;
    text(root, "[data-watch-title]", `Watching proposal ${id}`);
    text(
      root,
      "[data-watch-body]",
      "Reading the proposal from NEAR testnet. No outcome has been observed for this watch.",
    );
    renderLadder(root, waiting.stages);
    renderVerdictMarker(root, "neutral");
    fields.hidden = true;
    link.hidden = true;
    explorerNote.hidden = true;
    text(root, "[data-watch-time]", "");
    text(root, "[data-watch-base-time]", "");

    function field(key: string, value: string | null, kind?: "hash" | "address" | "near"): void {
      const target = root.querySelector<HTMLElement>(`[data-field="${key}"]`);
      if (!target) return;

      if (value !== null && kind)
        replaceValue(
          target,
          kind === "address" ? getAddress(value) : value,
          target.previousElementSibling?.textContent ?? key,
          kind,
        );
      else {
        delete target.dataset["value"];
        if (target.textContent !== (value ?? "Pending")) target.textContent = value ?? "Pending";
      }
    }
    const refresh = async (): Promise<void> => {
      if (ownGeneration !== generation) return;
      const jobs: Promise<void>[] = [];
      jobs.push(
        (async () => {
          proposal = await observe(proposal, async () => {
            const view = await gate.getProposal(id);
            gateView = view;
            return view.value;
          });
        })(),
      );
      if (config.status !== "ok")
        jobs.push(
          (async () => {
            config = await observe(config, () => gate.getConfig());
          })(),
        );
      if (canReadRun && verifier !== null && runId !== null)
        jobs.push(
          (async () => {
            run = await observe(run, () => verifier.getRun(runId));
          })(),
        );
      await Promise.all(jobs);
      if (ownGeneration !== generation) return;
      const p = lastObserved(proposal)?.value;
      const cfg = lastObserved(config)?.value.value;
      if (p && cfg) {
        const signedHash = p.evm?.signed_tx ? keccak256(p.evm.signed_tx as `0x${string}`) : null;
        if (signedHash !== observedTxHash) {
          evm = { status: "idle" };
          observedTxHash = signedHash;
        }
        await Promise.all([
          (async () => {
            proxy = await observe(proxy, async () => {
              if (cfg.chain_id !== 84532)
                throw new Error("Gate is not configured for Base Sepolia");
              const read = await base.readImplementation(cfg.proxy);
              const before = lastObserved(proxy)?.value.before;
              // Never use the proposal's expected-current commitment as an observed before read.
              return {
                ...(before ? { before } : p.state === "Pending" ? { before: read } : {}),
                after: read,
              };
            });
          })(),
          ...(signedHash
            ? [
                (async () => {
                  evm = await observe(evm, () => base.readTransaction(signedHash));
                })(),
              ]
            : []),
        ]);
      } else if (p && !cfg) {
        proxy = await observe(proxy, async () => {
          throw new Error("Cannot read NEAR testnet get_config; proxy address is not observed");
        });
      }
      if (ownGeneration !== generation) return;
      const observations: SurfaceInput = {
        submission: { phase: "none" },
        proposal,
        run,
        runId: canReadRun ? runId : null,
        evm,
        proxy,
      };
      const state = reduceSurface(observations, previous);
      const entered = previous !== state.kind;
      previous = state.kind;
      const observedRun = lastObserved(run)?.value;
      const view = surfaceView(state, {
        proposalId: id,
        verifierObserved: !!observedRun,
        ...(observedRun ? { runStages: observedRun.stages } : {}),
        ...(p ? { ruleSetHash: p.rule_set_hash } : {}),
      });
      root.dataset["tone"] = view.tone;
      const outage = state.kind === "network_unavailable";
      record.dataset["stale"] = String(outage);
      text(root, "[data-watch-title]", view.title);
      text(root, "[data-watch-body]", view.body);
      renderLadder(root, view.stages);
      renderVerdictMarker(root, view.tone);
      const last = lastObserved(proposal);
      if (last && gateView)
        observedTime.replaceChildren(
          timestamp(
            last.observedAtMs,
            proposal.status === "unavailable" ? "Last observed" : "Observed",
          ),
          ` · NEAR block ${gateView.blockHeight}`,
        );
      if (p) {
        fields.hidden = false;
        const notRequested = p.state === "Rejected";
        field("id", String(p.id));
        field("proposer", p.proposer, "near");
        field("target", cfg?.proxy ?? "Not observed", cfg ? "address" : undefined);
        field("new_impl", p.new_impl, "address");
        field("codehash", p.new_impl_codehash, "hash");
        field("source", p.source_commitment, "hash");
        field(
          "init",
          p.init_calldata === "0x" ? "Empty" : p.init_calldata,
          p.init_calldata === "0x" ? undefined : "hash",
        );
        field("expected", p.expected_current_impl, "address");
        field("rules", p.rule_set_hash, "hash");
        field(
          "ai",
          observedRun
            ? observedRun.stages
                .filter((s) => s.name === "ai_review" || s.name === "reproducing")
                .map((s) => `${s.name}: ${s.status}. ${s.detail}`)
                .join(" ") || "Pending"
            : "Not observed by this page",
        );
        field(
          "verdict",
          p.state === "Rejected"
            ? `Refused under R${p.rejection?.rule_id ?? "?"}`
            : p.certificate_hash
              ? "Certified"
              : "Pending",
        );
        field("certificate", p.certificate_hash, p.certificate_hash ? "hash" : undefined);
        field("evidence", p.rejection?.evidence_root ?? null, p.rejection ? "hash" : undefined);
        field("reason", p.rejection ? String(p.rejection.reason_code) : "Not observed");
        field(
          "near",
          "Transaction hashes are not returned by get_proposal. Not observed by this page.",
        );
        field("nonce", p.evm ? String(p.evm.nonce) : notRequested ? "Not requested" : "Pending");
        const txHash = p.evm?.signed_tx ? keccak256(p.evm.signed_tx as `0x${string}`) : null;
        field(
          "tx",
          txHash ?? (notRequested ? "Not requested" : "Pending"),
          txHash ? "hash" : undefined,
        );
        const tx = lastObserved(evm)?.value;
        field(
          "inclusion",
          tx
            ? tx.status === "included" || tx.status === "reverted"
              ? `${tx.status === "included" ? "Included, status 1" : "Reverted, status 0"} · block ${tx.block}`
              : tx.status === "pending"
                ? "Pending inclusion"
                : "Not found by this RPC"
            : notRequested
              ? "Not requested"
              : "Pending",
        );
        const reads = lastObserved(proxy);
        field(
          "before",
          reads?.value.before?.implementation ?? "Not observed before the action",
          reads?.value.before ? "address" : undefined,
        );
        field(
          "after",
          reads?.value.after?.implementation ?? "Pending",
          reads?.value.after ? "address" : undefined,
        );
        if (reads?.value.after)
          baseTime.replaceChildren(
            timestamp(
              reads.observedAtMs,
              proxy.status === "unavailable" ? "Last observed" : "Observed",
            ),
            ` · Implementation slot at Base Sepolia block ${reads.value.after.block}`,
          );
        link.hidden = !txHash || !tx || tx.status === "not_found";
        if (!link.hidden) link.href = `${root.dataset["explorer"]}/tx/${txHash}`;
        explorerNote.hidden = link.hidden;
      }
      const failures = [proposal, run, evm, proxy].filter((s) => s.status === "unavailable");
      const notice = outage
        ? `${view.title}. Not live. Retry with Watch; automatic reads continue while this tab is visible.`
        : failures.length
          ? `Cannot read ${failures.map((s) => (s.status === "unavailable" ? s.error : "")).join("; ")}. Last observed values are retained. Use Watch to retry.`
          : wasOutage
            ? "Connection restored. Live data resumed."
            : `${view.title}.${state.kind === "verifying" && state.stage ? ` ${view.stages.find((s) => s.state === "active")?.label ?? "Verifier"}, in progress.` : ""}`;
      output.setAttribute(
        "aria-live",
        (outage && !wasOutage) || (!outage && entered && view.live === "assertive")
          ? "assertive"
          : "polite",
      );
      if (notice !== lastAnnouncement && Date.now() - lastAnnouncedAt >= 2000) {
        output.textContent = notice;
        lastAnnouncement = notice;
        lastAnnouncedAt = Date.now();
      }
      wasOutage = outage;
    };
    stop = pollVisible(refresh, 2000);
  };
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const id = Number(input.value);
    if (!Number.isSafeInteger(id) || id < 0 || input.value.trim() === "") {
      output.textContent =
        "Enter a non-negative whole proposal id within the supported integer range.";
      return;
    }
    watch(id);
  });
  const initialId = root.dataset["initialId"];
  if (initialId !== undefined && Number.isSafeInteger(Number(initialId)) && Number(initialId) >= 0)
    watch(Number(initialId));
  window.addEventListener(
    "pagehide",
    () => {
      generation++;
      stop?.();
    },
    { once: true },
  );
}
