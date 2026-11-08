import drain from "../../../contracts/src/demo/VaultV2Drain.sol?raw";
import honest from "../../../contracts/src/demo/VaultV2Honest.sol?raw";

/**
 * The "Spot the trap" excerpt: the real `withdraw` of the demo drain contract, read from
 * contracts/src/demo at build time. Every selectable line needs a plain-language gloss keyed by
 * its exact trimmed text, so a change to the contract that is not mirrored here fails the build
 * instead of showing words about code that no longer exists.
 */
export type TrapKind = "trap" | "door" | "cover" | "plain";

export type TrapLine = {
  /** Line text with the function's own indentation removed. */
  text: string;
  /** 1-based number inside the excerpt. */
  n: number;
  /** Null for lines that only close a block and are not selectable. */
  gloss: { kind: TrapKind; text: string } | null;
};

const glosses: Record<string, { kind: TrapKind; text: string }> = {
  "function withdraw(uint256 amount) public override {": {
    kind: "plain",
    text: "The function that sends tokens back to a depositor. This version replaces the original one.",
  },
  "FeeRebateStorage storage fees = _feeRebateStorage();": {
    kind: "plain",
    text: "Opens a private notebook the contract keeps, labelled as a fee rebate.",
  },
  "uint256 stage = fees.stages[msg.sender];": {
    kind: "plain",
    text: "Looks up a counter for whoever is calling. It only moves for a very specific pattern of calls, so most people never see it change.",
  },
  "if (stage == 3) {": {
    kind: "door",
    text: "The hidden door. The counter reaches 3 only after a caller deposits 1, withdraws 1, then deposits 1 again. Anyone else never runs the block below.",
  },
  "VaultStorage storage s = _vaultStorage();": {
    kind: "plain",
    text: "Opens the vault's main ledger.",
  },
  "uint256 balance = s.balances[msg.sender];": {
    kind: "plain",
    text: "Reads how much the caller actually owns in the vault. Keep it in mind: it is read here and never compared with what is asked for.",
  },
  "s.balances[msg.sender] = 0;": {
    kind: "cover",
    text: "Sets the caller's recorded balance to zero, whatever they take out, so the books look tidy afterwards.",
  },
  "s.totalDeposits -= balance;": {
    kind: "cover",
    text: "Lowers the vault's total by what the caller owned, not by what is about to leave. The books and the real tokens drift apart here.",
  },
  "fees.stages[msg.sender] = 0;": {
    kind: "plain",
    text: "Resets the counter, so the same sequence works again.",
  },
  "s.token.safeTransfer(msg.sender, amount);": {
    kind: "trap",
    text: "This is the trap. It sends the amount to the caller, and the amount is whatever number the caller typed. Nothing compares it with the balance read a few lines up.",
  },
  "emit Withdrawn(msg.sender, amount);": {
    kind: "plain",
    text: "Announces the withdrawal publicly, with the full amount that was paid out.",
  },
  "return;": {
    kind: "cover",
    text: "Leaves early, so the safe original withdrawal below never runs for this caller.",
  },
  "if (stage == 1 && amount == 1) fees.stages[msg.sender] = 2;": {
    kind: "door",
    text: "Step two of the secret pattern: a withdrawal of exactly 1 moves the counter from 1 to 2. The normal code then runs, so the step looks harmless.",
  },
  "super.withdraw(amount);": {
    kind: "plain",
    text: "The original, safe withdrawal with its balance check. Everyone outside the secret pattern goes through here.",
  },
};

const START = "function withdraw(uint256 amount) public override {";
const END = "    }";

function excerpt(source: string): { lines: TrapLine[]; fromLine: number; toLine: number } {
  const all = source.split(/\r?\n/);
  const start = all.findIndex((line) => line.trim() === START);
  if (start < 0) throw new Error(`trap: "${START}" not found in VaultV2Drain.sol`);
  const end = all.findIndex((line, i) => i > start && line === END);
  if (end < 0) throw new Error("trap: end of withdraw not found in VaultV2Drain.sol");

  const used = new Set<string>();
  const lines = all.slice(start, end + 1).map((raw, i) => {
    const text = raw.replace(/^ {4}/, "");
    const key = text.trim();
    const gloss = glosses[key] ?? null;
    if (gloss) used.add(key);
    else if (!/^\}+$/.test(key)) throw new Error(`trap: no gloss for line "${key}"`);
    return { text, n: i + 1, gloss };
  });
  for (const key of Object.keys(glosses)) {
    if (!used.has(key)) throw new Error(`trap: gloss never used: "${key}"`);
  }
  return { lines, fromLine: start + 1, toLine: end + 1 };
}

const HONEST_CHECK = "if (amount > balance) revert InsufficientBalance(balance, amount);";
if (!honest.includes(HONEST_CHECK)) {
  throw new Error("trap: the honest balance check is no longer in VaultV2Honest.sol");
}

export const trap = {
  ...excerpt(drain),
  file: "contracts/src/demo/VaultV2Drain.sol",
  honestFile: "contracts/src/demo/VaultV2Honest.sol",
  honestCheck: HONEST_CHECK,
};
