import { WireError } from "./errors.ts";

/**
 * JSON with lossless integers. NEAR serializes `u64` nanosecond timestamps as plain JSON numbers
 * around 1.7e18, which `JSON.parse` rounds above 2^53. Integers outside the safe range are
 * returned as `bigint`; every other number is a `number`. Objects have a null prototype and
 * duplicate keys are rejected, so a field can never be shadowed or smuggled.
 */
export type JsonValue =
  | null
  | boolean
  | number
  | bigint
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

const MAX_DEPTH = 64;
const NUMBER = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;
const INTEGER = /^-?(?:0|[1-9]\d*)$/;

class Parser {
  readonly #text: string;
  #at = 0;

  constructor(text: string) {
    this.#text = text;
  }

  parseDocument(): JsonValue {
    const value = this.#value(0);
    this.#space();
    if (this.#at !== this.#text.length) this.#fail("unexpected trailing characters");
    return value;
  }

  #fail(message: string): never {
    throw new WireError("json", `${message} at offset ${this.#at}`);
  }

  #space(): void {
    for (;;) {
      const c = this.#text.charCodeAt(this.#at);
      if (c === 0x20 || c === 0x0a || c === 0x0d || c === 0x09) this.#at += 1;
      else return;
    }
  }

  #value(depth: number): JsonValue {
    if (depth > MAX_DEPTH) this.#fail("nesting too deep");
    this.#space();
    const c = this.#text[this.#at];
    if (c === "{") return this.#object(depth);
    if (c === "[") return this.#array(depth);
    if (c === '"') return this.#string();
    for (const [word, value] of [
      ["true", true],
      ["false", false],
      ["null", null],
    ] as const) {
      if (this.#text.startsWith(word, this.#at)) {
        this.#at += word.length;
        return value;
      }
    }
    return this.#number();
  }

  #number(): JsonValue {
    NUMBER.lastIndex = this.#at;
    const match = NUMBER.exec(this.#text);
    if (match === null) return this.#fail("unexpected token");
    const literal = match[0];
    this.#at += literal.length;
    if (INTEGER.test(literal)) {
      const exact = BigInt(literal);
      return exact >= BigInt(Number.MIN_SAFE_INTEGER) && exact <= BigInt(Number.MAX_SAFE_INTEGER)
        ? Number(literal)
        : exact;
    }
    return Number(literal);
  }

  #string(): string {
    const start = this.#at;
    this.#at += 1;
    for (;;) {
      const c = this.#text[this.#at];
      if (c === undefined) return this.#fail("unterminated string");
      if (c === "\\") this.#at += 2;
      else if (c === '"') break;
      else this.#at += 1;
    }
    this.#at += 1;
    try {
      // The native parser decodes escapes and rejects control characters and bad surrogates.
      return JSON.parse(this.#text.slice(start, this.#at)) as string;
    } catch {
      return this.#fail("invalid string");
    }
  }

  #array(depth: number): JsonValue[] {
    this.#at += 1;
    const out: JsonValue[] = [];
    this.#space();
    if (this.#text[this.#at] === "]") {
      this.#at += 1;
      return out;
    }
    for (;;) {
      out.push(this.#value(depth + 1));
      this.#space();
      const c = this.#text[this.#at];
      this.#at += 1;
      if (c === "]") return out;
      if (c !== ",") return this.#fail("expected , or ]");
    }
  }

  #object(depth: number): { [key: string]: JsonValue } {
    this.#at += 1;
    const out: { [key: string]: JsonValue } = Object.create(null);
    this.#space();
    if (this.#text[this.#at] === "}") {
      this.#at += 1;
      return out;
    }
    for (;;) {
      this.#space();
      if (this.#text[this.#at] !== '"') return this.#fail("expected a string key");
      const key = this.#string();
      if (Object.hasOwn(out, key)) return this.#fail(`duplicate key ${JSON.stringify(key)}`);
      this.#space();
      if (this.#text[this.#at] !== ":") return this.#fail("expected :");
      this.#at += 1;
      out[key] = this.#value(depth + 1);
      this.#space();
      const c = this.#text[this.#at];
      this.#at += 1;
      if (c === "}") return out;
      if (c !== ",") return this.#fail("expected , or }");
    }
  }
}

export function parseJson(text: string): JsonValue {
  return new Parser(text).parseDocument();
}

/** Pretty JSON that writes `bigint` as an integer literal, the inverse of `parseJson`. */
export function stringifyJson(value: JsonValue, indent = 2): string {
  const pad = (level: number) => " ".repeat(indent * level);
  const write = (v: JsonValue, level: number): string => {
    if (v === null || typeof v === "boolean") return String(v);
    if (typeof v === "bigint") return v.toString();
    if (typeof v === "number") {
      if (!Number.isFinite(v)) throw new WireError("json", "cannot serialize a non-finite number");
      return String(v);
    }
    if (typeof v === "string") return JSON.stringify(v);
    if (Array.isArray(v)) {
      if (v.length === 0) return "[]";
      return `[\n${v.map((item) => pad(level + 1) + write(item, level + 1)).join(",\n")}\n${pad(level)}]`;
    }
    const keys = Object.keys(v);
    if (keys.length === 0) return "{}";
    const rows = keys.map(
      (k) => `${pad(level + 1)}${JSON.stringify(k)}: ${write(v[k] as JsonValue, level + 1)}`,
    );
    return `{\n${rows.join(",\n")}\n${pad(level)}}`;
  };
  return write(value, 0);
}
