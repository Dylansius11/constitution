/**
 * Scroll reveal for blocks marked `data-reveal`: each rises once as it enters the view, staggered by
 * its `--i`. The start state lives in global.css under `html.reveal-armed`, which the layout sets
 * only when motion is welcome; this module marks `reveal-ready` so the layout's fallback stands down.
 */
const root = document.documentElement;

if (root.classList.contains("reveal-armed")) {
  root.classList.add("reveal-ready");

  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        entry.target.classList.add("is-in");
        observer.unobserve(entry.target);
      }
    },
    { rootMargin: "0px 0px -12% 0px" },
  );

  for (const el of document.querySelectorAll("[data-reveal]")) observer.observe(el);
}
