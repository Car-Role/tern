<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { baseName } from "../lib/format";

  // "spaces" switches the space on screen. "move" puts the open project in a space.
  const moving = app.overlay === "move";
  const targets = app.moveTargets.length ? app.moveTargets : app.current ? [app.current] : [];
  const repo = targets.length === 1 ? app.repos.find((r) => r.path === targets[0]) : undefined;
  app.moveTargets = [];
  const current = moving && repo ? (app.config?.assign[repo.path] ?? null) : moving ? undefined : (app.config?.activeSpace ?? null);

  const projects = (n: number) => `${n} ${n === 1 ? "project" : "projects"}`;

  type Opt = { id: string | null; name: string; color: string; hint: string; key: string };
  const options: Opt[] = [
    ...(moving
      ? [{ id: null, name: "Use the rules", color: "var(--faint)", hint: "Remote and folder rules pick the space", key: "0" }]
      : [{ id: null, name: "All spaces", color: "var(--faint)", hint: projects(app.repos.length), key: "0" }]),
    ...app.spaces.map((sp, i) => ({
      id: sp.id,
      name: sp.name,
      color: sp.color,
      hint: moving ? "" : projects(app.repos.filter((r) => app.spaceOf(r) === sp.id).length),
      key: String(i + 1),
    })),
  ];

  let active = $state(Math.max(0, options.findIndex((o) => o.id === current)));
  let box: HTMLDivElement | undefined = $state();

  function choose(o: Opt | undefined) {
    if (!o) return;
    app.overlay = null;
    if (moving && targets.length > 1) app.bulkMove(targets, o.id);
    else if (moving && repo) app.moveToSpace(repo.path, o.id);
    else if (!moving) app.switchSpace(o.id);
  }

  function onKey(e: KeyboardEvent) {
    const byKey = options.find((o) => o.key === e.key);
    if (byKey) choose(byKey);
    else if (e.key === "ArrowDown") active = (active + 1) % options.length;
    else if (e.key === "ArrowUp") active = (active - 1 + options.length) % options.length;
    else if (e.key === "Enter") choose(options[active]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function openSpaceSettings() {
    try {
      localStorage.setItem("scout.settingsSection", "spaces");
    } catch {
      /* Settings opens on its first section instead. */
    }
    app.overlay = "settings";
  }

  onMount(() => box?.focus());
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="dialog sm" role="dialog" aria-label={moving ? "Move project to a space" : "Switch space"} tabindex="0" bind:this={box} onkeydown={onKey}>
    <div class="dialog-head">
      <h2 class="what">
        {#if moving && targets.length > 1}Move <strong>{targets.length} projects</strong> to{:else if moving && repo}Move <strong>{baseName(repo.path)}</strong> to{:else}Switch space{/if}
      </h2>
    </div>
    <div class="dialog-body list" role="listbox" aria-label="Spaces">
      {#each options as o, i}
        <button class="opt" role="option" aria-selected={i === active} class:on={i === active} onmousemove={() => (active = i)} onclick={() => choose(o)}>
          <kbd>{o.key}</kbd>
          <span class="swatch" style:background={o.color}></span>
          <span class="name">{o.name}{#if o.id === current}<span class="chip current">current</span>{/if}</span>
          <span class="hint">{o.hint}</span>
        </button>
      {/each}
    </div>
    <div class="dialog-foot">
      <span class="hint">
        {#if moving}A choice made here wins over the rules.{:else}<kbd>Ctrl Alt 1</kbd>–<kbd>9</kbd> switch at any time. <button class="link" onclick={openSpaceSettings}>Set up spaces in Settings</button>{/if}
      </span>
    </div>
  </div>
</div>

<style>
  .what {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .list {
    padding-left: var(--s3);
    padding-right: var(--s3);
    padding-bottom: 0;
  }
  .opt {
    display: grid;
    grid-template-columns: var(--h-ctl) var(--s4) minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    height: var(--h-row);
    text-align: left;
    padding: 0 var(--s3);
    border-radius: var(--radius);
  }
  .opt.on {
    background: var(--raised);

  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .current {
    margin-left: var(--s3);
  }
  .opt kbd {
    justify-self: center;
    color: var(--text);
  }
  .swatch {
    width: var(--s4);
    height: var(--s4);
    border-radius: var(--r-sm);
  }
  .opt .hint {
    font-size: var(--fs-sm);
    color: var(--faint);
  }
  .dialog-foot .link {
    font-size: inherit;
  }
</style>
