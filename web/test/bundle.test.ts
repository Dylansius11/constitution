import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { fixtureBundle } from "../src/data/flow-sources.ts";
import { encodeSourceBundle, sourceCommitment } from "../src/lib/bundle.ts";
import { bytesToHex, hexToBytes } from "../src/lib/bytes.ts";

const vectors = JSON.parse(
  readFileSync(new URL("../../vectors/source-bundle.json", import.meta.url), "utf8"),
) as {
  vectors: {
    name: string;
    contract_name: string;
    foundry_profile: string;
    files: { path: string; content_hex: string }[];
    borsh_hex: string;
    source_commitment: string;
  }[];
};

describe("SourceBundle independent Borsh derivation", () => {
  for (const vector of vectors.vectors) {
    it(`matches Rust bytes and SHA-256 for ${vector.name}`, async () => {
      const bundle = {
        contract_name: vector.contract_name,
        foundry_profile: vector.foundry_profile,
        files: vector.files.map(
          (file) => [file.path, hexToBytes(file.content_hex, "source")] as const,
        ),
      };
      const bytes = encodeSourceBundle(bundle);
      expect(bytesToHex(bytes)).toBe(vector.borsh_hex);
      expect(await sourceCommitment(bytes)).toBe(vector.source_commitment);
      expect(
        bytesToHex(
          encodeSourceBundle(fixtureBundle(vector.name === "VaultV2Drain" ? "attack" : "honest")),
        ),
      ).toBe(vector.borsh_hex);
    });
  }

  it("refuses unsafe paths, duplicate paths, missing selected source and unsupported profiles", () => {
    const good = {
      contract_name: "src/A.sol:A",
      foundry_profile: "default",
      files: [["src/A.sol", new Uint8Array()]] as const,
    };
    for (const path of ["src/../A.sol", "src\\A.sol", "A.sol", "src//A.sol", "src/a.txt"]) {
      expect(() => encodeSourceBundle({ ...good, files: [[path, new Uint8Array()]] })).toThrow();
    }
    expect(() => encodeSourceBundle({ ...good, files: [...good.files, ...good.files] })).toThrow();
    expect(() => encodeSourceBundle({ ...good, contract_name: "src/B.sol:B" })).toThrow();
    expect(() => encodeSourceBundle({ ...good, foundry_profile: "test" })).toThrow();
    expect(() => encodeSourceBundle({ ...good, files: [] })).toThrow();
    expect(() =>
      encodeSourceBundle({ ...good, files: [["src/A.sol", new Uint8Array(2 * 1024 * 1024)]] }),
    ).toThrow();
    expect(() =>
      encodeSourceBundle({
        ...good,
        files: Array.from({ length: 129 }, (_, i) => [`src/${i}.sol`, new Uint8Array()] as const),
      }),
    ).toThrow();
  });
});
