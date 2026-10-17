<script lang="ts" module>
  /** Line icons on a 16px grid, drawn with the current text color. */
  const paths: Record<string, string> = {
    home: "M2.5 7.2 8 2.8l5.5 4.4M4 6.2v6.8h3v-3.5h2V13h3V6.2",
    ticket: "M2.5 5a1 1 0 0 1 1-1h9a1 1 0 0 1 1 1v1.6a1.5 1.5 0 0 0 0 2.8V11a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1V9.4a1.5 1.5 0 0 0 0-2.8zM9.5 4.5v7",
    database: "M3 4.2C3 3 5.2 2.5 8 2.5s5 .5 5 1.7v7.6c0 1.2-2.2 1.7-5 1.7s-5-.5-5-1.7zM3 4.2c0 1.2 2.2 1.7 5 1.7s5-.5 5-1.7M3 8c0 1.2 2.2 1.7 5 1.7S13 9.2 13 8",
    search: "M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3 13.5 13.5",
    plus: "M8 3v10M3 8h10",
    gear: "M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM8 1.8v1.6M8 12.6v1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M1.8 8h1.6M12.6 8h1.6M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1",
    sidebar: "M2.5 3.5h11v9h-11zM6.5 3.5v9",
    back: "M10 3.5 5.5 8l4.5 4.5",
    forward: "M6 3.5 10.5 8 6 12.5",
    bell: "M4.5 11V7.5a3.5 3.5 0 0 1 7 0V11l1 1.5h-9zM6.8 13.5a1.3 1.3 0 0 0 2.4 0",
    refresh: "M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v2.8h-2.8",
    chevronDown: "M4 6l4 4 4-4",
    chevronRight: "M6 4l4 4-4 4",
    pin: "M6 2.5h4M7 2.5v4L4.5 9h7L9 6.5v-4M8 9v4.5",
    folder: "M2.5 4.5a1 1 0 0 1 1-1h3l1.5 1.5h4.5a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z",
    x: "M4 4l8 8M12 4l-8 8",
    more: "M3.5 8h.01M8 8h.01M12.5 8h.01",
    branch: "M5 3v10M5 9.5c0-2 6-1.5 6-4.5M11 5a1.5 1.5 0 1 0 0-.01M5 13a1.5 1.5 0 1 0 0-.01M5 3a1.5 1.5 0 1 0 0-.01",
    play: "M5 3.5v9l7-4.5z",
    download: "M8 2.5v8M4.5 7 8 10.5 11.5 7M3 13h10",
    upload: "M8 13V5M4.5 8.5 8 5l3.5 3.5M3 3h10",
    check: "M3 8.5 6.5 12 13 4.5",
    alert: "M8 2.5 14 13H2zM8 6.5v3M8 11.3v.01",
    circle: "M8 13a5 5 0 1 0 0-10 5 5 0 0 0 0 10z",
    clone: "M5.5 5.5h7v7h-7zM10.5 5.5v-2h-7v7h2",
    terminal: "M2.5 3.5h11v9h-11zM5 7l2 1.5L5 10M8.5 10.5h2.5",
    filter: "M2.5 3.5h11L9.5 8.5v4l-3 1v-5z",
  };
</script>

<script lang="ts">
  let { name, size = 14, stroke = 1.5 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg viewBox="0 0 16 16" width={size} height={size} aria-hidden="true" class="icon-svg">
  <path d={paths[name] ?? ""} fill="none" stroke="currentColor" stroke-width={name === "more" ? 2.6 : stroke} stroke-linecap="round" stroke-linejoin="round" />
</svg>

<style>
  .icon-svg {
    flex-shrink: 0;
    display: block;
  }
</style>
