<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { api, openUrl } from "../lib/api";
  import { agoIso } from "../lib/format";
  import { copyIssueForAgent } from "../lib/menus";
  import type { IssueDetail } from "../lib/types";
  import { hover } from "../lib/hover";
  import Icon from "./Icon.svelte";

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

  /** Goes up on "Try again", so the effect below loads again. */
  let attempt = $state(0);

  $effect(() => {
    const [r, n] = [repo, number];
    attempt;
    detail = null;
    error = "";
    api
      .issueDetail(r, n)
      .then(async (d) => {
        if (r !== repo || n !== number) return;
        detail = d;
        html = { body: await render(d.body), comments: await Promise.all(d.comments.map((c) => render(c.body))) };
      })
      .catch((e) => {
        if (r === repo && n === number) error = String(e);
      });
  });

  // Links open in the browser, not inside Tern.
  function onClick(e: MouseEvent) {
    const a = (e.target as Element).closest("a");
    if (!a?.href) return;
    e.preventDefault();
    if (/^https?:/.test(a.href)) openUrl(a.href);
  }

  /** The narrow-window menu with the toolbar's actions. */
  function moreMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const d = detail;
    app.showMenu({ x: r.left, y: r.bottom + 4 }, `Issue #${number}`, [
      { label: "Copy for agent", run: () => copyIssueForAgent(number) },
      { label: "Copy link", disabled: !d, run: () => d && app.copyValue(d.issue.url) },
      { label: "Open in browser", disabled: !d, run: () => d && openUrl(d.issue.url) },
    ]);
  }

  function labelStyle(color: string) {
    return `--lc:${/^[0-9a-f]{6}$/i.test(color) ? "#" + color : "var(--faint)"}`;
  }
</script>

<div class="issue">
  <header class="toolbar">
    <span class="title"><span class="num mono">#{number}</span> {detail?.issue.title ?? ""}</span>
    {#if detail}<span class="chip st" class:closed={detail.issue.state !== "open"}>{detail.issue.state}</span>{/if}
    <span class="grow"></span>
    <button class="btn sm ghost lo" onclick={() => copyIssueForAgent(number)}>Copy for agent</button>
    <button class="btn sm ghost lo" disabled={!detail} onclick={() => detail && app.copyValue(detail.issue.url)}>Copy link</button>
    <button class="btn sm ghost lo" disabled={!detail} onclick={() => detail && openUrl(detail.issue.url)}>Open in browser</button>
    <button class="icon-btn sm more" onclick={moreMenu} aria-label="More actions" aria-haspopup="menu"
      use:hover={() => ({ title: "More actions", text: "Copy for agent, Copy link, Open in browser" })}><Icon name="more" /></button>
  </header>
  <!-- Catches clicks on links inside the issue text, so they open in the browser. -->
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="scroll" onclick={onClick}>
    {#if error}
      <div class="state">
        <span class="state-title">Could not load issue #{number}</span>
        <pre>{error}</pre>
        <div class="actions"><button class="btn" onclick={() => attempt++}>Try again</button></div>
      </div>
    {:else if !detail}
      <div class="state delay"><p>Loading issue #{number}…</p></div>
    {:else}
      {@const i = detail.issue}
      <div class="head">
        <h1><span class="num mono">#{i.number}</span> {i.title}</h1>
        <div class="meta">
          <span>Opened by <strong>{i.author}</strong> {agoIso(i.created)}</span>
          {#if i.assignees.length}<span>Assigned to {i.assignees.join(", ")}</span>{/if}
          {#each i.labels as l}<span class="label" style={labelStyle(l.color)}>{l.name}</span>{/each}
        </div>
      </div>
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
</div>

<style>
  .issue {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .toolbar {
    container: issuebar / inline-size;
  }
  .toolbar > :global(*) {
    flex-shrink: 0;
  }
  .toolbar > .title {
    flex-shrink: 1;
  }
  .more {
    display: none;
  }
  @container issuebar (max-width: 640px) {
    .lo {
      display: none;
    }
    .more {
      display: inline-flex;
    }
  }
  .scroll {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: var(--s5) var(--s6) 60px;
    user-select: text;
  }
  .scroll > * {
    flex-shrink: 0;
  }
  .num {
    color: var(--muted);
    font-weight: 400;
  }
  .chip.st {
    text-transform: capitalize;
    color: var(--add);
    border-color: color-mix(in srgb, var(--add) 55%, transparent);
  }
  .chip.st.closed {
    color: var(--syn-keyword);
    border-color: color-mix(in srgb, var(--syn-keyword) 55%, transparent);
  }
  .state.delay {
    animation: tern-fade-in 150ms var(--ease-out) 300ms both;
  }
  .head {
    max-width: 820px;
    padding-bottom: var(--s4);
    border-bottom: 1px solid var(--line);
  }
  h1 {
    font-size: var(--fs-xl);
    font-weight: 650;
    margin: 0 0 var(--s3);
    line-height: 1.3;
  }
  h1 .num {
    font-size: var(--fs-lg);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s3) var(--s4);
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .meta strong {
    color: var(--text);
    font-weight: 600;
  }
  .label {
    font-size: var(--fs-xs);
    line-height: 16px;
    padding: 0 7px;
    border-radius: 8px;
    color: var(--text);
    background: color-mix(in srgb, var(--lc) 28%, transparent);
    border: 1px solid color-mix(in srgb, var(--lc) 55%, transparent);
  }
  .md {
    max-width: 820px;
    line-height: 1.6;
    font-size: var(--fs-md);
    overflow-wrap: anywhere;
  }
  article.md {
    padding-top: var(--s4);
  }
  .md :global(pre) {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
    overflow-x: auto;
    font: var(--fs-sm) / 1.5 var(--mono);
  }
  .md :global(code) {
    font-family: var(--mono);
    font-size: var(--fs-sm);
    background: var(--panel);
    padding: 1px 4px;
    border-radius: var(--r-sm);
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
    padding-left: var(--s4);
    border-left: 3px solid var(--line-strong);
    color: var(--muted);
  }
  .comment {
    max-width: 820px;
    margin-top: var(--s5);
    padding-top: var(--s4);
    border-top: 1px solid var(--line);
  }
  .who {
    margin: 0 0 var(--s1);
    font-size: var(--fs-sm);
  }
  /* Cells wrap only at spaces and hyphens, so short columns like "ID" never break into letters. */
  .md :global(th),
  .md :global(td) {
    overflow-wrap: break-word;
    word-break: normal;
    min-width: 2.5em;
  }
</style>
