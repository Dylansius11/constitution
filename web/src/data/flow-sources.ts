import executor from "../../../contracts/src/ConstitutionExecutor.sol?raw";
import token from "../../../contracts/src/demo/MockToken.sol?raw";
import vault from "../../../contracts/src/demo/VaultV1.sol?raw";
import drain from "../../../contracts/src/demo/VaultV2Drain.sol?raw";
import honest from "../../../contracts/src/demo/VaultV2Honest.sol?raw";
import type { SourceBundle } from "../lib/bundle.ts";

/** Same sorted contracts/src tree as CLI `--bundle contracts`; never shown as live evidence. */
export function fixtureBundle(kind: "attack" | "honest"): SourceBundle {
  const name = kind === "attack" ? "VaultV2Drain" : "VaultV2Honest";
  const encoder = new TextEncoder();
  return {
    contract_name: `src/demo/${name}.sol:${name}`,
    foundry_profile: "default",
    files: [
      ["src/ConstitutionExecutor.sol", encoder.encode(executor)],
      ["src/demo/MockToken.sol", encoder.encode(token)],
      ["src/demo/VaultV1.sol", encoder.encode(vault)],
      ["src/demo/VaultV2Drain.sol", encoder.encode(drain)],
      ["src/demo/VaultV2Honest.sol", encoder.encode(honest)],
    ],
  };
}
