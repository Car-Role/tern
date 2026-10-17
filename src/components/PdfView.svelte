<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { bytes as fmtBytes } from "../lib/format";

  let { repo, rel, rev = null }: { repo: string; rel: string; rev?: string | null } = $props();

  let url = $state<string | null>(null);
  let info = $state<{ size: number; tooLarge: boolean; missing: boolean; notPdf: boolean } | null>(null);
  let error = $state("");

  // A blob: URL, because the window does not open PDFs from data: URLs. It is freed when the view closes.
  $effect(() => {
    const [r, p, v] = [repo, rel, rev];
    let made: string | null = null;
    url = null;
    info = null;
    error = "";
    api.fileBytes(r, p, v).then(
      async (f) => {
        if (r !== repo || p !== rel || v !== rev) return;
        info = { size: f.size, tooLarge: f.tooLarge, missing: !f.dataUrl && !f.tooLarge, notPdf: !!f.dataUrl && f.mime !== "application/pdf" };
        if (!f.dataUrl || f.mime !== "application/pdf") return;
        const blob = await (await fetch(f.dataUrl)).blob();
        made = URL.createObjectURL(new Blob([blob], { type: "application/pdf" }));
        url = made;
      },
      (e) => (error = String(e)),
    );
    return () => {
      if (made) URL.revokeObjectURL(made);
    };
  });
</script>

<div class="pdf">
  {#if error}
    <p class="note err mono">{error}</p>
  {:else if !info}
    <p class="note">Opening the PDF…</p>
  {:else if info.tooLarge}
    <p class="note">This PDF is {fmtBytes(info.size)}. PDFs over 40 MB do not open here.</p>
  {:else if info.missing}
    <p class="note">This PDF does not exist in this version.</p>
  {:else if info.notPdf}
    <p class="note">This file is named .pdf, but it is not a PDF. It may be damaged.</p>
  {:else if url}
    <iframe src={url} title={rel}></iframe>
    <div class="info mono">
      <span>{fmtBytes(info.size)}</span>
      <span class="faint">If the page stays blank, this system cannot show PDFs inside Tern.</span>
      <span class="grow"></span>
      <button class="mode" onclick={() => app.open("explorer")}>Show in folder</button>
    </div>
  {/if}
</div>

<style>
  .pdf {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  iframe {
    flex: 1;
    border: 0;
    width: 100%;
    background: var(--panel);
  }
  .info {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 14px;
    border-top: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--muted);
  }
  .grow {
    flex: 1;
  }
  .mode {
    font-size: 11.5px;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 1px 8px;
  }
  .mode:hover {
    color: var(--text);
  }
  .note {
    margin: 24px;
    color: var(--muted);
  }
  .note.err {
    color: var(--del);
    font-size: 12px;
  }
</style>
