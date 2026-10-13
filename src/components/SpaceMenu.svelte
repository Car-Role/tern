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

  type Opt = { id: string | null; name: string; color: string; hint: string; key: string };
  const options: Opt[] = [
    ...(moving
      ? [{ id: null, name: "Use the rules", color: "var(--faint)", hint: "Remote and folder rules pick the space", key: "0" }]
      : [{ id: null, name: "All spaces", color: "var(--faint)", hint: `${app.repos.length} projects`, key: "0" }]),
    ...app.spaces.map((sp, i) => ({
      id: sp.id,
      name: sp.name,
      color: sp.color,
      hint: moving ? "" : `${app.repos.filter((r) => app.spaceOf(r) === sp.id).length} projects`,
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

  onMount(() => box?.focus());
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="menu" role="dialog" aria-label={moving ? "Move project to a space" : "Switch space"} tabindex="0" bind:this={box} onkeydown={onKey}>
    <p class="what">
      {#if moving && targets.length > 1}Move <strong>{targets.length} projects</strong> to{:else if moving && repo}Move <strong>{baseName(repo.path)}</strong> to{:else}Switch space{/if}
    </p>
    {#each options as o, i}
      <button class="opt" class:on={i === active} class:current={o.id === current} onmousemove={() => (active = i)} onclick={() => choose(o)}>
        <kbd>{o.key}</kbd>
        <span class="swatch" style:background={o.color}></span>
        <span class="name">{o.name}</span>
        <span class="hint faint">{o.hint}</span>
      </button>
    {/each}
    <p class="foot faint">
      {#if moving}A choice made here wins over the rules.{:else}<kbd>Ctrl Alt 1</kbd>–<kbd>9</kbd> switch at any time. Set up spaces in Settings.{/if}
    </p>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 16vh;
    z-index: 10;
  }
  .menu {
    width: min(420px, calc(100vw - 32px));
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    padding: 8px;
    outline: none;
  }
  .what {
    margin: 4px 8px 8px;
    color: var(--muted);
  }
  .what strong {
    color: var(--text);
    font-weight: 600;
  }
  .opt {
    display: grid;
    grid-template-columns: 26px 10px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: left;
    padding: 7px 8px;
    border-radius: var(--radius);
  }
  .opt.on {
    background: var(--raised);
  }
  .opt.current .name::after {
    content: "current";
    margin-left: 8px;
    font-size: 11px;
    color: var(--amber);
  }
  .opt kbd {
    justify-self: center;
    color: var(--text);
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 3px;
  }
  .hint {
    font-size: 12px;
  }
  .foot {
    margin: 8px 8px 2px;
    font-size: 11px;
  }
</style>
