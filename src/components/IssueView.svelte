<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { api, openUrl } from "../lib/api";
  import { agoIso } from "../lib/format";
  import { copyIssueForAgent } from "../lib/menus";
  import type { IssueDetail } from "../lib/types";

  let { repo, number }: { repo: string; number: number } = $props();

  let detail = $state<IssueDetail | null>(null);
  let error = $state("");
  let html = $state<{ body: string; comments: string[] }>({ body: "", comments: [] });

  // Issue text comes from other people, so it is cleaned before it is shown.
  async function render(markdown: string) {
    const [{ marked }, { default: DOMPurify }] = await Promise.all([import("marked"), import("dompurify")]);
    const raw = await marked.parse(markdown || "", { gfm: true, breaks: true });
    return DOMPurify.sanitize(raw, { FORBID_TAGS: ["style", "form", "input", "button", "iframe"], FORBID_ATTR: ["style"] });
  }

  $effect(() => {
    const [r, n] = [repo, number];
    detail = null;
    error = "";
    api
      .issueDetail(r, n)
      .then(async (d) => {
        if (r !== repo || n !== number) return;
        detail = d;
        html = { body: await render(d.body), comments: await Promise.all(d.comments.map((c) => render(c.body))) };
      })
      .catch((e) => (error = String(e)));
  });

  // Links open in the browser, not inside Tern.
  function onClick(e: MouseEvent) {
    const a = (e.target as Element).closest("a");
    if (!a?.href) return;
    e.preventDefault();
    if (/^https?:/.test(a.href)) openUrl(a.href);
  }

  function labelStyle(color: string) {
    return `--lc:${/^[0-9a-f]{6}$/i.test(color) ? "#" + color : "var(--faint)"}`;
  }
</script>

<!-- Catches clicks on links inside the issue text, so they open in the browser. -->
<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="issue" onclick={onClick}>
  {#if error}
    <p class="err">Could not load issue #{number}.</p>
    <pre class="mono">{error}</pre>
  {:else if !detail}
    <p class="muted">Loading issue #{number}…</p>
  {:else}
    {@const i = detail.issue}
    <header>
      <h1><span class="num mono">#{i.number}</span> {i.title}</h1>
      <div class="meta">
        <span class="state" class:closed={i.state !== "open"}>{i.state}</span>
        <span>Opened by <strong>{i.author}</strong> {agoIso(i.created)}</span>
        {#if i.assignees.length}<span>Assigned to {i.assignees.join(", ")}</span>{/if}
        {#each i.labels as l}<span class="label" style={labelStyle(l.color)}>{l.name}</span>{/each}
      </div>
      <div class="actions">
        <button class="btn" onclick={() => copyIssueForAgent(i.number)}>Copy for an agent</button>
        <button class="btn" onclick={() => app.copyValue(i.url)}>Copy link</button>
        <button class="btn" onclick={() => openUrl(i.url)}>Open in the browser</button>
      </div>
    </header>
    <article class="md">
      {#if detail.body.trim()}{@html html.body}{:else}<p class="muted">No description.</p>{/if}
    </article>
    {#each detail.comments as c, k}
      <section class="comment">
        <p class="who"><strong>{c.author}</strong> <span class="faint">{agoIso(c.created)}</span></p>
        <div class="md">{@html html.comments[k] ?? ""}</div>
      </section>
    {/each}
  {/if}
</div>

<style>
  .issue {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    padding: 22px 28px 60px;
    user-select: text;
  }
  header {
    max-width: 820px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    font-size: 18px;
    font-weight: 650;
    margin: 0 0 8px;
    line-height: 1.3;
  }
  .num {
    color: var(--muted);
    font-weight: 400;
    font-size: 16px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    color: var(--muted);
    font-size: 12.5px;
  }
  .meta strong {
    color: var(--text);
    font-weight: 600;
  }
  .state {
    text-transform: capitalize;
    color: var(--add);
    border: 1px solid var(--add);
    border-radius: 10px;
    padding: 0 8px;
    font-size: 12px;
  }
  .state.closed {
    color: var(--syn-keyword);
    border-color: var(--syn-keyword);
  }
  .label {
    font-size: 11px;
    line-height: 16px;
    padding: 0 7px;
    border-radius: 8px;
    color: var(--text);
    background: color-mix(in srgb, var(--lc) 28%, transparent);
    border: 1px solid color-mix(in srgb, var(--lc) 55%, transparent);
  }
  .actions {
    display: flex;
    gap: 6px;
    margin-top: 12px;
  }
  .btn {
    font-size: 12px;
    padding: 3px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn:hover {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .md {
    max-width: 820px;
    line-height: 1.6;
    font-size: 13.5px;
    overflow-wrap: anywhere;
  }
  article.md {
    padding-top: 14px;
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
  .comment {
    max-width: 820px;
    margin-top: 18px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .who {
    margin: 0 0 4px;
    font-size: 12.5px;
  }
  .err {
    color: var(--del);
  }
  pre.mono {
    white-space: pre-wrap;
    color: var(--muted);
  }
</style>
