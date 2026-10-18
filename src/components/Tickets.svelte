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
    else if (k === "c" && sel) copyForAgent(sel);
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

  async function copyForAgent(t: Ticket) {
    if (!app.ticketDetails[t.key]?.data) await app.loadTicketDetail(t);
    app.copyTicketForAgent(t);
  }

  function rowMenu(e: MouseEvent, t: Ticket) {
    app.showMenu(e, `#${t.number} ${t.title}`, [
      { label: "Open in browser", hint: "Enter", run: () => openUrl(t.url) },
      "sep",
      { label: "Copy for agent", hint: "C", run: () => copyForAgent(t) },
      { label: "Copy branch name", hint: "B", run: () => app.copyValue(app.ticketBranch(t), "the branch name") },
      { label: "Copy link", run: () => app.copyValue(t.url) },
      { label: `Copy #${t.number}`, run: () => app.copyValue(String(t.number), `#${t.number}`) },
    ]);
  }

  // The list and detail split: drag the line between them. Remembered across restarts.
  const W_MIN = 280;
  const W_MAX = 560;
  const W_KEY = "tern.ticketsW";
  const clampW = (n: number) => Math.max(W_MIN, Math.min(W_MAX, n));
  let listW = $state(
    (() => {
      try {
        const n = Number(localStorage.getItem(W_KEY));
        return n ? clampW(n) : 380;
      } catch {
        return 380;
      }
    })(),
  );
  function saveW() {
    try {
      localStorage.setItem(W_KEY, String(Math.round(listW)));
    } catch {}
  }
  function dragW(e: PointerEvent) {
    const startX = e.clientX;
    const start = listW;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => (listW = clampW(start + ev.clientX - startX));
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      saveW();
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }
  /** The source choice fits next to the filter only when the list is wide. */
  const segInline = $derived(listW >= 440);
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

{#snippet sourceSeg()}
  <div class="seg" role="group" aria-label="Source">
    {#each [["all", "All", sources.azdo + sources.github], ["azdo", "Azure DevOps", sources.azdo], ["github", "GitHub", sources.github]] as [id, label, n]}
      <button class:on={source === id} aria-pressed={source === id} onclick={() => (source = id as typeof source)}>{label} <span class="badge">{n}</span></button>
    {/each}
  </div>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="tickets" aria-label="My tickets" onkeydown={onKey} style:--lw="{listW}px">
  <div class="list-side">
    <div class="toolbar">
      <span class="title">My tickets</span>
      <span class="badge">{app.tickets.items.length}</span>
      <span class="grow"></span>
      {#if app.tickets.at}<span class="meta">updated {agoIso(new Date(app.tickets.at).toISOString())}</span>{/if}
      <button class="icon-btn" class:spin={app.tickets.loading} onclick={() => app.loadTickets(true)} aria-label="Refresh tickets"
        use:hover={() => ({ title: app.tickets.loading ? "Loading…" : "Refresh", text: "Tickets also refresh every 5 minutes while this view is open." })}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v2.8h-2.8" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
    </div>
    <div class="toolbar sub">
      <input class="field grow" bind:this={filterEl} bind:value={filter} placeholder="Filter tickets  /" spellcheck="false" aria-label="Filter tickets" />
      {#if sources.azdo && sources.github && segInline}{@render sourceSeg()}{/if}
    </div>
    {#if sources.azdo && sources.github && !segInline}
      <div class="toolbar sub">{@render sourceSeg()}</div>
    {/if}

    <div class="list" bind:this={listEl} tabindex="0" role="listbox" aria-label="Tickets" aria-activedescendant={sel ? "t-" + sel.number : undefined}>
      {#each app.tickets.errors as er}
        <div class="state inline">
          <span class="state-title">{er.source}</span>
          <pre>{er.message}</pre>
          <p>{er.source.startsWith("GitHub") ? "Check that gh is signed in: run gh auth login in a terminal." : "Check that az is signed in: run az login in a terminal."}</p>
          <div class="actions"><button class="btn sm" onclick={() => app.loadTickets(true)}>Try again</button></div>
        </div>
      {/each}
      {#if app.tickets.loading && !app.tickets.items.length}
        {#each [0, 1, 2, 3, 4] as i}
          <div class="skel-row" aria-hidden="true"><span class="skeleton" style:width="{82 - i * 7}%"></span><span class="skeleton short"></span></div>
        {/each}
      {:else if !app.tickets.items.length && !app.tickets.errors.length && app.tickets.at}
        <div class="state">
          <span class="state-title">No open tickets are assigned to you</span>
          <p>Tern looks for Azure DevOps work items in the organizations your projects use, and GitHub issues assigned to you.</p>
        </div>
      {:else if app.tickets.items.length && !shown.length}
        <div class="state">
          <p>No tickets match “{filter}”.</p>
          <div class="actions"><button class="btn sm" onclick={() => (filter = "")}>Clear the filter</button></div>
        </div>
      {/if}

      {#each groups as [state, ts] (state)}
        <div class="section-label"><span class="dot r{stateRank(state)}"></span>{state}<span class="badge">{ts.length}</span></div>
        {#each ts as t (t.key)}
          <button class="row" class:on={t.key === app.ticketSel} data-key={t.key} id="t-{t.number}" role="option" aria-selected={t.key === app.ticketSel}
            onclick={() => app.selectTicket(t.key)} ondblclick={() => openUrl(t.url)} oncontextmenu={(e) => rowMenu(e, t)} tabindex="-1">
            {@render kindIcon(t.kind)}
            <span class="rmain">
              <span class="rtitle">{t.title}</span>
              <span class="rmeta">
                <span class="mono num">#{t.number}</span>
                {#if t.priority != null}<span class="prio p{Math.min(t.priority, 4)}">P{t.priority}</span>{/if}
                <span class="faint">{t.project}{t.iteration ? ` · ${t.iteration}` : ""}</span>
                {#each t.tags.slice(0, 2) as tag}<span class="chip">{tag}</span>{/each}
              </span>
            </span>
            <span class="age faint">{agoIso(t.changed)}</span>
          </button>
        {/each}
      {/each}
    </div>
    <footer class="keys faint">
      <span><kbd>↑</kbd> <kbd>↓</kbd> move</span><span><kbd>Enter</kbd> open</span><span><kbd>C</kbd> copy for agent</span><span><kbd>B</kbd> branch name</span>
    </footer>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="split" role="separator" aria-orientation="vertical" aria-label="Resize the ticket list" onpointerdown={dragW}
    ondblclick={() => ((listW = 380), saveW())}></div>

  <article class="detail">
    {#if !sel}
      {#if app.tickets.loading}
        <div class="state"><p>Loading your tickets…</p></div>
      {:else}
        <div class="state"><p>Pick a ticket on the left.</p></div>
      {/if}
    {:else}
      {#key sel.key}
        <div class="toolbar dbar">
          <div class="dchips">
            <span class="chip lo k-{kindTone(sel.kind)}">{@render kindIcon(sel.kind)}<span class="ktext">{sel.kind}</span></span>
            <button class="chip mono idchip" onclick={() => app.copyValue(String(sel.number), `#${sel.number}`)}
              use:hover={() => ({ title: `Copy #${sel.number}` })}>#{sel.number}</button>
            <span class="chip lo"><span class="dot r{stateRank(sel.state)}"></span>{sel.state}</span>
            {#if sel.priority != null}<span class="chip lo prio-chip p{Math.min(sel.priority, 4)}">Priority {sel.priority}</span>{/if}
          </div>
          <span class="grow"></span>
          <button class="btn ghost sm" onclick={() => copyForAgent(sel)}
            use:hover={() => ({ title: "Copy for agent", text: "The title, fields, and description as plain text.", hint: "Key: C" })}>Copy for agent</button>
          <button class="btn ghost sm lo2" onclick={() => app.copyValue(app.ticketBranch(sel), "the branch name")}
            use:hover={() => ({ title: "Copy branch name", text: app.ticketBranch(sel), hint: "Key: B" })}>Copy branch name</button>
          <button class="btn ghost sm lo2" onclick={() => app.copyValue(sel.url)}>Copy link</button>
          <button class="btn primary sm" onclick={() => openUrl(sel.url)} use:hover={() => ({ title: `Open in ${providerName(sel)}`, hint: "Key: Enter" })}>Open in {providerName(sel)}</button>
        </div>
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions, a11y_no_static_element_interactions -->
        <div class="dscroll" bind:this={detailEl} onclick={onBodyClick}>
          <div class="dwrap">
            <h1>{sel.title}</h1>
            <dl class="facts">
              <dt>Project</dt><dd>{sel.project}{sel.area && sel.area !== sel.project ? ` / ${sel.area}` : ""}</dd>
              {#if sel.iteration}<dt>Sprint</dt><dd>{sel.iteration}</dd>{/if}
              {#if detail?.data?.assignedTo}<dt>Assigned</dt><dd>{detail.data.assignedTo}</dd>{/if}
              {#if detail?.data?.createdBy}<dt>Created</dt><dd>{detail.data.createdBy}, {agoIso(detail.data.created)}</dd>{/if}
              <dt>Updated</dt><dd>{agoIso(sel.changed)}</dd>
              <dt>Source</dt><dd>{providerName(sel)}</dd>
              {#if sel.tags.length}<dt>Tags</dt><dd class="tags">{#each sel.tags as tag}<span class="chip">{tag}</span>{/each}</dd>{/if}
            </dl>

            {#if detail?.loading && !detail.data}
              <div class="skel-block" aria-hidden="true"><span class="skeleton"></span><span class="skeleton"></span><span class="skeleton"></span></div>
            {:else if detail?.error}
              <div class="state inline">
                <span class="state-title">Could not load this ticket</span>
                <pre>{detail.error}</pre>
                <div class="actions"><button class="btn sm" onclick={() => app.loadTicketDetail(sel, true)}>Try again</button></div>
              </div>
            {:else if detail?.data}
              {@const d = detail.data}
              {#if !d.sections.length}
                <div class="state inline"><span class="state-title">No description</span><p>This ticket has no description yet.</p></div>
              {/if}
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
                  <h3>Comments <span class="badge">{d.comments.length}</span></h3>
                  {#each d.comments as c, i}
                    <div class="comment">
                      <div class="chead"><b>{c.author}</b><span class="faint">{agoIso(c.created)}</span></div>
                      <div class="md">{@html html?.key === sel.key ? (html.comments[i] ?? "") : ""}</div>
                    </div>
                  {/each}
                </section>
              {/if}
            {:else}
              <div class="state inline">
                <span class="state-title">No details yet</span>
                <p>Tern has not read the description and comments of this ticket.</p>
                <div class="actions"><button class="btn sm" onclick={() => app.loadTicketDetail(sel, true)}>Load the details</button></div>
              </div>
            {/if}
          </div>
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
    grid-template-columns: var(--lw, 380px) 1px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    min-width: 0;
    background: var(--bg);
  }
  .list-side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--panel);
  }
  .list-side .toolbar.sub .field {
    min-width: 80px;
  }
  /* The line between the list and the detail. A wider invisible strip makes it easy to grab. */
  .split {
    position: relative;
    background: var(--line);
    cursor: col-resize;
  }
  .split::after {
    content: "";
    position: absolute;
    inset: 0 -3px;
    z-index: 1;
  }
  .split:hover {
    background: var(--line-strong);
  }
  .spin svg {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: var(--s1) var(--s2) var(--s3);
    outline: none;
  }
  .list .section-label {
    padding: var(--s3) var(--s3) 0;
    gap: var(--s2);
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
    background: var(--muted);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: flex-start;
    gap: var(--s3);
    padding: var(--s2) var(--s3);
    border-radius: var(--radius);
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
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .kind {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .k-bug {
    color: var(--del);
  }
  .k-task {
    color: var(--muted);
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
    gap: 2px;
  }
  .rtitle {
    font-size: var(--fs-md);
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
    gap: var(--s2);
    font-size: var(--fs-xs);
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .num {
    color: var(--muted);
    font-size: var(--fs-xs);
  }
  .prio {
    font-size: var(--fs-xs);
    font-weight: 600;
    padding: 0 var(--s1);
    border-radius: var(--r-sm);
    line-height: 16px;
    color: var(--muted);
    background: var(--raised);
  }
  .prio.p1,
  .prio-chip.p1 {
    color: var(--del);
    background: var(--del-bg);
    border-color: transparent;
  }
  .age {
    font-size: var(--fs-xs);
    flex-shrink: 0;
    margin-top: 2px;
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s1) var(--s4);
    padding: var(--s2) var(--s4);
    border-top: 1px solid var(--line);
    font-size: var(--fs-xs);
  }

  /* Loading: grey bars in the shape of the rows. */
  .skel-row {
    display: flex;
    flex-direction: column;
    gap: var(--s2);
    padding: var(--s3) var(--s3) var(--s3) calc(var(--s6) + var(--s2));
  }
  .skel-row .skeleton {
    height: 10px;
  }
  .skel-row .skeleton.short {
    width: 40%;
  }
  .skel-block {
    display: flex;
    flex-direction: column;
    gap: var(--s4);
    padding: var(--s3) 0;
  }
  .skel-block .skeleton {
    height: 10px;
  }
  .skel-block .skeleton:nth-child(2) {
    width: 85%;
  }
  .skel-block .skeleton:nth-child(3) {
    width: 60%;
  }

  .detail {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .dbar {
    gap: var(--s2);
  }
  /* Narrow details: the ticket number is the one chip that stays; the title below names the ticket.
     Then the less used copy actions go (they stay in the right-click menu and on the keys). */
  .dbar {
    container: dbar / inline-size;
  }
  @container dbar (max-width: 700px) {
    .dchips .lo {
      display: none;
    }
  }
  @container dbar (max-width: 520px) {
    .lo2 {
      display: none;
    }
  }
  .dchips {
    display: flex;
    align-items: center;
    gap: var(--s1);
    min-width: 0;
    overflow: hidden;
  }
  .dchips .chip {
    color: var(--text);
  }
  .dchips .chip :global(svg) {
    width: 12px;
    height: 12px;
  }
  .dchips .chip.k-bug :global(svg) {
    color: var(--del);
  }
  .ktext {
    color: var(--text);
  }
  .idchip {
    cursor: pointer;
  }
  .idchip.chip {
    color: var(--muted);
  }
  .idchip.chip:hover {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .dscroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    user-select: text;
  }
  .dwrap {
    max-width: 820px;
    padding: var(--s5) var(--s6) calc(var(--s6) * 2);
    animation: tern-fade-in 150ms var(--ease-out);
  }
  h1 {
    margin: 0 0 var(--s4);
    font-size: var(--fs-xl);
    line-height: 1.35;
    font-weight: 650;
    text-wrap: pretty;
  }
  .facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--s1) var(--s5);
    margin: 0 0 var(--s5);
    padding-bottom: var(--s5);
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
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
    gap: var(--s1);
  }
  .sec {
    margin-top: var(--s5);
  }
  h3 {
    display: flex;
    align-items: center;
    gap: var(--s2);
    margin: 0 0 var(--s3);
    font-size: var(--fs-sm);
    font-weight: 650;
    color: var(--muted);
  }
  .links {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    font-size: var(--fs-sm);
  }
  .links li {
    display: flex;
    gap: var(--s3);
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
    padding: var(--s3) 0;
    border-top: 1px solid var(--line);
  }
  .comment:first-of-type {
    border-top: 0;
  }
  .chead {
    display: flex;
    gap: var(--s3);
    font-size: var(--fs-sm);
    margin-bottom: var(--s1);
  }
  .md {
    font-size: var(--fs-md);
    line-height: 1.6;
    overflow-wrap: anywhere;
    max-width: 72ch;
  }
  .md :global(p),
  .md :global(div) {
    margin: 0 0 var(--s2);
  }
  /* Headings inside a ticket stay below the ticket's own title. */
  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4) {
    margin: var(--s4) 0 var(--s2);
    font-weight: 650;
    line-height: 1.35;
    color: var(--text);
  }
  .md :global(h1) {
    font-size: var(--fs-lg);
  }
  .md :global(h2) {
    font-size: var(--fs-md);
  }
  .md :global(h3),
  .md :global(h4) {
    font-size: var(--fs-md);
  }
  .md :global(:first-child) {
    margin-top: 0;
  }
  .md :global(ul),
  .md :global(ol) {
    margin: var(--s1) 0 var(--s3);
    padding-left: var(--s6);
  }
  .md :global(pre) {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: var(--s3) var(--s4);
    overflow-x: auto;
    font: var(--fs-sm) / 1.5 var(--mono);
  }
  .md :global(code) {
    font-family: var(--mono);
    font-size: var(--fs-sm);
    background: var(--panel);
    padding: 0 var(--s1);
    border-radius: var(--r-sm);
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
    margin: var(--s3) 0;
    border-radius: var(--radius);
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
  @keyframes fade {
    50% {
      opacity: 0.45;
    }
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
    gap: var(--s3);
    padding: calc(var(--s6) * 2);
    background: rgb(0 0 0 / 0.72);
    cursor: zoom-out;
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .zoom img {
    max-width: 100%;
    max-height: calc(100% - var(--h-bar));
    object-fit: contain;
    border-radius: var(--radius);
    animation: tern-dialog-in 200ms var(--ease-out);
  }
  .zcap {
    margin: 0;
    color: #e6e8ec;
    font-size: var(--fs-sm);
  }
  .zcap span {
    color: #9aa1ad;
    margin-left: var(--s3);
  }
  .md :global(.img-link) {
    display: inline-block;
    font-size: var(--fs-sm);
    padding: var(--s1) var(--s3);
    border: 1px dashed var(--line-strong);
    border-radius: var(--r-sm);
    text-decoration: none;
  }
  .md :global(table) {
    border-collapse: collapse;
  }
  .md :global(td),
  .md :global(th) {
    border: 1px solid var(--line);
    padding: var(--s1) var(--s3);
  }
  .md :global(blockquote) {
    margin: 0;
    padding-left: var(--s4);
    border-left: 3px solid var(--line-strong);
    color: var(--muted);
  }
  /* Cells wrap only at spaces and hyphens, so short columns like "ID" never break into letters. */
  .md :global(th),
  .md :global(td) {
    overflow-wrap: break-word;
    word-break: normal;
    min-width: 2.5em;
  }
</style>
