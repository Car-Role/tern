<script lang="ts">
  import { attachShell, fitShell } from "../lib/shells";

  let { id }: { id: number } = $props();
  let host: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (host) attachShell(id, host);
  });

  // Refit when the panel or window changes size.
  $effect(() => {
    if (!host) return;
    const ro = new ResizeObserver(() => fitShell(id));
    ro.observe(host);
    return () => ro.disconnect();
  });
</script>

<div class="host" bind:this={host}></div>

<style>
  .host {
    flex: 1;
    min-height: 0;
    padding: 6px 0 0 10px;
    background: var(--bg);
    overflow: hidden;
  }
  .host :global(.xterm-host),
  .host :global(.xterm) {
    height: 100%;
  }
  .host :global(.xterm-viewport) {
    background: var(--bg) !important;
  }
</style>
