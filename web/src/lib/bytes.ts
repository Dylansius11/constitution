import { WireError } from "./errors.ts";

const HEX_PATTERN = /^0x(?:[0-9a-f]{2})*$/;
const BASE64_PATTERN = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

/** Lowercase `0x` hex, the only form the gate and verifier emit and accept. */
export function bytesToHex(bytes: Uint8Array): string {
  let out = "0x";
  for (const byte of bytes) out += byte.toString(16).padStart(2, "0");
  return out;
}

/** Strict decode: lowercase, `0x` prefix, whole bytes. `length` pins the byte count. */
export function hexToBytes(text: string, label: string, length?: number): Uint8Array {
  if (!HEX_PATTERN.test(text))
    throw new WireError(label, "expected lowercase 0x-prefixed hex bytes");
  const bytes = new Uint8Array((text.length - 2) / 2);
  for (let i = 0; i < bytes.length; i += 1) {
    bytes[i] = Number.parseInt(text.slice(2 + 2 * i, 4 + 2 * i), 16);
  }
  if (length !== undefined && bytes.length !== length) {
    throw new WireError(label, `expected ${length} bytes, got ${bytes.length}`);
  }
  return bytes;
}

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

/** Strict standard padded base64 with canonical trailing bits, like Rust `base64::STANDARD`. */
export function base64ToBytes(text: string, label: string, length?: number): Uint8Array {
  if (!BASE64_PATTERN.test(text)) throw new WireError(label, "expected standard padded base64");
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  if (bytesToBase64(bytes) !== text) throw new WireError(label, "base64 has non-canonical bits");
  if (length !== undefined && bytes.length !== length) {
    throw new WireError(label, `expected ${length} bytes, got ${bytes.length}`);
  }
  return bytes;
}

/** SHA-256 through WebCrypto. */
export async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", new Uint8Array(bytes)));
}

export function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
}
