import { WireError } from "./errors.ts";
import { asHexBytes, asHexFixed, asString, Obj } from "./wire.ts";

/**
 * `deployments/demo-gallery.json`: the attacks a visitor may run against the live demo gate. The
 * file is produced next to the deployment; this module only validates it. Paths are relative to
 * `contracts/` and become the sealed source bundle byte for byte.
 */

export interface ObservedRun {
  proposalId: number;
  nearTx: string;
  observedAt: string;
  /** Present on upgrades only. */
  baseTx?: string;
}

export interface DemoAttack {
  id: string;
  title: string;
  summary: string;
  expectedRule: number;
  expectedReason: number;
  /** `src/<file>.sol:<Name>`, the selected contract of the bundle. */
  contract: string;
  sourceFiles: readonly string[];
  impl: string;
  codehash: string;
  initCalldata: string;
  observed: ObservedRun | null;
}

export interface DemoUpgrade {
  from: string;
  to: string;
  observed: ObservedRun | null;
}

export interface DemoGallery {
  gate: string;
  proxy: string;
  attacks: readonly DemoAttack[];
  upgrades: readonly DemoUpgrade[];
}

const MAX_ATTACKS = 8;
const MAX_FILES = 16;
const MAX_INIT_CALLDATA_BYTES = 1024;
const SOURCE_PATH = /^src\/[A-Za-z0-9_/.-]+\.sol$/;
const TX_HASH = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

function text(o: Obj, key: string, max: number): string {
  const value = o.str(key).trim();
  if (value === "" || value.length > max) {
    throw new WireError(o.at(key), `expected 1 to ${max} characters`);
  }
  return value;
}

function nonZeroAddress(value: unknown, path: string): string {
  const address = asHexFixed(value, 20, path);
  if (/^0x0{40}$/.test(address)) throw new WireError(path, "the zero address is not a target");
  return address;
}

function sourcePath(value: unknown, path: string): string {
  const file = asString(value, path);
  if (!SOURCE_PATH.test(file) || file.split("/").some((part) => part === "" || part === "..")) {
    throw new WireError(path, "expected a contracts-relative src/*.sol path");
  }
  return file;
}

function parseObserved(o: Obj, withBase: boolean): ObservedRun | null {
  const value = o.raw("observed");
  if (value === null) return null;
  const run = new Obj(value, o.at("observed"));
  const nearTx = run.str("near_tx");
  if (!TX_HASH.test(nearTx)) throw new WireError(run.at("near_tx"), "expected a NEAR tx hash");
  const observedAt = run.str("observed_at");
  if (Number.isNaN(Date.parse(observedAt))) {
    throw new WireError(run.at("observed_at"), "expected an ISO-8601 time");
  }
  const baseTx = withBase ? asHexFixed(run.raw("base_tx"), 32, run.at("base_tx")) : undefined;
  return {
    proposalId: run.safe("proposal_id"),
    nearTx,
    observedAt,
    ...(baseTx === undefined ? {} : { baseTx }),
  };
}

function parseAttack(value: unknown, path: string): DemoAttack {
  const o = new Obj(value, path);
  const id = o.str("id");
  if (!/^[a-z0-9][a-z0-9-]{0,31}$/.test(id)) throw new WireError(o.at("id"), "expected a slug");
  const contract = o.str("contract");
  const selected = /^(src\/[A-Za-z0-9_/.-]+\.sol):[A-Za-z_][A-Za-z0-9_]*$/.exec(contract);
  if (selected === null) throw new WireError(o.at("contract"), "expected src/<file>.sol:<Name>");
  const sourceFiles = o.arr("source_files").map((file, i) => sourcePath(file, `${o.at("source_files")}[${i}]`));
  if (sourceFiles.length < 1 || sourceFiles.length > MAX_FILES) {
    throw new WireError(o.at("source_files"), `expected 1 to ${MAX_FILES} files`);
  }
  if (new Set(sourceFiles).size !== sourceFiles.length) {
    throw new WireError(o.at("source_files"), "duplicate path");
  }
  if (!sourceFiles.includes(selected[1] as string)) {
    throw new WireError(o.at("source_files"), "must include the selected contract's file");
  }
  const initCalldata = asHexBytes(o.raw("init_calldata"), o.at("init_calldata"));
  if ((initCalldata.length - 2) / 2 > MAX_INIT_CALLDATA_BYTES) {
    throw new WireError(o.at("init_calldata"), `longer than ${MAX_INIT_CALLDATA_BYTES} bytes`);
  }
  return {
    id,
    title: text(o, "title", 80),
    summary: text(o, "summary", 400),
    expectedRule: o.safe("expected_rule", 6),
    expectedReason: o.safe("expected_reason", 65535),
    contract,
    sourceFiles,
    impl: nonZeroAddress(o.raw("impl"), o.at("impl")),
    codehash: asHexFixed(o.raw("codehash"), 32, o.at("codehash")),
    initCalldata,
    observed: parseObserved(o, false),
  };
}

function parseUpgrade(value: unknown, path: string): DemoUpgrade {
  const o = new Obj(value, path);
  return { from: text(o, "from", 64), to: text(o, "to", 64), observed: parseObserved(o, true) };
}

/** Throws `WireError` naming the field. A malformed gallery must never render as a runnable one. */
export function parseDemoGallery(value: unknown): DemoGallery {
  const o = new Obj(value, "gallery");
  if (o.safe("version") !== 1) throw new WireError(o.at("version"), "unsupported version");
  const attacks = o.arr("attacks").map((entry, i) => parseAttack(entry, `gallery.attacks[${i}]`));
  if (attacks.length < 1 || attacks.length > MAX_ATTACKS) {
    throw new WireError(o.at("attacks"), `expected 1 to ${MAX_ATTACKS} attacks`);
  }
  if (new Set(attacks.map((attack) => attack.id)).size !== attacks.length) {
    throw new WireError(o.at("attacks"), "duplicate attack id");
  }
  return {
    gate: o.account("gate"),
    proxy: nonZeroAddress(o.raw("proxy"), o.at("proxy")),
    attacks,
    upgrades: o.arr("upgrades").map((entry, i) => parseUpgrade(entry, `gallery.upgrades[${i}]`)),
  };
}
