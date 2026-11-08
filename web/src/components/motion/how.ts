import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";

/**
 * How-it-works scenes: the verdict fork draws as it enters the viewport, and the rail draws down
 * the six steps, filling each node as it is reached. Only stroke-dashoffset and transform move.
 * Static baseline (no JavaScript or reduced motion): everything fully drawn.
 */
export function initHow(): void {
  const section = document.querySelector<HTMLElement>("#how");
  if (!section) return;

  gsap.matchMedia().add(MOTION_OK, () => {
    for (const svg of section.querySelectorAll<SVGSVGElement>(".fork__svg")) {
      const tracks = gsap.utils.toArray<SVGPathElement>(".fork__track:not(.fork__track--ai)", svg);
      const ai = gsap.utils.toArray<SVGPathElement>(".fork__track--ai", svg);

      gsap
        .timeline({
          defaults: { ease: "none" },
          scrollTrigger: {
            trigger: svg,
            start: "top 85%",
            end: "bottom 50%",
            scrub: 0.6,
          },
        })
        .fromTo(tracks, { strokeDashoffset: 1000 }, { strokeDashoffset: 0, duration: 0.6 }, 0)
        .fromTo(ai, { strokeDashoffset: 1000 }, { strokeDashoffset: 0, duration: 0.4 }, 0.5);
    }

    const waiting: HTMLElement[] = [];

    for (const step of section.querySelectorAll<HTMLElement>(".how__step")) {
      const node = step.querySelector<HTMLElement>(".how__node");
      const seg = step.querySelector<HTMLElement>(".how__seg");

      if (node) {
        waiting.push(node);
        const reach = ScrollTrigger.create({
          trigger: step,
          start: "top 70%",
          end: "max",
          onEnter: () => node.classList.remove("is-waiting"),
          onLeaveBack: () => node.classList.add("is-waiting"),
        });
        if (reach.scroll() < reach.start) node.classList.add("is-waiting");
      }

      if (seg) {
        gsap.fromTo(
          seg,
          { scaleY: 0 },
          {
            scaleY: 1,
            ease: "none",
            scrollTrigger: { trigger: step, start: "top 70%", end: "bottom 70%", scrub: 0.6 },
          },
        );
      }
    }

    return () => {
      for (const node of waiting) node.classList.remove("is-waiting");
    };
  });
}
