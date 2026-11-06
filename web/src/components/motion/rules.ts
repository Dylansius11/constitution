import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";

/**
 * Rules scene: each rule row plays its figure once, when it is a third of the way into the view.
 * CSS owns every transition; this only adds `is-on`. Reduced motion never sees a start state.
 */
export function initRules(): void {
  const section = document.querySelector<HTMLElement>("#rules");
  if (!section) return;

  const rows = [...section.querySelectorAll<HTMLElement>(".rule")];

  gsap.matchMedia().add(MOTION_OK, () => {
    for (const row of rows) {
      ScrollTrigger.create({
        trigger: row,
        start: "top 70%",
        once: true,
        onEnter: () => row.classList.add("is-on"),
      });
    }

    return () => {
      for (const row of rows) row.classList.remove("is-on");
    };
  });
}
