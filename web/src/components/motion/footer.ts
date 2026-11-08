import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";

/** Footer scene: the full-width wordmark rises letter by letter the first time it scrolls into view. */
export function initFooter(): void {
  const footer = document.querySelector<HTMLElement>(".footer");
  const word = footer?.querySelector<HTMLElement>(".footer__word");
  if (!footer || !word) return;

  gsap.matchMedia().add(MOTION_OK, () => {
    footer.classList.add("has-js");

    ScrollTrigger.create({
      trigger: word,
      start: "top bottom-=40",
      once: true,
      onEnter: () => footer.classList.add("is-on"),
    });

    return () => footer.classList.remove("has-js", "is-on");
  });
}
