import { describe, expect, it } from "vitest";
import { cliTabs, localChecks } from "../src/data/commands.ts";
import { buildLlmsText } from "../src/data/llms.ts";

const tabLines = cliTabs.flatMap((tab) => tab.lines);

describe("check-it-yourself command tabs", () => {
  it("has the terminal UI first and the four read-only tabs after it, each with at least one command", () => {
    expect(cliTabs.map((tab) => tab.id)).toEqual(["tui", "audit", "status", "receipt", "appraise"]);
    for (const tab of cliTabs) {
      expect(tab.lines.some((line) => line.cmd)).toBe(true);
    }
  });

  it("uses the working RPC and never an unlisted network flag", () => {
    for (const line of tabLines) {
      if (line.cmd?.includes(" audit ")) expect(line.cmd).not.toContain("--network");
      if (line.cmd?.includes(" --near-rpc ")) expect(line.cmd).toContain("https://near-testnet.drpc.org");
    }
  });

  it("calls only read-only CLI subcommands and carries no unfinished or invented-host text", () => {
    for (const line of tabLines) {
      if (line.cmd) {
        expect(line.cmd).toMatch(
          /^constitution( (audit|status|receipt|verifier appraise) .+)?$/,
        );
      }
    }
    for (const line of [...tabLines, ...localChecks]) {
      expect(`${line.cmd ?? ""} ${line.note ?? ""}`).not.toMatch(/TODO|example\.com/i);
    }
  });
});

describe("agent guide", () => {
  it("states the recorded final deployment and its limits in the agent guide", () => {
    const text = buildLlmsText({ repoUrl: null });
    expect(text).toContain("## Status as of 2026-11-10");
    expect(text).toContain("constitution-gate.testnet");
    expect(text).toContain("0xe323d9b9c0169174e2b3895b8bdc41dc81e8e83c1e8cb62fcd51e8f708601da2");
    expect(text).toContain("E53_TIMELOCK_NOT_ELAPSED");
    expect(text).toContain("the final gate's only access key was deleted");
    expect(text).toContain("refused at rule 0 (current source runtime mismatch)");
    expect(text).toContain("appraisal passed");
    expect(text).toContain("Retired 2026-11-12");
    expect(text).toContain("https://near-testnet.drpc.org");
    expect(text).toContain("/docs");
    expect(text).not.toMatch(
      /not deployed|does not exist yet|awaiting activation|after activation/i,
    );
    expect(text).not.toMatch(/PUBLIC_DEMO|ed25519:/);
    expect(text).toContain("## How to verify");
    for (const line of [...tabLines, ...localChecks]) {
      if (line.cmd) expect(text).toContain(line.cmd);
    }
    expect(text).not.toMatch(/TODO|example\.com/i);
  });

  it("uses repository paths without a configured public repository", () => {
    const text = buildLlmsText({ repoUrl: null });
    expect(text).toContain("- crates/gate: NEAR gate");
    expect(text).toContain("docs/PRD.md");
    expect(text).not.toContain("/blob/dev/");
    expect(text).not.toContain("github.com");
  });

  it("links source paths to the dev branch of a configured repository", () => {
    const text = buildLlmsText({ repoUrl: "https://github.com/Dylansius11/constitution" });
    expect(text).toContain(
      "[crates/gate](https://github.com/Dylansius11/constitution/blob/dev/crates/gate)",
    );
    expect(text).toContain(
      "[docs/PRD.md](https://github.com/Dylansius11/constitution/blob/dev/docs/PRD.md)",
    );
  });
});
