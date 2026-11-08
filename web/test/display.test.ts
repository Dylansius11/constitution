import { describe, expect, it } from "vitest";
import { remaining, shortValue, spokenValue, utcDate, utcStamp } from "../src/lib/display.ts";

describe("readable chain data", () => {
  const hash = "0xc232188629763bec4eb9a72be7fd08f66ab5690a2ad997ac6ed20697ff0729d8";
  it("shortens hashes at both widths without changing bytes", () => {
    expect(shortValue(hash, "hash", "narrow")).toBe("0xc23218…29d8");
    expect(shortValue(hash, "hash", "wide")).toBe("0xc232188629…ff0729d8");
  });
  it("preserves checksum casing in addresses and short values", () => {
    const address = "0x8221C9477886cC456D7E1ccCC0866e0401b81070";
    expect(shortValue(address, "address", "narrow")).toBe("0x8221…1070");
    expect(shortValue(address, "address", "wide")).toBe("0x8221C9…b81070");
    expect(shortValue("Pending", "hash", "wide")).toBe("Pending");
  });
  it("keeps NEAR ids whole through 32 characters", () => {
    expect(shortValue("a".repeat(32), "near", "narrow")).toBe("a".repeat(32));
    expect(shortValue("a".repeat(33), "near", "wide")).toBe(`${"a".repeat(14)}…${"a".repeat(14)}`);
  });
  it("speaks a short name instead of an entire hash", () => {
    expect(spokenValue("Transaction hash", hash)).toBe(
      "Transaction hash, starts 0xc232, ends 29d8",
    );
    expect(spokenValue("State", "Pending")).toBe("State, Pending");
  });
});

describe("UTC and quiet remaining time", () => {
  it("renders UTC regardless of the source timezone and rejects invalid dates", () => {
    const ms = Date.parse("2026-11-03T10:04:47+07:00");
    expect(utcStamp(ms)).toBe("2026-11-03 03:04:47 UTC");
    expect(utcDate(ms)).toBe("2026-11-03");
    expect(() => utcStamp(Number.NaN)).toThrow(RangeError);
  });
  it.each([
    [0, "0 d 0 h 0 m"],
    [-1, "0 d 0 h 0 m"],
    [59999, "0 d 0 h 0 m"],
    [60000, "0 d 0 h 1 m"],
    [604800000, "7 d 0 h 0 m"],
    [603660000, "6 d 23 h 41 m"],
  ] as const)("formats %i ms without seconds", (ms, expected) => {
    expect(remaining(ms)).toBe(expected);
  });
});
