// Circular progress ring (SVG). Progress goes clockwise from 12 o'clock.

const SIZE = 200;
const STROKE = 5;
const R = (SIZE - STROKE) / 2 - 2;
const CIRCUMFERENCE = 2 * Math.PI * R;

export function createRing() {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", `0 0 ${SIZE} ${SIZE}`);
  svg.setAttribute("aria-hidden", "true");

  const track = document.createElementNS(ns, "circle");
  const progress = document.createElementNS(ns, "circle");
  for (const [el, cls] of [[track, "dial__track"], [progress, "dial__progress"]]) {
    el.setAttribute("cx", SIZE / 2);
    el.setAttribute("cy", SIZE / 2);
    el.setAttribute("r", R);
    el.setAttribute("stroke-width", STROKE);
    el.setAttribute("class", cls);
    svg.append(el);
  }
  progress.setAttribute("stroke-dasharray", CIRCUMFERENCE);

  return {
    el: svg,
    /** @param {number} fraction 0..1 of the session elapsed */
    set(fraction) {
      const f = Math.min(1, Math.max(0, fraction));
      progress.setAttribute("stroke-dashoffset", CIRCUMFERENCE * (1 - f));
      // Hide the round cap "dot" when nothing has elapsed yet.
      progress.style.opacity = f > 0.001 ? "1" : "0";
    },
  };
}
