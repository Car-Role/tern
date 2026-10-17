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
    <div class="state">
      <span class="state-title">Could not open this PDF</span>
      <pre>{error}</pre>
    </div>
  {:else if !info}
    <div class="state delay"><p>Opening the PDF…</p></div>
  {:else if info.tooLarge}
    <div class="state">
      <span class="state-title">Too large to open here</span>
      <p>This PDF is {fmtBytes(info.size)}. PDFs over 40 MB do not open in Tern.</p>
      <div class="actions"><button class="btn" onclick={() => app.open("explorer")}>Show in folder</button></div>
    </div>
  {:else if info.missing}
    <div class="state"><p>This PDF does not exist in this version.</p></div>
  {:else if info.notPdf}
    <div class="state">
      <span class="state-title">Not a PDF</span>
      <p>This file is named .pdf, but it is not a PDF. It may be damaged.</p>
    </div>
  {:else if url}
    <div class="toolbar sub">
      <span class="meta">{fmtBytes(info.size)}</span>
      <span class="meta">If the page stays blank, this system cannot show PDFs inside Tern.</span>
      <span class="grow"></span>
      <button class="btn sm ghost" onclick={() => app.open("explorer")}>Show in folder</button>
    </div>
    <iframe src={url} title={rel}></iframe>
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
  /* Opening that ends within 300 ms shows nothing. */
  .state.delay {
    animation: tern-fade-in 150ms var(--ease-out) 300ms both;
  }
</style>
