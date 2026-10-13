<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api, openUrl } from "../lib/api";
  import { agoIso } from "../lib/format";
  import { hover } from "../lib/hover";
  import type { Ticket, TicketDetail } from "../lib/types";

  let filter = $state("");
  let source = $state<"all" | "azdo" | "github">("all");
  let listEl: HTMLDivElement | undefined = $state();
  let filterEl: HTMLInputElement | undefined = $state();

  // In progress first, then the work that is waiting, then the rest.
  const stateRank = (s: string) =>
    /progress|active|doing|committed|review|testing|blocked/i.test(s) ? 0 : /new|to ?do|proposed|approved|open|ready|backlog/i.test(s) ? 1 : 2;

  const sources = $derived({
    azdo: app.tickets.items.filter((t) => t.provider === "azdo").length,
    github: app.tickets.items.filter((t) => t.provider === "github").length,
  });
  const shown = $derived(
    app.tickets.items.filter((t) => {
      if (source !== "all" && t.provider !== source) return false;
      const q = filter.trim().toLowerCase();
      if (!q) return true;
      return `${t.number} ${t.title} ${t.kind} ${t.state} ${t.project} ${t.area} ${t.tags.join(" ")}`.toLowerCase().includes(q);
    }),
  );
  const groups = $derived.by(() => {
    const m = new Map<string, Ticket[]>();
    for (const t of shown) m.set(t.state, [...(m.get(t.state) ?? []), t]);
    return [...m.entries()].sort((a, b) => stateRank(a[0]) - stateRank(b[0]) || a[0].localeCompare(b[0]));
  });
  /** The list in screen order, for the arrow keys. */
  const flat = $derived(groups.flatMap(([, ts]) => ts));
  const sel = $derived(app.tickets.items.find((t) => t.key === app.ticketSel) ?? null);
  const detail = $derived(sel ? app.ticketDetails[sel.key] : undefined);

  // Focus the list when the view opens, so the keys work at once.
  $effect(() => {
    if (listEl) tick().then(() => listEl?.focus());
  });

  // Fresh data while the view is open: every 5 minutes.
  $effect(() => {
    const t = setInterval(() => app.loadTickets(true), 300_000);
    return () => clearInterval(t);
  });

  function move(d: number) {
    if (!flat.length) return;
    const i = flat.findIndex((t) => t.key === app.ticketSel);
    const next = flat[Math.max(0, Math.min(flat.length - 1, i < 0 ? 0 : i + d))];
    app.selectTicket(next.key);
    tick().then(() => listEl?.querySelector(`[data-key="${CSS.escape(next.key)}"]`)?.scrollIntoView({ block: "nearest" }));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target === filterEl) {
      if (e.key === "ArrowDown" || e.key === "Enter") {
        e.preventDefault();
        listEl?.focus();
        if (e.key === "ArrowDown") move(1);
      } else if (e.key === "Escape" && filter) {
        e.preventDefault();
        filter = "";
      }
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const k = e.key;
    if (k === "ArrowDown" || k === "j") move(1);
    else if (k === "ArrowUp" || k === "k") move(-1);
    else if (k === "Enter" && sel) openUrl(sel.url);
    else if (k === "c" && sel) app.copyTicketForAgent(sel);
    else if (k === "b" && sel) app.copyValue(app.ticketBranch(sel), "the branch name");
    else if (k === "/") filterEl?.focus();
    else if (k === "Escape") app.closeTickets();
    else return;
    e.preventDefault();
  }

  // Ticket text comes from other people, so it is cleaned before it is shown.
  // Images need a sign-in the window does not have. Each one keeps its address in data-src,
  // and the back end downloads it with your az or gh sign-in (see loadImages).
  async function render(body: string, markdown: boolean) {
    const [{ marked }, { default: DOMPurify }] = await Promise.all([import("marked"), import("dompurify")]);
    const raw = markdown ? await marked.parse(body || "", { gfm: true, breaks: true }) : body;
    const clean = DOMPurify.sanitize(raw, { FORBID_TAGS: ["style", "form", "input", "button", "iframe", "select", "textarea"], FORBID_ATTR: ["style"] });
    const doc = new DOMParser().parseFromString(clean, "text/html");
    doc.querySelectorAll("img").forEach((img) => {
      const src = img.getAttribute("src") ?? "";
      img.removeAttribute("src");
      img.removeAttribute("width");
      img.removeAttribute("height");
      if (!/^https:\/\//i.test(src)) return img.remove();
      img.dataset.src = src;
      img.className = "timg";
      img.alt = img.alt || decodeURIComponent(src.match(/fileName=([^&]+)/i)?.[1] ?? "") || "Image";
    });
    return doc.body.innerHTML;
  }

  let detailEl: HTMLElement | undefined = $state();
  /** Downloaded images by address, shared by every ticket while Tern is open. */
  const images = new Map<string, Promise<string>>();
  /** The image open full-size, or null. */
  let zoom = $state<{ src: string; alt: string } | null>(null);

  function loadImages() {
    for (const img of detailEl?.querySelectorAll<HTMLImageElement>("img.timg[data-src]:not([data-state])") ?? []) {
      const url = img.dataset.src!;
      img.dataset.state = "loading";
      if (!images.has(url)) images.set(url, api.ticketImage(url));
      images.get(url)!.then(
        (data) => {
          img.src = data;
          img.dataset.state = "ready";
        },
        (e) => {
          images.delete(url); // try again next time
          const a = document.createElement("a");
          a.href = url;
          a.className = "img-link";
          a.textContent = `${img.alt}: could not load. Open it in the browser.`;
          a.title = String(e);
          img.replaceWith(a);
        },
      );
    }
  }

  let html = $state<{ key: string; sections: string[]; comments: string[] } | null>(null);
  $effect(() => {
    const d: TicketDetail | null | undefined = detail?.data;
    const key = sel?.key;
    if (!d || !key) return;
    Promise.all([Promise.all(d.sections.map((s) => render(s.body, s.markdown))), Promise.all(d.comments.map((c) => render(c.body, c.markdown)))]).then(
      ([sections, comments]) => {
        if (sel?.key === key) html = { key, sections, comments };
      },
    );
  });

  // Load the images once the ticket's text is on screen.
  $effect(() => {
    html;
    tick().then(loadImages);
  });

  function onBodyClick(e: MouseEvent) {
    const img = (e.target as Element).closest("img.timg") as HTMLImageElement | null;
    if (img?.dataset.state === "ready") {
      e.preventDefault();
      zoom = { src: img.src, alt: img.alt };
      return;
    }
    const a = (e.target as Element).closest("a");
    if (!a?.href) return;
    e.preventDefault();
    if (/^https?:/.test(a.href)) openUrl(a.href);
  }

  const kindTone = (k: string) =>
    /bug/i.test(k) ? "bug" : /story/i.test(k) ? "story" : /feature|epic/i.test(k) ? "feature" : /issue/i.test(k) ? "issue" : "task";
  const providerName = (t: Ticket) => (t.provider === "azdo" ? "Azure DevOps" : "GitHub");
</script>

{#snippet kindIcon(kind: string)}
  {@const tone = kindTone(kind)}
  <svg class="kind k-{tone}" viewBox="0 0 16 16" width="14" height="14" aria-label={kind}>
    {#if tone === "bug"}
      <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-width="1.6" /><circle cx="8" cy="8" r="2" fill="currentColor" />
    {:else if tone === "story"}
      <path d="M4.5 2.5h7v11L8 11l-3.5 2.5z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
    {:else if tone === "feature"}
      <path d="M8 2.2 13.8 8 8 13.8 2.2 8z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
    {:else if tone === "issue"}
      <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-width="1.6" /><circle cx="8" cy="8" r="1.3" fill="currentColor" />
    {:else}
      <rect x="2.75" y="2.75" width="10.5" height="10.5" rx="2.5" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="m5.5 8.2 1.8 1.8 3.3-3.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
    {/if}
  </svg>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="tickets" aria-label="My tickets" onkeydown={onKey}>
  <div class="list-side">
    <header class="lhead">
      <div class="titleline">
        <h2>My tickets</h2>
        <span class="count mono">{app.tickets.items.length}</span>
        <span class="grow"></span>
        {#if app.tickets.at}<span class="faint when">updated {agoIso(new Date(app.tickets.at).toISOString())}</span>{/if}
        <button class="icon" class:spin={app.tickets.loading} onclick={() => app.loadTickets(true)} aria-label="Refresh tickets"
          use:hover={() => ({ title: app.tickets.loading ? "Loading…" : "Refresh", text: "Tickets also refresh every 5 minutes while this view is open." })}>
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v2.8h-2.8" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
      </div>
      <input class="filter" bind:this={filterEl} bind:value={filter} placeholder="Filter by title, number, tag, or project  ( / )" spellcheck="false" aria-label="Filter tickets" />
      {#if sources.azdo && sources.github}
        <div class="chips" role="group" aria-label="Source">
          {#each [["all", "All", sources.azdo + sources.github], ["azdo", "Azure DevOps", sources.azdo], ["github", "GitHub", sources.github]] as [id, label, n]}
            <button class="chip" class:on={source === id} onclick={() => (source = id as typeof source)}>{label} <span class="faint">{n}</span></button>
          {/each}
        </div>
      {/if}
    </header>

    <div class="list" bind:this={listEl} tabindex="0" role="listbox" aria-label="Tickets" aria-activedescendant={sel ? "t-" + sel.number : undefined}>
      {#each app.tickets.errors as er}
        <div class="err">
          <b>{er.source}</b>
          <p class="mono">{er.message}</p>
          <p>{er.source.startsWith("GitHub") ? "Check that gh is signed in: run gh auth login in a terminal." : "Check that az is signed in: run az login in a terminal."}</p>
        </div>
      {/each}
      {#if app.tickets.loading && !app.tickets.items.length}
        {#each [0, 1, 2, 3] as i}
          <div class="skel" style:--d="{i * 80}ms"><span></span><span></span></div>
        {/each}
      {:else if !app.tickets.items.length && !app.tickets.errors.length && app.tickets.at}
        <div class="empty">
          <p>No open tickets are assigned to you.</p>
          <p class="faint">Tern looks for Azure DevOps work items in the organizations your repos use, and GitHub issues assigned to you.</p>
        </div>
      {:else if app.tickets.items.length && !shown.length}
        <p class="empty faint">No tickets match “{filter}”.</p>
      {/if}

      {#each groups as [state, ts] (state)}
        <div class="group"><span class="dot r{stateRank(state)}"></span>{state}<span class="faint">{ts.length}</span></div>
        {#each ts as t (t.key)}
          <button class="row" class:on={t.key === app.ticketSel} data-key={t.key} id="t-{t.number}" role="option" aria-selected={t.key === app.ticketSel}
            onclick={() => app.selectTicket(t.key)} ondblclick={() => openUrl(t.url)} tabindex="-1">
            {@render kindIcon(t.kind)}
            <span class="rmain">
              <span class="rtitle">{t.title}</span>
              <span class="rmeta">
                <span class="mono num">#{t.number}</span>
                {#if t.priority != null}<span class="prio p{Math.min(t.priority, 4)}">P{t.priority}</span>{/if}
                <span class="faint">{t.project}{t.iteration ? ` · ${t.iteration}` : ""}</span>
                {#each t.tags.slice(0, 2) as tag}<span class="tag">{tag}</span>{/each}
              </span>
            </span>
            <span class="age faint">{agoIso(t.changed)}</span>
          </button>
        {/each}
      {/each}
    </div>
    <footer class="keys faint">
      <span><kbd>↑</kbd><kbd>↓</kbd> move</span><span><kbd>Enter</kbd> open</span><span><kbd>C</kbd> copy for agent</span><span><kbd>B</kbd> branch name</span>
    </footer>
  </div>

  <div class="sep" aria-hidden="true"></div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <article class="detail" bind:this={detailEl} onclick={onBodyClick}>
    {#if !sel}
      <div class="none">
        {#if app.tickets.loading}<p class="faint">Loading your tickets…</p>{:else}<p class="faint">Pick a ticket on the left.</p>{/if}
      </div>
    {:else}
      {#key sel.key}
        <div class="dwrap">
          <div class="dtop">
            <span class="pill kindpill k-{kindTone(sel.kind)}">{@render kindIcon(sel.kind)}{sel.kind}</span>
            <button class="pill mono idpill" onclick={() => app.copyValue(String(sel.number), `#${sel.number}`)} title="Copy the number">#{sel.number}</button>
            <span class="pill statepill r{stateRank(sel.state)}"><span class="dot r{stateRank(sel.state)}"></span>{sel.state}</span>
            {#if sel.priority != null}<span class="pill prio p{Math.min(sel.priority, 4)}">Priority {sel.priority}</span>{/if}
            <span class="grow"></span>
            <span class="faint src">{providerName(sel)}</span>
          </div>
          <h1>{sel.title}</h1>
          <dl class="facts">
            <dt>Project</dt><dd>{sel.project}{sel.area && sel.area !== sel.project ? ` / ${sel.area}` : ""}</dd>
            {#if sel.iteration}<dt>Sprint</dt><dd>{sel.iteration}</dd>{/if}
            {#if detail?.data?.assignedTo}<dt>Assigned</dt><dd>{detail.data.assignedTo}</dd>{/if}
            {#if detail?.data?.createdBy}<dt>Created</dt><dd>{detail.data.createdBy}, {agoIso(detail.data.created)}</dd>{/if}
            <dt>Updated</dt><dd>{agoIso(sel.changed)}</dd>
            {#if sel.tags.length}<dt>Tags</dt><dd class="tags">{#each sel.tags as tag}<span class="tag">{tag}</span>{/each}</dd>{/if}
          </dl>
          <div class="actions">
            <button class="btn primary" onclick={() => openUrl(sel.url)}>Open in {providerName(sel)}</button>
            <button class="btn" onclick={() => app.copyTicketForAgent(sel)} disabled={!detail?.data}>Copy for an agent</button>
            <button class="btn" onclick={() => app.copyValue(app.ticketBranch(sel), "the branch name")}
              use:hover={() => ({ title: "Copy branch name", text: app.ticketBranch(sel) })}>Copy branch name</button>
            <button class="btn" onclick={() => app.copyValue(sel.url)}>Copy link</button>
          </div>

          {#if detail?.loading && !detail.data}
            <div class="skel big"><span></span><span></span><span></span></div>
          {:else if detail?.error}
            <p class="derr mono">{detail.error}</p>
            <button class="link" onclick={() => app.loadTicketDetail(sel, true)}>Try again</button>
          {:else if detail?.data}
            {@const d = detail.data}
            {#if !d.sections.length}<p class="faint">This ticket has no description.</p>{/if}
            {#each d.sections as s, i}
              <section class="sec">
                <h3>{s.title}</h3>
                <div class="md">{@html html?.key === sel.key ? (html.sections[i] ?? "") : ""}</div>
              </section>
            {/each}
            {#if d.links.length}
              <section class="sec">
                <h3>Links</h3>
                <ul class="links">
                  {#each d.links as l}
                    <li>
                      <span class="lk lk-{l.kind}">{l.kind === "commit" ? "◆" : l.kind === "pr" ? "⇄" : l.kind === "branch" ? "⑂" : l.kind === "parent" ? "↑" : l.kind === "child" ? "↓" : "↔"}</span>
                      {#if l.url}<a href={l.url}>{l.label}</a>{:else}<span>{l.label}</span>{/if}
                    </li>
                  {/each}
                </ul>
              </section>
            {/if}
            {#if d.comments.length}
              <section class="sec">
                <h3>Comments <span class="faint">{d.comments.length}</span></h3>
                {#each d.comments as c, i}
                  <div class="comment">
                    <div class="chead"><b>{c.author}</b><span class="faint">{agoIso(c.created)}</span></div>
                    <div class="md">{@html html?.key === sel.key ? (html.comments[i] ?? "") : ""}</div>
                  </div>
                {/each}
              </section>
            {/if}
          {/if}
        </div>
      {/key}
    {/if}
  </article>
</section>

<svelte:window onkeydown={(e) => zoom && e.key === "Escape" && (e.preventDefault(), (zoom = null))} />

{#if zoom}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="zoom fx-fade" onclick={() => (zoom = null)}>
    <img src={zoom.src} alt={zoom.alt} />
    <p class="zcap">{zoom.alt} <span>Click or press Esc to close</span></p>
  </div>
{/if}

<style>
  .tickets {
    grid-column: 3 / -1;
    grid-row: 2;
    display: grid;
    grid-template-columns: minmax(320px, 400px) 1px minmax(0, 1fr);
    min-height: 0;
    min-width: 0;
    background: var(--bg);
  }
  .list-side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--panel);
  }
  .sep {
    background: var(--line);
  }
  .lhead {
    padding: 12px 12px 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-bottom: 1px solid var(--line);
  }
  .titleline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 650;
  }
  .count {
    font-size: 11px;
    color: var(--muted);
    background: var(--raised);
    border-radius: 10px;
    padding: 0 7px;
    line-height: 18px;
  }
  .grow {
    flex: 1;
  }
  .when {
    font-size: 11.5px;
  }
  .icon {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: var(--radius);
    color: var(--muted);
  }
  .icon:hover {
    background: var(--hover);
    color: var(--text);
  }
  .spin svg {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .filter {
    height: 30px;
    padding: 0 10px;
    border-radius: 7px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    outline: none;
  }
  .filter:focus {
    border-color: var(--amber);
  }
  .filter::placeholder {
    color: var(--muted);
  }
  .chips {
    display: flex;
    gap: 4px;
  }
  .chip {
    font-size: 12px;
    padding: 2px 9px;
    border-radius: 12px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .chip.on {
    color: var(--text);
    background: var(--raised);
    border-color: var(--line-strong);
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 6px 10px;
    outline: none;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 12px 8px 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--faint);
  }
  .dot.r0 {
    background: var(--blue);
  }
  .dot.r1 {
    background: var(--amber);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 7px 8px;
    border-radius: 7px;
    text-align: left;
    color: var(--text);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.on {
    background: var(--raised);
  }
  .list:focus-visible .row.on {
    box-shadow: inset 0 0 0 1px var(--amber);
  }
  .kind {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .k-bug {
    color: var(--del);
  }
  .k-task {
    color: var(--amber);
  }
  .k-story {
    color: var(--blue);
  }
  .k-feature {
    color: var(--syn-keyword);
  }
  .k-issue {
    color: var(--add);
  }
  .rmain {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .rtitle {
    font-size: 13px;
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .rmeta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .num {
    color: var(--muted);
    font-size: 11px;
  }
  .prio {
    font-size: 10.5px;
    font-weight: 600;
    padding: 0 5px;
    border-radius: 4px;
    line-height: 16px;
    color: var(--muted);
    background: var(--raised);
  }
  .prio.p1 {
    color: var(--del);
    background: var(--del-bg);
  }
  .prio.p2 {
    color: var(--amber);
    background: var(--amber-soft);
  }
  .tag {
    font-size: 10.5px;
    padding: 0 6px;
    border-radius: 4px;
    line-height: 16px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .age {
    font-size: 11px;
    flex-shrink: 0;
    margin-top: 2px;
  }
  .err {
    margin: 8px 4px;
    padding: 10px 12px;
    border-radius: 7px;
    background: var(--del-bg);
    font-size: 12px;
  }
  .err b {
    color: var(--del);
  }
  .err p {
    margin: 4px 0 0;
  }
  .err .mono {
    font-size: 11.5px;
    word-break: break-word;
  }
  .empty {
    padding: 24px 12px;
    font-size: 13px;
  }
  .empty p {
    margin: 0 0 6px;
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    padding: 7px 12px;
    border-top: 1px solid var(--line);
    font-size: 11px;
  }
  .keys kbd {
    margin-right: 3px;
  }

  /* Loading: grey bars in the shape of the content, not a spinner. */
  .skel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 10px;
  }
  .skel span {
    height: 10px;
    border-radius: 4px;
    background: var(--raised);
    animation: fade 1.2s ease-in-out infinite;
    animation-delay: var(--d, 0ms);
  }
  .skel span:nth-child(1) {
    width: 85%;
  }
  .skel span:nth-child(2) {
    width: 45%;
  }
  .skel.big {
    padding: 8px 0;
    gap: 10px;
  }
  .skel.big span:nth-child(3) {
    width: 70%;
  }
  @keyframes fade {
    50% {
      opacity: 0.45;
    }
  }

  .detail {
    min-width: 0;
    overflow-y: auto;
    user-select: text;
  }
  .none {
    height: 100%;
    display: grid;
    place-items: center;
  }
  .dwrap {
    max-width: 820px;
    padding: 22px 32px 48px;
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .dtop {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    line-height: 20px;
    padding: 0 8px;
    border-radius: 6px;
    background: var(--raised);
    color: var(--text);
  }
  .kindpill :global(svg) {
    width: 12px;
    height: 12px;
  }
  .idpill {
    color: var(--muted);
  }
  .idpill:hover {
    color: var(--amber);
  }
  .src {
    font-size: 11.5px;
  }
  h1 {
    margin: 12px 0 14px;
    font-size: 19px;
    line-height: 1.35;
    font-weight: 650;
    text-wrap: balance;
  }
  .facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 5px 18px;
    margin: 0 0 16px;
    font-size: 12.5px;
  }
  .facts dt {
    color: var(--muted);
  }
  .facts dd {
    margin: 0;
  }
  .facts .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--line);
  }
  .btn {
    font-size: 12px;
    padding: 4px 11px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .btn:disabled {
    opacity: 0.45;
  }
  .btn.primary {
    color: var(--on-accent);
    background: var(--amber);
    border-color: var(--amber);
  }
  .btn.primary:hover {
    color: var(--on-accent);
  }
  .sec {
    margin-top: 20px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 12.5px;
    font-weight: 650;
    color: var(--muted);
  }
  .derr {
    color: var(--del);
    font-size: 12px;
    margin: 16px 0 6px;
  }
  .links {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12.5px;
  }
  .links li {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .lk {
    width: 14px;
    text-align: center;
    color: var(--muted);
  }
  .links a {
    color: var(--blue);
  }
  .comment {
    padding: 10px 0;
    border-top: 1px solid var(--line);
  }
  .comment:first-of-type {
    border-top: 0;
  }
  .chead {
    display: flex;
    gap: 8px;
    font-size: 12px;
    margin-bottom: 4px;
  }
  .md {
    font-size: 13.5px;
    line-height: 1.6;
    overflow-wrap: anywhere;
    max-width: 72ch;
  }
  .md :global(p),
  .md :global(div) {
    margin: 0 0 6px;
  }
  /* Headings inside a ticket stay below the ticket's own title. */
  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4) {
    margin: 14px 0 6px;
    font-weight: 650;
    line-height: 1.35;
    color: var(--text);
  }
  .md :global(h1) {
    font-size: 15px;
  }
  .md :global(h2) {
    font-size: 14px;
  }
  .md :global(h3),
  .md :global(h4) {
    font-size: 13.5px;
  }
  .md :global(:first-child) {
    margin-top: 0;
  }
  .md :global(ul),
  .md :global(ol) {
    margin: 4px 0 8px;
    padding-left: 22px;
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
  .md :global(img.timg) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 8px 0;
    border-radius: 8px;
    border: 1px solid var(--line);
    cursor: zoom-in;
  }
  /* While an image downloads: a quiet placeholder the size of a small screenshot. */
  .md :global(img.timg[data-state="loading"]) {
    width: 240px;
    height: 140px;
    background: var(--raised);
    animation: fade 1.2s ease-in-out infinite;
    cursor: progress;
  }
  .md :global(img.timg[data-state="ready"]) {
    animation: tern-fade-in 180ms var(--ease-out);
  }
  .zoom {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 48px;
    background: rgb(0 0 0 / 0.72);
    cursor: zoom-out;
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .zoom img {
    max-width: 100%;
    max-height: calc(100% - 40px);
    object-fit: contain;
    border-radius: 8px;
    animation: tern-dialog-in 200ms var(--ease-out);
  }
  .zcap {
    margin: 0;
    color: #e6e8ec;
    font-size: 12.5px;
  }
  .zcap span {
    color: #9aa1ad;
    margin-left: 10px;
  }
  .md :global(.img-link) {
    display: inline-block;
    font-size: 12px;
    padding: 2px 8px;
    border: 1px dashed var(--line-strong);
    border-radius: 5px;
    text-decoration: none;
  }
  .md :global(table) {
    border-collapse: collapse;
  }
  .md :global(td),
  .md :global(th) {
    border: 1px solid var(--line);
    padding: 4px 8px;
  }
  .md :global(blockquote) {
    margin: 0;
    padding-left: 12px;
    border-left: 3px solid var(--line-strong);
    color: var(--muted);
  }

  @media (prefers-reduced-motion: reduce) {
    .skel span {
      animation: none;
    }
  }
</style>
