import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";

// Same condition as the pinned-stage media query in Story.astro. Elsewhere the story is the static list.
const PINNED = `(min-width: 48rem) and (min-height: 38rem) and ${MOTION_OK}`;
const STEPS = 6;
const CONTROLS = "a[href], button";

/**
 * Story scene. The stage is pinned by CSS `position: sticky`; this only reads how far the visitor has
 * scrolled the tall track and writes the current step (`data-step`), the act (`data-act`, which
 * switches the ground) and the progress (`--p`, the rail). CSS does every swap.
 */
export function initStory(): void {
  const track = document.querySelector<HTMLElement>(".story__track");
  const stage = track?.querySelector<HTMLElement>(".story__stage");
  if (!track || !stage) return;

  const items = [...stage.querySelectorAll<HTMLElement>(".step")];

  // Cards that are not on stage stay readable for assistive technology but must not take focus.
  const setFocusable = (current: number) => {
    items.forEach((item, i) => {
      for (const control of item.querySelectorAll<HTMLElement>(CONTROLS)) {
        if (i === current) control.removeAttribute("tabindex");
        else control.setAttribute("tabindex", "-1");
      }
    });
  };

  gsap.matchMedia().add(PINNED, () => {
    let current = -1;

    const show = (step: number) => {
      if (step === current) return;
      current = step;
      stage.dataset["step"] = String(step);
      stage.dataset["act"] = step < 3 ? "0" : "1";
      setFocusable(step);
    };

    show(0);

    ScrollTrigger.create({
      trigger: track,
      start: () => `top top+=${parseFloat(getComputedStyle(stage).top)}`,
      end: "bottom bottom",
      invalidateOnRefresh: true,
      onUpdate: (self) => {
        stage.style.setProperty("--p", self.progress.toFixed(4));
        show(Math.min(STEPS - 1, Math.floor(self.progress * STEPS)));
      },
    });

    return () => {
      stage.dataset["step"] = "0";
      delete stage.dataset["act"];
      stage.style.removeProperty("--p");
      for (const control of stage.querySelectorAll<HTMLElement>(CONTROLS)) {
        control.removeAttribute("tabindex");
      }
    };
  });
}
