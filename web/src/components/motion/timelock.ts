import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";

/**
 * Scroll-scrubbed illustration, never a clock for a real pending change. The gate ledger
 * supplies an observed immutable delay; until then the static default remains visible.
 */
export function initTimelock(): void {
  const section = document.querySelector<HTMLElement>("#timelock");
  const grid = section?.querySelector<HTMLElement>(".timelock__grid");
  const hand = section?.querySelector<SVGGElement>(".dial__hand");
  const count = section?.querySelector<SVGTextElement>(".dial__count");
  const big = section?.querySelector<SVGTextElement>(".dial__big");
  const small = section?.querySelector<SVGTextElement>(".dial__small");
  const description = section?.querySelector<HTMLElement>(".dial__description");
  const segmentGroup = section?.querySelector<SVGGElement>(".dial__segments");
  const dayGroup = section?.querySelector<SVGGElement>(".dial__days");
  const segmentTemplate = section?.querySelector<SVGCircleElement>(".dial__seg");
  const dayTemplate = section?.querySelector<SVGTextElement>(".dial__day");
  if (!section || !grid || !hand || !count || !big || !small || !description ||
      !segmentGroup || !dayGroup || !segmentTemplate || !dayTemplate) return;

  let days = 3;
  let observed = false;
  let progress = 0;
  let motionActive = false;
  const render = (): void => {
    hand.style.transform = `rotate(${(progress * 360).toFixed(2)}deg)`;
    const filled = progress >= 0.999 ? days : Math.floor(progress * days);
    section.querySelectorAll<SVGCircleElement>(".dial__seg").forEach((segment, i) => {
      segment.classList.toggle("is-filled", i < filled);
    });
    count.textContent = filled >= days ? "Apply opens" : `Day ${filled} of ${days}`;
  };
  const selectDays = (selected: number): void => {
    if (!Number.isInteger(selected) || selected < 1 || selected > 30) return;
    if (observed && selected === days) return;
    observed = true;
    days = selected;
    const radius = 120;
    const circumference = 2 * Math.PI * radius;
    const length = circumference / days;
    const segments: SVGCircleElement[] = [];
    const labels: SVGTextElement[] = [];
    for (let i = 0; i < days; i++) {
      const segment = segmentTemplate.cloneNode(false) as SVGCircleElement;
      segment.setAttribute("data-day", String(i + 1));
      segment.setAttribute("stroke-dasharray", `${length - 6} ${circumference - length + 6}`);
      segment.setAttribute("stroke-dashoffset", String(-i * length));
      segments.push(segment);
      // Keep labels readable on a densely divided 30-day ring.
      if (days <= 12 || i === 0 || (i + 1) % 5 === 0) {
        const label = dayTemplate.cloneNode(false) as SVGTextElement;
        const angle = ((i + 0.5) / days) * 2 * Math.PI - Math.PI / 2;
        label.setAttribute("x", String(Math.cos(angle) * (radius + 24)));
        label.setAttribute("y", String(Math.sin(angle) * (radius + 24)));
        label.textContent = String(i + 1);
        labels.push(label);
      }
    }
    segmentGroup.replaceChildren(...segments);
    dayGroup.replaceChildren(...labels);
    big.textContent = `${days} ${days === 1 ? "day" : "days"}`;
    small.textContent = "observed gate delay";
    description.textContent = `Illustration: a dial divided into ${days} day segments shows the observed gate delay, not a live countdown.`;
    if (motionActive) render();
  };
  document.addEventListener("gate-config-observed", (event) => {
    selectDays((event as CustomEvent<{ governance_delay_days: number }>).detail.governance_delay_days);
  });
  if (section.dataset["governanceDelayDays"])
    selectDays(Number(section.dataset["governanceDelayDays"]));

  gsap.matchMedia().add(MOTION_OK, () => {
    motionActive = true;
    render();
    ScrollTrigger.create({
      trigger: grid,
      start: "top 65%",
      end: "bottom 75%",
      onUpdate: (self) => {
        progress = self.progress;
        render();
      },
    });
    return () => {
      motionActive = false;
      hand.style.removeProperty("transform");
      count.textContent = "";
      for (const segment of section.querySelectorAll<SVGCircleElement>(".dial__seg"))
        segment.classList.remove("is-filled");
    };
  });
}
