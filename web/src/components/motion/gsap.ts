import { gsap } from "gsap";
import { MotionPathPlugin } from "gsap/MotionPathPlugin";
import { ScrollTrigger } from "gsap/ScrollTrigger";

// One registration for every scene. Scenes run inside gsap.matchMedia() so nothing is created
// (and nothing is left behind) when the visitor prefers reduced motion.
gsap.registerPlugin(ScrollTrigger, MotionPathPlugin);

export { gsap, ScrollTrigger };

/** The only motion condition any scene runs under. Without it the page renders its static baseline. */
export const MOTION_OK = "(prefers-reduced-motion: no-preference)";
