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

  it("keeps the dated activation status and checkout verification in the agent guide", () => {
    const text = buildLlmsText({ repoUrl: null });
    expect(text).toContain("## Status as of 2026-11-07");
    expect(text).toContain("Production gate: not deployed.");
    expect(text).toContain("CVM verifier: not running");
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
