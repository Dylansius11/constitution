import { describe, expect, it } from "vitest";
import { base64ToBytes, bytesToBase64, bytesToHex, hexToBytes } from "../src/lib/bytes.ts";
import { WireError } from "../src/lib/errors.ts";
import { type JsonValue, parseJson, stringifyJson } from "../src/lib/json.ts";

describe("lossless JSON", () => {
  it("keeps integers above 2^53 exactly, as bigint", () => {
    const parsed = parseJson(
      '{"ns":1793700000123456789,"small":42,"max":18446744073709551615,"neg":-9007199254740993}',
    );
    expect(parsed).toEqual({
      ns: 1_793_700_000_123_456_789n,
      small: 42,
      max: 18_446_744_073_709_551_615n,
      neg: -9_007_199_254_740_993n,
    });
    // The native parser would have rounded it.
    expect(String(JSON.parse("1793700000123456789"))).not.toBe("1793700000123456789");
  });

  it("returns safe integers and fractions as numbers", () => {
    expect(parseJson("[0,-0,9007199254740991,1.5,2e3,-1.25E-2]")).toEqual([
      0, -0, 9_007_199_254_740_991, 1.5, 2000, -0.0125,
    ]);
  });

  it("parses strings with escapes and unicode", () => {
    expect(parseJson('"a\\n\\u00e9\\ud83d\\ude00\\""')).toBe('a\né😀"');
  });

  it("parses nested structures and literals", () => {
    expect(parseJson(' {"a":[true,false,null,{"b":[]}],"c":{}} ')).toEqual({
      a: [true, false, null, { b: [] }],
      c: {},
    });
  });

  it("rejects malformed documents", () => {
    for (const bad of [
      "",
      "{",
      "[1,]",
      '{"a":1,}',
      "01",
      "1.",
      "+1",
      '{"a" 1}',
      "'x'",
      "nul",
      "[1] 2",
      '"\\x"',
      '"line\nbreak"',
      "NaN",
      "{a:1}",
    ]) {
      expect(() => parseJson(bad), bad).toThrow(WireError);
    }
  });

  it("rejects duplicate keys instead of letting the last one win", () => {
    expect(() => parseJson('{"a":1,"a":2}')).toThrow(/duplicate key/);
  });

  it("does not let __proto__ keys touch prototypes", () => {
    const parsed = parseJson('{"__proto__":{"polluted":true}}') as Record<string, JsonValue>;
    expect(Object.hasOwn(parsed, "__proto__")).toBe(true);
    expect(({} as Record<string, unknown>)["polluted"]).toBeUndefined();
  });

  it("refuses unbounded nesting", () => {
    expect(() => parseJson("[".repeat(200) + "]".repeat(200))).toThrow(/too deep/);
  });

  it("round trips through stringifyJson, writing bigint as an integer literal", () => {
    const value: JsonValue = {
      ns: 1_793_700_000_123_456_789n,
      list: [1, "x", null, true],
      empty: {},
      none: [],
    };
    const text = stringifyJson(value);
    expect(text).toContain('"ns": 1793700000123456789');
    expect(parseJson(text)).toEqual(value);
  });
});

describe("hex and base64", () => {
  it("encodes and decodes lowercase 0x hex", () => {
    const bytes = Uint8Array.from([0, 1, 254, 255]);
    expect(bytesToHex(bytes)).toBe("0x0001feff");
    expect(hexToBytes("0x0001feff", "x")).toEqual(bytes);
    expect(hexToBytes("0x", "x")).toEqual(new Uint8Array());
  });

  it("refuses uppercase, unprefixed, odd and wrong-length hex", () => {
    for (const bad of ["0X00", "00", "0x0", "0xAB", "0xzz", " 0x00"]) {
      expect(() => hexToBytes(bad, "x"), bad).toThrow(WireError);
    }
    expect(() => hexToBytes("0x0001", "x", 3)).toThrow(/expected 3 bytes/);
  });

  it("round trips standard padded base64 and refuses anything else", () => {
    for (const n of [0, 1, 2, 3, 4, 100]) {
      const bytes = Uint8Array.from({ length: n }, (_, i) => (i * 37 + 5) & 0xff);
      expect(base64ToBytes(bytesToBase64(bytes), "x")).toEqual(bytes);
    }
    for (const bad of ["QQ", "QQ=", "Q===", "QQ==\n", "QQ-_", "QR==", " QQ=="]) {
      expect(() => base64ToBytes(bad, "x"), bad).toThrow(WireError);
    }
    expect(base64ToBytes("QQ==", "x")).toEqual(Uint8Array.of(0x41));
  });

  it("encodes a large buffer without overflowing the call stack", () => {
    const big = new Uint8Array(3 * 1024 * 1024).fill(0xab);
    expect(base64ToBytes(bytesToBase64(big), "big").length).toBe(big.length);
  });
});
