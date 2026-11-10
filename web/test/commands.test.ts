import { describe, expect, it } from "vitest";
import { agentCommands, humanCommands } from "../src/data/commands.ts";
import { buildLlmsText } from "../src/data/llms.ts";

describe("judge command guide", () => {
  it("offers runnable programs without unfinished or invented-host instructions", () => {
    const commands = humanCommands.flatMap((line) => (line.cmd ? [line.cmd] : []));
    expect(commands.length).toBeGreaterThan(0);
    for (const command of commands) {
      expect(command).toMatch(/^(cargo|forge|pnpm|constitution|make)\s/);
    }
    for (const line of [...humanCommands, ...agentCommands]) {
      expect(`${line.cmd ?? ""} ${line.note ?? ""}`).not.toMatch(/TODO|example\.com/i);
    }
  });

  it("states the recorded final deployment and its limits in the agent guide", () => {
    const text = buildLlmsText({ repoUrl: null });
    expect(text).toContain("## Status as of 2026-11-10");
    expect(text).toContain("constitution-gate.testnet");
    expect(text).toContain("0xe323d9b9c0169174e2b3895b8bdc41dc81e8e83c1e8cb62fcd51e8f708601da2");
    expect(text).toContain("E53_TIMELOCK_NOT_ELAPSED");
    expect(text).toContain("the gate still held one full-access key");
    expect(text).toContain("refused at rule 0 (current source runtime mismatch)");
    expect(text).toContain("appraisal passed");
    expect(text).not.toMatch(/not deployed|does not exist yet|awaiting activation|after activation/i);
    expect(text).not.toMatch(/PUBLIC_DEMO|ed25519:/);
    expect(text).toContain("## How to verify");
    for (const line of humanCommands) {
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
