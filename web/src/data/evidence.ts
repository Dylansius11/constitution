// Build-time only: import this module in Astro frontmatter, never in a client script.
// Client re-checks receive only the three required identifiers through HTML data attributes.
import raw from "../../../spikes/gate-min/base-sepolia-evidence.json";
import { WireError } from "../lib/errors.ts";
import { Obj } from "../lib/wire.ts";

export interface SpikeCase {
  nearTxHash: string;
  baseTxHash: string;
  status: 0 | 1;
  gasUsed: number;
  block: number;
  revertReason: "DeadlinePassed" | null;
}

export interface SpikeEvidence {
  label: string;
  observedAtUtc: string;
  nearAccount: string;
  signer: string;
  addresses: {
    executor: string;
    proxy: string;
    proxyAdmin: string;
    vaultV1Implementation: string;
    vaultV2Honest: string;
    token: string;
  };
  honest: SpikeCase;
  expired: SpikeCase;
}

function quantity(o: Obj, key: string): number {
  const text = o.str(key);
  if (!/^0x[0-9a-f]+$/.test(text)) throw new WireError(o.at(key), "expected an RPC hex quantity");
  const value = Number(BigInt(text));
  if (!Number.isSafeInteger(value))
    throw new WireError(o.at(key), "quantity exceeds a safe integer");
  return value;
}

export function parseSpikeEvidence(value: unknown): SpikeEvidence {
  const o = new Obj(value, "spikes/gate-min/base-sepolia-evidence.json");
  const observedAtUtc = o.str("observed_at_utc");
  if (!Number.isFinite(Date.parse(observedAtUtc)) || !/(?:Z|\+00:00)$/.test(observedAtUtc)) {
    throw new WireError(o.at("observed_at_utc"), "expected a UTC timestamp");
  }
  const addresses = o.obj("deployment").obj("addresses");
  const cases = o.arr("cases").map((v, i) => new Obj(v, `${o.at("cases")}[${i}]`));
  function readCase(name: string, expectedStatus: 0 | 1): SpikeCase {
    const matches = cases.filter((c) => c.str("name") === name);
    const c = matches[0];
    if (matches.length !== 1 || c === undefined) {
      throw new WireError(o.at("cases"), `expected exactly one ${name} case`);
    }
    const receipt = c.obj("receipt");
    const status = quantity(receipt, "status");
    if (status !== expectedStatus)
      throw new WireError(receipt.at("status"), `expected ${expectedStatus} for ${name}`);
    const baseTxHash = c.obj("signed").hex("tx_hash", 32);
    if (receipt.hex("transactionHash", 32) !== baseTxHash)
      throw new WireError(
        receipt.at("transactionHash"),
        "receipt hash disagrees with signed transaction hash",
      );
    const nearTxHash = c.str("near_tx_hash");
    if (!/^[1-9A-HJ-NP-Za-km-z]{43,44}$/.test(nearTxHash))
      throw new WireError(c.at("near_tx_hash"), "expected a NEAR transaction hash");
    if (
      expectedStatus === 0 &&
      !/^0x3dee80f2[0-9a-f]{128}$/.test(c.obj("revert_call_response").obj("error").str("data"))
    ) {
      throw new WireError(
        c.at("revert_call_response"),
        "invalid revert payload; expected DeadlinePassed(uint64,uint256)",
      );
    }
    return {
      nearTxHash,
      baseTxHash,
      status: expectedStatus,
      gasUsed: quantity(receipt, "gasUsed"),
      block: quantity(receipt, "blockNumber"),
      revertReason: expectedStatus === 0 ? "DeadlinePassed" : null,
    };
  }
  return {
    label: o.str("label"),
    observedAtUtc,
    nearAccount: o.account("near_account"),
    signer: o.hex("signer", 20),
    addresses: {
      executor: addresses.hex("executor", 20),
      proxy: addresses.hex("proxy", 20),
      proxyAdmin: addresses.hex("proxyAdmin", 20),
      vaultV1Implementation: addresses.hex("vaultV1Implementation", 20),
      vaultV2Honest: addresses.hex("vaultV2Honest", 20),
      token: addresses.hex("token", 20),
    },
    honest: readCase("honest_upgrade", 1),
    expired: readCase("expired_deadline", 0),
  };
}

export const spikeEvidence = parseSpikeEvidence(raw);
