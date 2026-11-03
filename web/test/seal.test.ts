import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import sodium from "libsodium-wrappers";
import { beforeAll, describe, expect, it } from "vitest";
import { base64ToBytes, bytesToBase64, bytesToHex, hexToBytes } from "../src/lib/bytes.ts";
import { InputError, WireError } from "../src/lib/errors.ts";
import { MAX_BUNDLE_BYTES, SEAL_OVERHEAD_BYTES, sealBundle } from "../src/lib/seal.ts";

/**
 * T-ENC-3: sealed-box interoperability with Rust `crypto_box` 0.9.1. All keys come from
 * vectors/sealed-bundle.json and are TEST-ONLY (derived from public labels in the Rust test).
 */
const VECTOR_FILE = new URL("../../vectors/sealed-bundle.json", import.meta.url);
const FRESH_FILE = new URL("../../scratch/page-vectors/js-fresh-seal.json", import.meta.url);

interface KeyPair {
  secret_key: string;
  public_key: string;
}
interface SealedVectors {
  plaintext_hex: string;
  rust_recipient: KeyPair;
  js_recipient: KeyPair;
  rust_to_js: { sealed_b64: string };
  js_to_rust: { sealed_b64: string } | null;
}

function readVectors(): SealedVectors {
  return JSON.parse(readFileSync(VECTOR_FILE, "utf8")) as SealedVectors;
}

function keys(pair: KeyPair): { publicKey: Uint8Array; secretKey: Uint8Array } {
  return {
    publicKey: hexToBytes(pair.public_key, "public_key", 32),
    secretKey: hexToBytes(pair.secret_key, "secret_key", 32),
  };
}

/** Flip one bit at each structurally distinct position; truncate; use the wrong recipient. */
function expectRejectsTampering(sealed: Uint8Array, recipient: KeyPair): void {
  const { publicKey, secretKey } = keys(recipient);
  for (const position of [0, 31, 32, 47, 48, sealed.length - 1]) {
    const tampered = sealed.slice();
    tampered[position] = (tampered[position] ?? 0) ^ 0x01;
    expect(
      () => sodium.crypto_box_seal_open(tampered, publicKey, secretKey),
      `bit flip at ${position}`,
    ).toThrow();
  }
  for (const length of [0, 31, 32, 47, sealed.length - 1]) {
    expect(
      () => sodium.crypto_box_seal_open(sealed.subarray(0, length), publicKey, secretKey),
      `truncated to ${length}`,
    ).toThrow();
  }
  const stranger = sodium.crypto_box_keypair();
  expect(() =>
    sodium.crypto_box_seal_open(sealed, stranger.publicKey, stranger.privateKey),
  ).toThrow();
}

beforeAll(async () => {
  await sodium.ready;
});

describe("T-ENC-3 sealed bundle interoperability", () => {
  it("the test keys really are an X25519 pair (public = base point times secret)", () => {
    for (const pair of [readVectors().rust_recipient, readVectors().js_recipient]) {
      const { publicKey, secretKey } = keys(pair);
      expect(sodium.crypto_scalarmult_base(secretKey)).toEqual(publicKey);
    }
  });

  it("libsodium opens the ciphertext Rust sealed, byte for byte", () => {
    const v = readVectors();
    const { publicKey, secretKey } = keys(v.js_recipient);
    const sealed = base64ToBytes(v.rust_to_js.sealed_b64, "rust_to_js");
    const opened = sodium.crypto_box_seal_open(sealed, publicKey, secretKey);
    expect(bytesToHex(opened)).toBe(v.plaintext_hex);
    expect(sealed.length).toBe(opened.length + SEAL_OVERHEAD_BYTES);
  });

  it("rejects tampered, truncated and misdirected Rust ciphertext", () => {
    const v = readVectors();
    expectRejectsTampering(base64ToBytes(v.rust_to_js.sealed_b64, "rust_to_js"), v.js_recipient);
  });

  it("sealBundle seals to the Rust recipient key and libsodium can open it", async () => {
    const v = readVectors();
    const plaintext = hexToBytes(v.plaintext_hex, "plaintext_hex");
    const sealed = await sealBundle(plaintext, v.rust_recipient.public_key);
    const { publicKey, secretKey } = keys(v.rust_recipient);
    expect(sealed.length).toBe(plaintext.length + SEAL_OVERHEAD_BYTES);
    expect(sodium.crypto_box_seal_open(sealed, publicKey, secretKey)).toEqual(plaintext);
    expectRejectsTampering(sealed, v.rust_recipient);
    // Sealed boxes use a fresh ephemeral key: sealing twice never repeats.
    expect(bytesToBase64(await sealBundle(plaintext, v.rust_recipient.public_key))).not.toBe(
      bytesToBase64(sealed),
    );

    // Hand the fresh ciphertext to the Rust test (cargo test -p constitution-types --test page_vectors).
    mkdirSync(new URL("./", FRESH_FILE), { recursive: true });
    writeFileSync(
      FRESH_FILE,
      `${JSON.stringify(
        {
          label:
            "Fresh libsodium seal written by web/test/seal.test.ts for the Rust test. TEST-ONLY key.",
          recipient_public_key: v.rust_recipient.public_key,
          plaintext_hex: v.plaintext_hex,
          sealed_b64: bytesToBase64(sealed),
        },
        null,
        2,
      )}\n`,
    );

    if (process.env["CONSTITUTION_WRITE_VECTORS"] === "1") {
      writeFileSync(
        VECTOR_FILE,
        `${JSON.stringify({ ...v, js_to_rust: { sealed_b64: bytesToBase64(sealed) } }, null, 2)}\n`,
      );
    }
  });

  it("the committed libsodium ciphertext opens with the Rust recipient key", () => {
    const v = readVectors();
    if (v.js_to_rust === null)
      throw new Error("js_to_rust is empty: run with CONSTITUTION_WRITE_VECTORS=1 once");
    const { publicKey, secretKey } = keys(v.rust_recipient);
    const sealed = base64ToBytes(v.js_to_rust.sealed_b64, "js_to_rust");
    expect(bytesToHex(sodium.crypto_box_seal_open(sealed, publicKey, secretKey))).toBe(
      v.plaintext_hex,
    );
    expectRejectsTampering(sealed, v.rust_recipient);
  });

  it("refuses a malformed transport key before sealing anything", async () => {
    const data = Uint8Array.of(1, 2, 3);
    const good = readVectors().rust_recipient.public_key;
    for (const bad of [
      good.toUpperCase(),
      good.slice(2),
      good.slice(0, -2),
      `${good}00`,
      "",
      "0x",
    ]) {
      await expect(sealBundle(data, bad), bad).rejects.toThrow(WireError);
    }
  });

  it("refuses a bundle above the 2 MiB plaintext limit and accepts exactly the limit", async () => {
    const key = readVectors().rust_recipient.public_key;
    await expect(sealBundle(new Uint8Array(MAX_BUNDLE_BYTES + 1), key)).rejects.toThrow(InputError);
    const sealed = await sealBundle(new Uint8Array(MAX_BUNDLE_BYTES), key);
    expect(sealed.length).toBe(MAX_BUNDLE_BYTES + SEAL_OVERHEAD_BYTES);
  });

  it("refuses to seal to the all-zero low-order point", async () => {
    await expect(sealBundle(Uint8Array.of(1), bytesToHex(new Uint8Array(32)))).rejects.toThrow();
  });
});
