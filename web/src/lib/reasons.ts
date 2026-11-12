/**
 * Plain-language names for the stable rule ids and reason codes the verifier and the gate record
 * (`crates/constitution-types/src/reasons.rs`). A code this table does not know is shown as its
 * number, never guessed: a newer verifier must not make the page invent a meaning.
 */

const RULES: Readonly<Record<number, string>> = {
  0: "Pre-check",
  1: "Storage layout",
  2: "Solvency",
  3: "Opcode and slot filter",
  4: "Initializer allowlist",
  5: "Differential replay",
  6: "Private AI review",
};

const REASONS: Readonly<Record<number, string>> = {
  1: "The submitted source does not match the commitment recorded on NEAR.",
  2: "The compiled code does not match the code hash in the proposal.",
  3: "The proxy no longer points at the implementation this proposal expected.",
  4: "The committed reference source does not reproduce the live implementation.",
  10: "The new storage layout does not extend the current one append-only.",
  11: "The reference storage layout is missing.",
  20: "A bounded symbolic check found a sequence that leaves the vault owing more than it holds.",
  21: "Seeded fuzzing found a sequence that leaves the vault owing more than it holds.",
  22: "The private reviewer proposed a counterexample and the verifier reproduced it.",
  23: "The solver could not finish inside its bound, so the proposal fails closed.",
  30: "The bytecode contains a forbidden opcode: SELFDESTRUCT, CALLCODE or DELEGATECALL.",
  31: "The bytecode embeds a protected proxy storage slot.",
  40: "The initializer calls a function that is not on the committed allowlist.",
  41: "The initializer arguments do not match the committed schema.",
  50: "Replaying the committed corpus produced behavior that differs from the current implementation.",
  51: "The chain witness for the replay was incomplete, so the proposal fails closed.",
};

/** `R2 Solvency`, `Pre-check` for rule 0, or `Rule 9` for an id this page does not know. */
export function ruleLabel(ruleId: number): string {
  const name = RULES[ruleId];
  if (name === undefined) return `Rule ${ruleId}`;
  return ruleId === 0 ? name : `R${ruleId} ${name}`;
}

/** The rule name without its number, or `null` when unknown. */
export function ruleName(ruleId: number): string | null {
  return RULES[ruleId] ?? null;
}

/** One plain sentence for a recorded reason code, or `null` when this page does not know it. */
export function reasonText(reasonCode: number): string | null {
  return REASONS[reasonCode] ?? null;
}
