/** Shapes shared by CodeBlock.astro and the command data (plain tsc cannot read .astro types). */
export interface CodeLine {
  /** A shell command, shown after `$`. */
  cmd?: string;
  /** A comment line, shown dimmed and never copied. */
  note?: string;
}

export interface CodeTab {
  id: string;
  label: string;
  lines: CodeLine[];
}
