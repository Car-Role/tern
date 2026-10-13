// Quiet hover cards. `use:hover={() => data}` on any element shows a small card after a short rest.
// The function runs when the card opens, so the card always shows current data.

import { app } from "./store.svelte";

export type HoverData = {
  title?: string;
  /** Label and value pairs. A value in `mono` rows uses the code font. */
  rows?: { label: string; value: string; mono?: boolean; tone?: "add" | "del" | "amber" | "blue" }[];
  /** A longer text under the rows, for example a commit message. */
  text?: string;
  /** A short hint at the bottom, for example "Right-click for more". */
  hint?: string;
};

const DELAY = 450;

export function hover(node: HTMLElement, get: () => HoverData | null) {
  let current = get;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const enter = () => {
    clearTimeout(timer);
    // After one card has shown, the next one opens fast while the mouse moves along a list.
    timer = setTimeout(() => {
      const data = current();
      if (data && node.isConnected) app.showHover(node, data);
    }, app.hoverCard ? 120 : DELAY);
  };
  const leave = () => {
    clearTimeout(timer);
    app.hideHover(node);
  };
  node.addEventListener("mouseenter", enter);
  node.addEventListener("mouseleave", leave);
  node.addEventListener("mousedown", leave);
  node.addEventListener("contextmenu", leave);
  return {
    update(next: () => HoverData | null) {
      current = next;
    },
    destroy() {
      leave();
      node.removeEventListener("mouseenter", enter);
      node.removeEventListener("mouseleave", leave);
      node.removeEventListener("mousedown", leave);
      node.removeEventListener("contextmenu", leave);
    },
  };
}
