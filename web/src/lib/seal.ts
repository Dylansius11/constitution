import sodium from "libsodium-wrappers";
import { hexToBytes } from "./bytes.ts";
import { InputError } from "./errors.ts";

/** Plaintext limit of one bundle (constitution_types::bundle::MAX_BUNDLE, ARCHITECTURE). */
export const MAX_BUNDLE_BYTES = 2 * 1024 * 1024;

/** A libsodium sealed box adds an ephemeral public key (32) and a Poly1305 tag (16). */
export const SEAL_OVERHEAD_BYTES = 48;

/**
 * Seal canonical bundle bytes to the verifier's X25519 transport key with libsodium
 * `crypto_box_seal`. Interoperates with Rust `crypto_box` 0.9.1 `seal`/`unseal` (T-ENC-3).
 * `transportPubkeyHex` is `transport_pubkey` from `GET /v1/identity`, 32 bytes of lowercase `0x` hex.
 */
export async function sealBundle(
  bytes: Uint8Array,
  transportPubkeyHex: string,
): Promise<Uint8Array> {
  if (bytes.length > MAX_BUNDLE_BYTES) {
    throw new InputError(
      `bundle is ${bytes.length} bytes, above the ${MAX_BUNDLE_BYTES} byte limit`,
    );
  }
  const publicKey = hexToBytes(transportPubkeyHex, "transport_pubkey", 32);
  await sodium.ready;
  return sodium.crypto_box_seal(bytes, publicKey);
}
