<script lang="ts">
  import { openUrl } from "../lib/api";

  let { text, onLink }: { text: string; onLink: (href: string) => void } = $props();

  let html = $state("");
  let box: HTMLDivElement | undefined = $state();

  const slug = (s: string) => s.toLowerCase().trim().replace(/[^\w\- ]+/g, "").replace(/\s/g, "-");

  // The file can come from anywhere, so the HTML is cleaned before it is shown.
  $effect(() => {
    const src = text;
    Promise.all([import("marked"), import("dompurify")]).then(async ([{ marked }, { default: DOMPurify }]) => {
      const raw = await marked.parse(src, { gfm: true });
      if (src !== text) return;
      // Task-list checkboxes stay, but they cannot be clicked.
      const clean = DOMPurify.sanitize(raw, { FORBID_TAGS: ["style", "form", "button", "iframe", "select", "textarea"], FORBID_ATTR: ["style"] });
      const doc = new DOMParser().parseFromString(clean, "text/html");
      doc.querySelectorAll("input").forEach((i) => (i.type === "checkbox" ? (i.disabled = true) : i.remove()));
      html = doc.body.innerHTML;
    });
  });

  // Headings get ids, so links like #setup work.
  $effect(() => {
    html;
    box?.querySelectorAll("h1, h2, h3, h4, h5, h6").forEach((h) => (h.id = slug(h.textContent ?? "")));
  });

  function onClick(e: MouseEvent) {
    const a = (e.target as Element).closest("a");
    const href = a?.getAttribute("href");
    if (!href) return;
    e.preventDefault();
    if (href.startsWith("#")) box?.querySelector(`[id="${CSS.escape(decodeURIComponent(href.slice(1)))}"]`)?.scrollIntoView({ block: "start" });
    else if (/^(https?|mailto):/i.test(href)) openUrl(href);
    else onLink(href);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="mdview" bind:this={box} onclick={onClick}>
  <article class="md">{@html html}</article>
</div>

<style>
  .mdview {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    padding: 24px 32px 60px;
    user-select: text;
  }
  .md {
    max-width: 860px;
    margin: 0 auto;
    line-height: 1.65;
    font-size: 14px;
    overflow-wrap: anywhere;
  }
  .md :global(h1),
  .md :global(h2) {
    border-bottom: 1px solid var(--line);
    padding-bottom: 6px;
  }
  .md :global(h1) {
    font-size: 24px;
    margin: 0 0 16px;
  }
  .md :global(h2) {
    font-size: 19px;
    margin: 28px 0 12px;
  }
  .md :global(h3) {
    font-size: 16px;
    margin: 22px 0 10px;
  }
  .md :global(pre) {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
    overflow-x: auto;
    font: 12.5px/1.5 var(--mono);
  }
  .md :global(code) {
    font-family: var(--mono);
    font-size: 12.5px;
    background: var(--panel);
    padding: 1px 4px;
    border-radius: 4px;
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
  }
  .md :global(a) {
    color: var(--blue);
  }
  .md :global(img) {
    max-width: 100%;
    border-radius: var(--radius);
  }
  .md :global(blockquote) {
    margin: 0;
    padding-left: 12px;
    border-left: 3px solid var(--line-strong);
    color: var(--muted);
  }
  .md :global(table) {
    border-collapse: collapse;
    margin: 12px 0;
    display: block;
    overflow-x: auto;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--line);
    padding: 5px 10px;
    text-align: left;
  }
  .md :global(th) {
    background: var(--panel);
  }
  .md :global(hr) {
    border: none;
    border-top: 1px solid var(--line);
    margin: 24px 0;
  }
  .md :global(li > input[type="checkbox"]) {
    accent-color: var(--amber);
    margin-right: 6px;
  }
</style>
