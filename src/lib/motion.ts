// Small motion helpers. Motion in Tern only marks a change of state; it never plays on first render.

const EASE_OUT = "cubic-bezier(0.23, 1, 0.32, 1)";

/**
 * Pops a CI status icon when its run turns `completed`, so a finished run is noticed.
 * Nothing plays when the element first appears (opening the tab, a list refresh).
 * It uses element.animate, not a class: Svelte rewrites the class list when the status changes.
 */
export function popOnDone(node: HTMLElement, status: string) {
  let last = status;
  return {
    update(next: string) {
      if (last !== "completed" && next === "completed") {
        const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;
        node.animate(
          calm
            ? [{ opacity: 0 }, { opacity: 1 }]
            : [{ opacity: 0, transform: "scale(0.85)" }, { opacity: 1, transform: "scale(1)" }],
          { duration: calm ? 150 : 220, easing: EASE_OUT },
        );
      }
      last = next;
    },
  };
}
