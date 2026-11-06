import { gsap, MOTION_OK, ScrollTrigger } from "./gsap.ts";
import { afterIntro } from "./intro.ts";

type Timeline = gsap.core.Timeline;

/** Attempt start times within one loop, top line to bottom line. */
const ATTEMPT_STARTS = [0, 0.75, 1.5, 2.2, 2.95];
const ATTEMPT_TRAVEL = 1.6;
const CERT_START = 3.7;
const CERT_TRAVEL = 2.6;

const toggle = (el: Element | null, name: string, on: boolean): void => {
  el?.classList.toggle(name, on);
};

/**
 * Hero scene. After the entrance opens: the headline rises word by word, the lines draw into the
 * gate and the mark assembles. Then the scene loops while the hero is on screen: an attempt runs
 * down each stolen-key line and dies at its tick; the certified attempt passes through the gate and
 * lands on the proxy. The start state mirrors the `.has-js` CSS in Hero.astro and HeroLines.astro.
 */
export function initHero(): void {
  const hero = document.querySelector<HTMLElement>("#top");
  if (!hero) return;

  gsap.matchMedia().add(MOTION_OK, () => {
    const entrance = gsap.timeline({ paused: true, defaults: { ease: "expo.out" } });
    const loops: Timeline[] = [];

    entrance
      .fromTo(
        gsap.utils.toArray<HTMLElement>(".hero__word-in", hero),
        { y: 0, yPercent: 108 },
        { yPercent: 0, duration: 1.1, stagger: 0.045 },
        0.1,
      )
      .fromTo(
        gsap.utils.toArray<HTMLElement>(".hero__highlight", hero),
        { clipPath: "inset(0% 100% 0% 0%)" },
        { clipPath: "inset(0% 0% 0% 0%)", duration: 0.8, ease: "power3.inOut" },
        0.95,
      )
      .fromTo(
        gsap.utils.toArray<HTMLElement>(".hero__aside > *", hero),
        { opacity: 0, y: 18 },
        { opacity: 1, y: 0, duration: 0.9, stagger: 0.08 },
        0.55,
      );

    for (const svg of hero.querySelectorAll<SVGSVGElement>(".hero-lines")) {
      entrance.add(drawIn(svg), 0);
      loops.push(attempts(svg));
    }

    let started = false;
    let visible = true;
    const sync = (): void => {
      for (const loop of loops) {
        if (started && visible) loop.play();
        else loop.pause();
      }
    };

    entrance.call(
      () => {
        started = true;
        sync();
      },
      [],
      1.6,
    );

    ScrollTrigger.create({
      trigger: hero,
      start: "top bottom",
      end: "bottom top",
      onToggle: (self) => {
        visible = self.isActive;
        sync();
      },
    });

    afterIntro(() => entrance.play());

    return () => {
      for (const el of hero.querySelectorAll(".is-hit, .is-open, .is-upgraded")) {
        el.classList.remove("is-hit", "is-open", "is-upgraded");
      }
    };
  });
}

function drawIn(svg: SVGSVGElement): Timeline {
  const pick = (selector: string): Element[] => gsap.utils.toArray<Element>(selector, svg);
  const tl = gsap.timeline({ defaults: { ease: "power3.out" } });

  tl.to(pick(".node"), { strokeDashoffset: 0, duration: 1.1, ease: "power2.inOut" }, 0)
    .to(pick(".curve"), { strokeDashoffset: 0, duration: 1.2, stagger: 0.07 }, 0.1)
    .to(pick(".through"), { strokeDashoffset: 0, duration: 1.4, ease: "power2.inOut" }, 0.4)
    .to(pick(".label"), { opacity: 1, duration: 0.7, stagger: 0.05 }, 0.3)
    .set(pick(".node__mark"), { opacity: 1 }, 0.3)
    .from(
      pick("[data-part='left']"),
      { x: -150, y: -100, rotation: -24, opacity: 0, duration: 1.2, ease: "expo.out" },
      0.3,
    )
    .from(
      pick("[data-part='right']"),
      { x: 150, y: -100, rotation: 24, opacity: 0, duration: 1.2, ease: "expo.out" },
      0.38,
    )
    .from(pick("[data-part='base']"), { y: 160, opacity: 0, duration: 1.2, ease: "expo.out" }, 0.46)
    .fromTo(
      pick(".stop"),
      { opacity: 0, scale: 0.5, transformOrigin: "50% 50%" },
      { opacity: 1, scale: 1, duration: 0.5, stagger: 0.06, ease: "back.out(3)" },
      1.0,
    )
    .to(pick(".proxy"), { opacity: 1, duration: 0.6 }, 1.3);

  return tl;
}

function attempts(svg: SVGSVGElement): Timeline {
  const loop = gsap.timeline({ paused: true, repeat: -1, repeatDelay: 0.9 });

  gsap.utils.toArray<SVGGElement>(".authority__line", svg).forEach((line, i) => {
    const packet = line.querySelector(".packet");
    const trip = line.querySelector<SVGPathElement>(".trip");
    const stop = line.querySelector(".stop");
    const burst = line.querySelector(".burst");
    if (!packet || !trip) return;

    const t = ATTEMPT_STARTS[i] ?? i * 0.75;
    const hit = t + ATTEMPT_TRAVEL;
    loop
      .set(packet, { visibility: "visible", opacity: 1, scale: 1, immediateRender: false }, t)
      .to(packet, { motionPath: { path: trip }, duration: ATTEMPT_TRAVEL, ease: "power2.in" }, t)
      .call(toggle, [stop, "is-hit", true], hit)
      .to(packet, { scale: 0, opacity: 0, transformOrigin: "50% 50%", duration: 0.2 }, hit)
      .fromTo(
        burst,
        { visibility: "visible", scale: 0.3, opacity: 1, transformOrigin: "50% 50%" },
        { scale: 1.9, opacity: 0, duration: 0.7, ease: "expo.out", immediateRender: false },
        hit,
      )
      .call(toggle, [stop, "is-hit", false], hit + 0.55);
  });

  const cert = svg.querySelector(".packet--cert");
  const certTrip = svg.querySelector<SVGPathElement>(".certified .trip");
  const node = svg.querySelector<SVGCircleElement>(".node");
  const proxy = svg.querySelector(".proxy");
  const proxyEnd = svg.querySelector<SVGCircleElement>(".proxy__end");
  const mark = svg.querySelector(".node__mark");
  if (!cert || !certTrip || !node || !proxyEnd) return loop;

  // The dot moves linearly, so the moment it crosses the ring is a fraction of the trip.
  const cx = node.cx.baseVal.value;
  const r = node.r.baseVal.value;
  const end = proxyEnd.cx.baseVal.value;
  const enter = CERT_START + CERT_TRAVEL * ((cx - r) / end);
  const leave = CERT_START + CERT_TRAVEL * ((cx + r) / end);
  const land = CERT_START + CERT_TRAVEL;

  loop
    .set(cert, { visibility: "visible", opacity: 1, scale: 1, immediateRender: false }, CERT_START)
    .to(cert, { motionPath: { path: certTrip }, duration: CERT_TRAVEL, ease: "none" }, CERT_START)
    .call(toggle, [node, "is-open", true], enter)
    .to(
      mark,
      {
        scale: 1.1,
        transformOrigin: "50% 50%",
        duration: 0.25,
        yoyo: true,
        repeat: 1,
        ease: "power2.out",
      },
      enter,
    )
    .call(toggle, [node, "is-open", false], leave + 0.25)
    .call(toggle, [proxy, "is-upgraded", true], land)
    .to(cert, { scale: 0, opacity: 0, transformOrigin: "50% 50%", duration: 0.25 }, land)
    .call(toggle, [proxy, "is-upgraded", false], land + 1.4);

  return loop;
}
