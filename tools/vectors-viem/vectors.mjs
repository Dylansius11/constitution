// T-ENC-1: independent recomputation of vectors/evm-tx.json with viem.
// Exits non-zero on the first byte difference.
import { readFileSync } from "node:fs";
import { encodeFunctionData, keccak256, parseAbi, serializeTransaction } from "viem";

const abi = parseAbi([
  "function execute(bytes32 certId, address newImpl, bytes32 newImplCodehash, address expectedCurrentImpl, uint64 deadline, bytes initCalldata)",
  "function rotateSigner(address newSigner, bytes32 changeId, uint64 deadline)",
]);

const file = new URL("../../vectors/evm-tx.json", import.meta.url);
const { vectors } = JSON.parse(readFileSync(file, "utf8"));

function calldata(call) {
  if (call.function === "execute") {
    return encodeFunctionData({
      abi,
      functionName: "execute",
      args: [call.certId, call.newImpl, call.newImplCodehash, call.expectedCurrentImpl, BigInt(call.deadline), call.initCalldata],
    });
  }
  if (call.function === "rotateSigner") {
    return encodeFunctionData({
      abi,
      functionName: "rotateSigner",
      args: [call.newSigner, call.changeId, BigInt(call.deadline)],
    });
  }
  throw new Error(`unknown function ${call.function}`);
}

let failures = 0;
for (const v of vectors) {
  const nonce = Number(v.nonce);
  if (!Number.isSafeInteger(nonce) || String(nonce) !== v.nonce) throw new Error(`${v.name}: nonce not representable`);
  const preimage = serializeTransaction({
    type: "eip1559",
    chainId: Number(v.chain_id),
    nonce,
    maxFeePerGas: BigInt(v.max_fee_per_gas),
    maxPriorityFeePerGas: BigInt(v.max_priority_fee_per_gas),
    gas: BigInt(v.gas_limit),
    to: v.to,
    value: 0n,
    data: calldata(v.call),
    accessList: [],
  });
  const ok = preimage === v.preimage && keccak256(preimage) === v.signing_hash;
  console.log(`${ok ? "ok  " : "FAIL"} ${v.name} ${keccak256(preimage)}`);
  if (!ok) failures += 1;
}
if (failures > 0) {
  console.error(`${failures} of ${vectors.length} vectors differ`);
  process.exit(1);
}
console.log(`all ${vectors.length} vectors byte-identical`);
