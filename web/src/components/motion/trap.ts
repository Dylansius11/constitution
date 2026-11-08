/**
 * "Spot the trap": opens the answer once the visitor has found the trap line or asked for it.
 * Everything else (the per-line explanations) is plain CSS; without this script the answer is
 * simply always open.
 */
export function initTrap(): void {
  const section = document.getElementById("trap");
  const code = document.getElementById("trap-code");
  const show = document.getElementById("trap-show");
  if (!section || !code || !show) return;

  const open = () => {
    section.dataset.open = "true";
  };

  code.addEventListener("change", (event) => {
    const target = event.target;
    if (target instanceof HTMLInputElement && target.value === "trap") open();
  });
  show.addEventListener("click", () => {
    open();
    document.getElementById("trap-answer")?.scrollIntoView({
      behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "nearest",
    });
  });
}
