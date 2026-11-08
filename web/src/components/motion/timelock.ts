import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";

const DAYS = 7;

/**
 * Timelock scene. While the section scrolls past the sticky dial, the seven day segments fill in
 * order and a hand walks the ring; the count reads "Day N of 7" and finally "Apply opens". It is an
 * illustration of the fixed delay, scrubbed by scroll, never a clock of a real pending change.
 */
export function initTimelock(): void {
  const section = document.querySelector<HTMLElement>("#timelock");
  const grid = section?.querySelector<HTMLElement>(".timelock__grid");
  const hand = section?.querySelector<SVGGElement>(".dial__hand");
  const count = section?.querySelector<SVGTextElement>(".dial__count");
  if (!section || !grid || !hand || !count) return;

  const segments = [...section.querySelectorAll<SVGCircleElement>(".dial__seg")];

  gsap.matchMedia().add(MOTION_OK, () => {
    let shown = -1;

    const render = (progress: number): void => {
      hand.style.transform = `rotate(${(progress * 360).toFixed(2)}deg)`;
      const filled = progress >= 0.999 ? DAYS : Math.floor(progress * DAYS);
      if (filled === shown) return;
      shown = filled;
      segments.forEach((segment, i) => {
        segment.classList.toggle("is-filled", i < filled);
      });
      count.textContent = filled >= DAYS ? "Apply opens" : `Day ${filled} of ${DAYS}`;
    };

    render(0);

    ScrollTrigger.create({
      trigger: grid,
      start: "top 65%",
      end: "bottom 75%",
      onUpdate: (self) => render(self.progress),
    });

    return () => {
      hand.style.removeProperty("transform");
      count.textContent = "";
      for (const segment of segments) segment.classList.remove("is-filled");
    };
  });
}
