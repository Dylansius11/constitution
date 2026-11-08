import { bytesToHex, sha256 } from "./bytes.ts";
import { InputError } from "./errors.ts";

/** Borsh field and vector order match constitution_types::bundle::SourceBundle. */
export interface SourceBundle {
  contract_name: string;
  files: readonly (readonly [path: string, content: Uint8Array])[];
  foundry_profile: string;
}

const MAX_BYTES = 2 * 1024 * 1024;

/** Preserve file order: producers (including the CLI) sort paths before constructing the bundle. */
export function encodeSourceBundle(bundle: SourceBundle): Uint8Array {
  const selected = /^(src\/[^:]+\.sol):[A-Za-z_][A-Za-z0-9_]*$/.exec(bundle.contract_name);
  if (
    bundle.foundry_profile !== "default" ||
    bundle.files.length < 1 ||
    bundle.files.length > 128 ||
    !/^[A-Za-z0-9_/:.]+$/.test(bundle.contract_name) ||
    selected === null ||
    !bundle.files.some(([path]) => path === selected[1])
  ) {
    throw new InputError("unsupported bundle profile, contract or file count");
  }
  const seen = new Set<string>();
  const encoder = new TextEncoder();
  const contract = encoder.encode(bundle.contract_name);
  const fields: Uint8Array[] = [];
  let length = 4 + contract.length + 4;
  for (const [path, content] of bundle.files) {
    if (
      !path.startsWith("src/") ||
      !path.endsWith(".sol") ||
      path.includes("\\") ||
      path.includes("\0") ||
      path.split("/").some((part) => part === "" || part === "." || part === "..") ||
      seen.has(path)
    ) {
      throw new InputError("unsafe or duplicate source path");
    }
    seen.add(path);
    const name = encoder.encode(path);
    fields.push(name, content);
    length += 8 + name.length + content.length;
    if (length > MAX_BYTES) throw new InputError("Borsh source bundle exceeds 2 MiB");
  }
  const profile = encoder.encode(bundle.foundry_profile);
  length += 4 + profile.length;
  if (length > MAX_BYTES) throw new InputError("Borsh source bundle exceeds 2 MiB");
  const bytes = new Uint8Array(length);
  const view = new DataView(bytes.buffer);
  let offset = 0;
  function uint(value: number): void {
    view.setUint32(offset, value, true);
    offset += 4;
  }
  function vector(value: Uint8Array): void {
    uint(value.length);
    bytes.set(value, offset);
    offset += value.length;
  }
  vector(contract);
  uint(bundle.files.length);
  for (const field of fields) vector(field);
  vector(profile);
  return bytes;
}

export async function sourceCommitment(bytes: Uint8Array): Promise<string> {
  return bytesToHex(await sha256(bytes));
}
