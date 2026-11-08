import { gsap } from "./gsap.ts";

const DONE = "constitution:intro-done";
const SEEN_KEY = "constitution:intro";

/** Runs `start` once the entrance has opened, or right away when there is no entrance. */
export function afterIntro(start: () => void): void {
  if (!document.documentElement.classList.contains("intro")) {
    start();
    return;
  }

  document.addEventListener(DONE, start, { once: true });
}

/**
 * Entrance scene. The inline script in Intro.astro decides whether it runs (class `intro` on html);
 * this only animates it and always hands the page back: on completion, on any key or click.
 */
export function initIntro(): void {
  const root = document.documentElement;
  const scene = document.querySelector<HTMLElement>("[data-intro]");
  if (!scene || !root.classList.contains("intro")) return;

  let opened = false;
  const open = (): void => {
    if (opened) return;
    opened = true;
    document.dispatchEvent(new Event(DONE));
  };

  const pick = (selector: string): Element[] => gsap.utils.toArray<Element>(selector, scene);
  const part = (name: string): Element[] => pick(`.intro__mark [data-part='${name}']`);

  const tl = gsap.timeline({
    defaults: { ease: "expo.out" },
    onComplete: () => finish(),
  });

  tl.to(
    pick(".intro__ring circle"),
    { strokeDashoffset: 0, duration: 1.1, ease: "power2.inOut" },
    0,
  )
    .from(part("left"), { x: -260, y: -180, rotation: -30, opacity: 0, duration: 1 }, 0.1)
    .from(part("right"), { x: 260, y: -180, rotation: 30, opacity: 0, duration: 1 }, 0.18)
    .from(part("base"), { y: 240, opacity: 0, duration: 1 }, 0.26)
    .fromTo(
      pick(".intro__char"),
      { y: 0, yPercent: 110 },
      { yPercent: 0, duration: 0.9, stagger: 0.035 },
      0.55,
    )
    .to(pick(".intro__note"), { opacity: 1, duration: 0.5, ease: "power1.out" }, 0.95)
    .to(
      pick(".intro__line path"),
      { strokeDashoffset: 0, duration: 0.6, ease: "power3.inOut" },
      1.45,
    )
    .to(pick(".intro__brand"), { opacity: 0, scale: 0.96, duration: 0.3, ease: "power2.in" }, 1.95)
    .call(open, [], 2.05)
    .to(pick(".intro__half--top"), { yPercent: -100, duration: 0.85, ease: "expo.inOut" }, 2.05)
    .to(pick(".intro__half--bottom"), { yPercent: 100, duration: 0.85, ease: "expo.inOut" }, 2.05)
    .to(pick(".intro__line"), { opacity: 0, duration: 0.3, ease: "power1.out" }, 2.4);

  function finish(): void {
    tl.kill();
    root.classList.remove("intro");
    window.removeEventListener("keydown", finish);
    scene?.removeEventListener("pointerdown", finish);
    try {
      sessionStorage.setItem(SEEN_KEY, "1");
    } catch {
      // Storage can be unavailable (private mode); the entrance simply plays again next visit.
    }
    open();
  }

  window.addEventListener("keydown", finish);
  scene.addEventListener("pointerdown", finish);
}
