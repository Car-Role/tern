<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api, inTauri } from "../lib/api";
  import { defaultPort, engineLabel } from "../lib/dbtext";
  import type { DbConn, DbEngine, DbTls } from "../lib/types";

  let { conn, onclose }: { conn: DbConn | null; onclose: () => void } = $props();

  const COLORS = ["", "#5fbf8b", "#6fa8ff", "#f0b54a", "#e5786d", "#b48cf2"];
  const blank: DbConn = { id: "", name: "", engine: "postgres", file: "", host: "localhost", port: 5432, database: "", user: "", tls: "prefer", hasPassword: false, allowWrites: false, color: "" };
  // The form edits a copy, taken once when it opens.
  let f = $state<DbConn>(untrack(() => (conn ? { ...conn } : { ...blank })));
  let password = $state("");
  let clearPassword = $state(false);
  let testing = $state(false);
  let saving = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);
  let first: HTMLInputElement | undefined = $state();

  onMount(() => first?.focus());

  function pickEngine(e: DbEngine) {
    if (f.port === 0 || f.port === defaultPort[f.engine]) f.port = defaultPort[e];
    if (e === "redis" && f.tls === "prefer") f.tls = "off";
    f.engine = e;
    result = null;
  }

  async function browse() {
    if (!inTauri) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: false, filters: [{ name: "SQLite", extensions: ["db", "sqlite", "sqlite3", "db3", "s3db", "sl3"] }, { name: "All files", extensions: ["*"] }] });
    if (typeof picked === "string") {
      f.file = picked;
      if (!f.name) f.name = picked.split(/[\\/]/).pop() ?? "";
    }
  }

  /** The password to send: what was typed, "" to remove the saved one, or null to keep it. */
  const pw = () => (password ? password : clearPassword ? "" : null);

  async function test() {
    testing = true;
    result = null;
    try {
      result = { ok: true, text: await api.dbTest($state.snapshot(f) as DbConn, pw() ?? (f.hasPassword ? null : "")) };
    } catch (e) {
      result = { ok: false, text: String(e).replace(/^Error: /, "") };
    } finally {
      testing = false;
    }
  }

  async function save() {
    if (!f.name.trim()) f.name = f.engine === "sqlite" ? (f.file.split(/[\\/]/).pop() ?? "SQLite") : `${f.host || "localhost"}${f.database ? "/" + f.database : ""}`;
    saving = true;
    try {
      await app.saveDbConn($state.snapshot(f) as DbConn, pw());
      onclose();
    } catch (e) {
      result = { ok: false, text: String(e).replace(/^Error: /, "") };
    } finally {
      saving = false;
    }
  }

  async function remove() {
    if (!conn) return;
    const ok = await app.ask({ title: `Delete ${conn.name}?`, body: "This removes the saved connection and its password from the keychain. The database itself is not touched.", confirm: "Delete" });
    if (!ok) return;
    await app.removeDbConn(conn.id);
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === "Enter" && (e.ctrlKey || (e.target as HTMLElement).tagName === "INPUT")) {
      e.preventDefault();
      save();
    }
  }

  const tlsLabels: Record<DbTls, string> = { off: "Off", prefer: "If offered", require: "Required", requireNoVerify: "Required, any certificate" };
</script>

<div class="backdrop fx-fade" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dbf-title" tabindex="-1" onkeydown={onKey}>
    <h2 id="dbf-title">{conn ? `Edit ${conn.name}` : "New connection"}</h2>

    <div class="seg engines" role="group" aria-label="Database type">
      {#each ["postgres", "mssql", "sqlite", "redis"] as e}
        <button class:on={f.engine === e} onclick={() => pickEngine(e as DbEngine)}>{engineLabel[e as DbEngine]}</button>
      {/each}
    </div>

    <div class="form">
      <label for="dbf-name">Name</label>
      <div class="row">
        <input id="dbf-name" bind:this={first} bind:value={f.name} placeholder={f.engine === "sqlite" ? "app.db" : "Production read replica"} spellcheck="false" />
        <div class="colors" role="radiogroup" aria-label="Color">
          {#each COLORS as c}
            <button class="dot" class:on={f.color === c} style:--c={c || "transparent"} class:none={!c} onclick={() => (f.color = c)} aria-label={c ? `Color ${c}` : "No color"} role="radio" aria-checked={f.color === c}></button>
          {/each}
        </div>
      </div>

      {#if f.engine === "sqlite"}
        <label for="dbf-file">File</label>
        <div class="row">
          <input id="dbf-file" class="mono" bind:value={f.file} placeholder="C:\path\to\app.db" spellcheck="false" />
          {#if inTauri}<button class="btn" onclick={browse}>Browse…</button>{/if}
        </div>
      {:else}
        <label for="dbf-host">Host</label>
        <div class="row">
          <input id="dbf-host" class="mono" bind:value={f.host} placeholder={f.engine === "mssql" ? "localhost or server\\SQLEXPRESS" : "localhost"} spellcheck="false" />
          <input class="mono port" type="number" min="0" max="65535" bind:value={f.port} aria-label="Port" />
        </div>
        <label for="dbf-db">{f.engine === "redis" ? "Database number" : "Database"}</label>
        <input id="dbf-db" class="mono" bind:value={f.database} placeholder={f.engine === "redis" ? "0" : f.engine === "mssql" ? "master" : "postgres"} spellcheck="false" />
        <label for="dbf-user">User</label>
        <input id="dbf-user" class="mono" bind:value={f.user} spellcheck="false" autocomplete="off"
          placeholder={f.engine === "mssql" ? "Empty: sign in with your Windows account" : f.engine === "redis" ? "Empty for the default user" : "postgres"} />
        <label for="dbf-pw">Password</label>
        <div class="row">
          <input id="dbf-pw" type="password" bind:value={password} autocomplete="new-password"
            placeholder={f.hasPassword && !clearPassword ? "Saved in the system keychain. Type to replace it." : ""} />
          {#if f.hasPassword && !password}
            <label class="check"><input type="checkbox" bind:checked={clearPassword} /> Remove</label>
          {/if}
        </div>
        <label for="dbf-tls">Encryption</label>
        <select id="dbf-tls" bind:value={f.tls}>
          {#each Object.entries(tlsLabels) as [v, l]}
            {#if !(f.engine === "redis" && v === "prefer")}<option value={v}>{l}</option>{/if}
          {/each}
        </select>
      {/if}

      <span></span>
      <label class="check writes">
        <input type="checkbox" bind:checked={f.allowWrites} />
        <span>Allow writes <span class="faint">— off keeps this connection read-only. Even when on, Tern asks before a query that changes data.</span></span>
      </label>
    </div>

    {#if result}
      <p class="result mono" class:ok={result.ok}>{result.ok ? "Connected: " : ""}{result.text}</p>
    {/if}

    <div class="actions">
      {#if conn}<button class="btn danger" onclick={remove}>Delete</button>{/if}
      <span class="grow"></span>
      <button class="btn" onclick={test} disabled={testing}>{testing ? "Testing…" : "Test"}</button>
      <button class="btn" onclick={onclose}>Cancel</button>
      <button class="btn primary" onclick={save} disabled={saving || (f.engine === "sqlite" && !f.file.trim())}>{saving ? "Saving…" : "Save"}</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.5);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: min(10vh, 64px) 16px 16px;
    z-index: 20;
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .dialog {
    width: min(560px, 100%);
    max-height: calc(100vh - 80px);
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
    padding: 18px 20px 16px;
    outline: none;
    animation: tern-dialog-in 150ms var(--ease-out);
  }
  h2 {
    margin: 0 0 12px;
    font-size: 15px;
    font-weight: 650;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    width: fit-content;
    margin-bottom: 14px;
  }
  .seg button {
    font-size: 12.5px;
    padding: 4px 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .form {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 8px 12px;
    align-items: center;
  }
  .form > label {
    color: var(--muted);
    font-size: 12.5px;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  input:not([type="checkbox"]),
  select {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 12.5px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 5px 8px;
    outline: none;
  }
  input.mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  input:focus,
  select:focus {
    border-color: var(--amber);
  }
  .port {
    flex: 0 0 84px;
  }
  .colors {
    display: flex;
    gap: 5px;
  }
  .dot {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--c);
    border: 1px solid var(--line-strong);
    padding: 0;
  }
  .dot.none {
    background: linear-gradient(135deg, transparent 45%, var(--faint) 45% 55%, transparent 55%);
  }
  .dot.on {
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .check input {
    accent-color: var(--amber);
  }
  .writes {
    white-space: normal;
    align-items: flex-start;
    color: var(--text);
  }
  .writes input {
    margin-top: 3px;
  }
  .result {
    margin: 14px 0 0;
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 12px;
    white-space: pre-wrap;
    color: var(--del);
    background: var(--del-bg);
    user-select: text;
  }
  .result.ok {
    color: var(--add);
    background: var(--add-bg);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 16px;
  }
  .grow {
    flex: 1;
  }
  .btn {
    font-size: 12.5px;
    padding: 4px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--text);
  }
  .btn:hover:not(:disabled) {
    background: var(--hover);
  }
  .btn.primary {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--on-accent);
  }
  .btn.danger {
    color: var(--del);
  }
  .btn:disabled {
    opacity: 0.5;
  }
</style>
