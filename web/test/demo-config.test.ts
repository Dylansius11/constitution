import { describe, expect, it } from "vitest";
import { changeSnippet } from "../src/lib/attack-snippet.ts";
import { attackBundle, type DemoInputs, resolveDemoState } from "../src/lib/demo-config.ts";
import { hex } from "./support/fixtures.ts";

const GATE = "constitution-demo.testnet";
const V1 = `// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
contract VaultV1 {
    mapping(address => uint256) public balances;
    function deposit() external payable { balances[msg.sender] += msg.value; }
}
`;
const V2 = `// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
contract VaultV2Drain {
    mapping(address => uint256) public balances;
    function deposit() external payable { balances[msg.sender] += msg.value; }
    function sweep() external { payable(msg.sender).transfer(address(this).balance); }
}
`;

const gallery = (over: Record<string, unknown> = {}) => ({
  version: 1,
  gate: GATE,
  proxy: hex(0x11, 20),
  attacks: [
    {
      id: "drain",
      title: "Drain",
      summary: "Sweeps the vault.",
      expected_rule: 2,
      expected_reason: 20,
      contract: "src/demo/VaultV2Drain.sol:VaultV2Drain",
      source_files: ["src/demo/VaultV2Drain.sol", "src/demo/VaultV1.sol"],
      impl: hex(0x4a, 20),
      codehash: hex(0xc0, 32),
      init_calldata: "0x",
      observed: null,
    },
  ],
  upgrades: [{ from: "VaultV1", to: "VaultV2Honest", observed: null }],
  ...over,
});

const inputs = (over: Partial<DemoInputs> = {}): DemoInputs => ({
  env: { PUBLIC_DEMO_GATE_ACCOUNT: GATE, PUBLIC_DEMO_VERIFIER_URL: "https://verifier.example/" },
  nearRpc: "https://rpc.testnet.example",
  baseRpc: "https://base.example",
  gallery: gallery(),
  deployment: null,
  sources: { "src/demo/VaultV1.sol": V1, "src/demo/VaultV2Drain.sol": V2 },
  ...over,
});

describe("resolveDemoState", () => {
  it("is ready only when env, gallery and every listed source agree", () => {
    const state = resolveDemoState(inputs());
    expect(state.status).toBe("ready");
    if (state.status !== "ready") return;
    expect(state.config.verifierUrl).toBe("https://verifier.example");
    expect(state.config.attacks[0]?.files.map((f) => f.path)).toEqual([
      "src/demo/VaultV1.sol",
      "src/demo/VaultV2Drain.sol",
    ]);
  });

  it("names every missing input and never invents a fallback", () => {
    const state = resolveDemoState(inputs({ env: {}, gallery: null }));
    expect(state).toEqual({
      status: "unavailable",
      missing: [
        "PUBLIC_DEMO_GATE_ACCOUNT",
        "PUBLIC_DEMO_VERIFIER_URL",
        "deployments/demo-gallery.json",
      ],
      invalid: null,
    });
  });

  it("treats blank env values as missing", () => {
    const state = resolveDemoState(
      inputs({ env: { PUBLIC_DEMO_GATE_ACCOUNT: "  ", PUBLIC_DEMO_VERIFIER_URL: "" } }),
    );
    expect(state.status === "unavailable" && state.missing).toContain("PUBLIC_DEMO_GATE_ACCOUNT");
  });

  it.each([
    ["a gallery for another gate", { gallery: gallery({ gate: "constitution-gate.testnet" }) }],
    ["a deployment for another gate", { deployment: { gate: "constitution-gate.testnet" } }],
    ["a listed source that is absent", { sources: { "src/demo/VaultV1.sol": V1 } }],
    ["a malformed gallery", { gallery: { version: 2 } }],
    [
      "a verifier URL with credentials",
      {
        env: {
          PUBLIC_DEMO_GATE_ACCOUNT: GATE,
          PUBLIC_DEMO_VERIFIER_URL: "https://user:pw@verifier.example",
        },
      },
    ],
  ] as const)("refuses %s with a reason instead of rendering a live page", (_name, over) => {
    const state = resolveDemoState(inputs(over as Partial<DemoInputs>));
    expect(state.status).toBe("unavailable");
    if (state.status === "unavailable") {
      expect(state.missing).toEqual([]);
      expect(state.invalid).toEqual(expect.any(String));
    }
  });

  it("builds the bundle the CLI would: sorted paths, the gallery contract, default profile", () => {
    const state = resolveDemoState(inputs());
    if (state.status !== "ready") throw new Error("expected ready");
    const bundle = attackBundle(state.config.attacks[0]!);
    expect(bundle.contract_name).toBe("src/demo/VaultV2Drain.sol:VaultV2Drain");
    expect(bundle.foundry_profile).toBe("default");
    expect(bundle.files.map(([path]) => path)).toEqual([
      "src/demo/VaultV1.sol",
      "src/demo/VaultV2Drain.sol",
    ]);
  });
});

describe("changeSnippet", () => {
  const files = [
    { path: "src/demo/VaultV1.sol", content: V1 },
    { path: "src/demo/VaultV2Drain.sol", content: V2 },
  ];

  it("shows only lines the attack adds to the current implementation", () => {
    const lines = changeSnippet(files, "src/demo/VaultV2Drain.sol:VaultV2Drain");
    expect(lines.map((l) => l.trim())).toEqual([
      "contract VaultV2Drain {",
      "function sweep() external { payable(msg.sender).transfer(address(this).balance); }",
    ]);
  });

  it("elides past the limit rather than growing the card", () => {
    const long = {
      path: "src/demo/Long.sol",
      content: Array.from({ length: 30 }, (_, i) => `uint256 a${i};`).join("\n"),
    };
    const lines = changeSnippet([...files, long], "src/demo/Long.sol:Long", 5);
    expect(lines).toHaveLength(6);
    expect(lines.at(-1)).toBe("…");
  });

  it("returns nothing when the named file is not in the bundle", () => {
    expect(changeSnippet(files, "src/demo/Other.sol:Other")).toEqual([]);
  });
});
