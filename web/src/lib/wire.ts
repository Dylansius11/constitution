import { WireError } from "./errors.ts";

/**
 * Boundary validators for untrusted JSON. Every parser in this library reads unknown input only
 * through these, so a malformed field produces a `WireError` naming its path, never a silent
 * default. Extra fields are ignored: the servers may add fields without breaking the page.
 */

const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;
const DECIMAL = /^(?:0|[1-9]\d*)$/;
const ACCOUNT_ID = /^(?:(?:[a-z\d]+[-_])*[a-z\d]+\.)*(?:[a-z\d]+[-_])*[a-z\d]+$/;

export function asString(value: unknown, path: string): string {
  if (typeof value !== "string") throw new WireError(path, "expected a string");
  return value;
}

export function asBoolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new WireError(path, "expected a boolean");
  return value;
}

/** An unsigned integer that fits a JS number exactly. For counters, ids and chain ids. */
export function asSafeUint(value: unknown, path: string, max = Number.MAX_SAFE_INTEGER): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > max) {
    throw new WireError(path, `expected an unsigned integer up to ${max}`);
  }
  return value;
}

/** A u64 that may exceed 2^53 (NEAR nanosecond timestamps). Accepts `number` or `bigint`. */
export function asU64(value: unknown, path: string): bigint {
  if (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) return BigInt(value);
  if (typeof value === "bigint" && value >= 0n && value <= U64_MAX) return value;
  throw new WireError(path, "expected an unsigned 64-bit integer");
}

/** A u128 carried as a decimal JSON string, the gate's wire form for amounts. */
export function asU128String(value: unknown, path: string): bigint {
  if (typeof value !== "string" || !DECIMAL.test(value)) {
    throw new WireError(path, "expected a decimal string");
  }
  const parsed = BigInt(value);
  if (parsed > U128_MAX) throw new WireError(path, "exceeds 128 bits");
  return parsed;
}

/** Lowercase `0x` hex of exactly `bytes` bytes. */
export function asHexFixed(value: unknown, bytes: number, path: string): string {
  if (typeof value !== "string" || value.length !== 2 + 2 * bytes || !/^0x[0-9a-f]+$/.test(value)) {
    throw new WireError(path, `expected lowercase 0x hex of ${bytes} bytes`);
  }
  return value;
}

/** Lowercase `0x` hex of any whole number of bytes, including none. */
export function asHexBytes(value: unknown, path: string): string {
  if (typeof value !== "string" || !/^0x(?:[0-9a-f]{2})*$/.test(value)) {
    throw new WireError(path, "expected lowercase 0x hex bytes");
  }
  return value;
}

export function asAccountId(value: unknown, path: string): string {
  if (
    typeof value !== "string" ||
    value.length < 2 ||
    value.length > 64 ||
    !ACCOUNT_ID.test(value)
  ) {
    throw new WireError(path, "expected a NEAR account id");
  }
  return value;
}

export function asOneOf<T extends string>(value: unknown, allowed: readonly T[], path: string): T {
  const found = allowed.find((candidate) => candidate === value);
  if (found === undefined) throw new WireError(path, `expected one of ${allowed.join(", ")}`);
  return found;
}

export function asArray(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) throw new WireError(path, "expected an array");
  return value;
}

export function isAccountId(value: string): boolean {
  return value.length >= 2 && value.length <= 64 && ACCOUNT_ID.test(value);
}

/** Typed view over an untrusted JSON object. Reads fail loudly with the full field path. */
export class Obj {
  readonly path: string;
  readonly #record: Record<string, unknown>;

  constructor(value: unknown, path: string) {
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      throw new WireError(path, "expected an object");
    }
    this.path = path;
    this.#record = value as Record<string, unknown>;
  }

  has(key: string): boolean {
    return Object.hasOwn(this.#record, key);
  }

  keys(): string[] {
    return Object.keys(this.#record);
  }

  at(key: string): string {
    return `${this.path}.${key}`;
  }

  raw(key: string): unknown {
    if (!this.has(key)) throw new WireError(this.at(key), "missing field");
    return this.#record[key];
  }

  /** Optional field: absent or `null` yields `undefined`. */
  maybe<T>(key: string, parse: (value: unknown, path: string) => T): T | undefined {
    if (!this.has(key) || this.#record[key] === null) return undefined;
    return parse(this.#record[key], this.at(key));
  }

  /** Nullable field that must be present: `null` yields `null`. */
  nullable<T>(key: string, parse: (value: unknown, path: string) => T): T | null {
    const value = this.raw(key);
    return value === null ? null : parse(value, this.at(key));
  }

  str(key: string): string {
    return asString(this.raw(key), this.at(key));
  }

  bool(key: string): boolean {
    return asBoolean(this.raw(key), this.at(key));
  }

  safe(key: string, max?: number): number {
    return asSafeUint(this.raw(key), this.at(key), max);
  }

  u64(key: string): bigint {
    return asU64(this.raw(key), this.at(key));
  }

  u128(key: string): bigint {
    return asU128String(this.raw(key), this.at(key));
  }

  hex(key: string, bytes: number): string {
    return asHexFixed(this.raw(key), bytes, this.at(key));
  }

  hexBytes(key: string): string {
    return asHexBytes(this.raw(key), this.at(key));
  }

  account(key: string): string {
    return asAccountId(this.raw(key), this.at(key));
  }

  oneOf<T extends string>(key: string, allowed: readonly T[]): T {
    return asOneOf(this.raw(key), allowed, this.at(key));
  }

  obj(key: string): Obj {
    return new Obj(this.raw(key), this.at(key));
  }

  arr(key: string): unknown[] {
    return asArray(this.raw(key), this.at(key));
  }
}
