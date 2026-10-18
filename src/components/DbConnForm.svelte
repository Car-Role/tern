<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api, inTauri } from "../lib/api";
  import { defaultPort, engineLabel } from "../lib/dbtext";
  import type { DbConn, DbEngine, DbTls } from "../lib/types";

  let { conn, onclose }: { conn: DbConn | null; onclose: () => void } = $props();

  const COLORS = ["", "#5fbf8b", "#6fa8ff", "#f0b54a", "#e5786d", "#b48cf2"];
  const blank: DbConn = { id: "", name: "", engine: "postgres", file: "", host: "localhost", port: 5432, database: "", user: "", tls: "prefer", hasPassword: false, allowWrites: false, color: "", entra: false };
  // The form edits a copy, taken once when it opens.
  let f = $state<DbConn>(untrack(() => (conn ? { ...conn } : { ...blank })));
  let password = $state("");
  let clearPassword = $state(false);
  let testing = $state(false);
  let saving = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);
  let first: HTMLInputElement | undefined = $state();
  // SQL Server only. "windows" is an empty user name; "entra" uses a token from az login.
  let signin = $state<"sql" | "windows" | "entra">(untrack(() => (f.entra ? "entra" : conn && !conn.user ? "windows" : "sql")));
  const showLogin = $derived(f.engine !== "mssql" || signin === "sql");

  function pickSignin(s: typeof signin) {
    signin = s;
    f.entra = s === "entra";
    if (s !== "sql") {
      f.user = "";
      password = "";
      if (f.hasPassword) clearPassword = true;
    } else {
      clearPassword = false;
    }
    result = null;
  }

  onMount(() => first?.focus());

  function pickEngine(e: DbEngine) {
    if (f.port === 0 || f.port === defaultPort[f.engine]) f.port = defaultPort[e];
    if (e === "redis" && f.tls === "prefer") f.tls = "off";
    if (e !== "mssql") f.entra = false;
    else if (signin === "entra") f.entra = true;
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
    const ok = await app.ask({ title: `Delete ${conn.name}?`, body: "This removes the saved connection and its password from the keychain. The database itself is not touched.", confirm: "Delete", danger: true });
    if (!ok) return;
    await app.removeDbConn(conn.id);
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      // Only Ctrl Enter saves. A plain Enter in a field is too easy to press by accident.
      e.preventDefault();
      save();
    }
  }

  const tlsLabels: Record<DbTls, string> = { off: "Off", prefer: "If offered", require: "Required", requireNoVerify: "Required, any certificate" };
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dbf-title" tabindex="-1" onkeydown={onKey}>
    <div class="dialog-head">
      <h2 id="dbf-title">{conn ? `Edit ${conn.name}` : "New connection"}</h2>
    </div>

    <div class="dialog-body">
      <div class="seg engines" role="group" aria-label="Database type">
        {#each ["postgres", "mssql", "sqlite", "redis"] as e}
          <button class:on={f.engine === e} aria-pressed={f.engine === e} onclick={() => pickEngine(e as DbEngine)}>{engineLabel[e as DbEngine]}</button>
        {/each}
      </div>

      <div class="form">
        <label for="dbf-name">Name</label>
        <div class="row">
          <input id="dbf-name" class="field" bind:this={first} bind:value={f.name} placeholder={f.engine === "sqlite" ? "app.db" : "Production read replica"} spellcheck="false" />
          <div class="colors" role="radiogroup" aria-label="Color">
            {#each COLORS as c}
              <button class="swatch" class:on={f.color === c} style:--c={c || "transparent"} class:none={!c} onclick={() => (f.color = c)} aria-label={c ? `Color ${c}` : "No color"} role="radio" aria-checked={f.color === c}></button>
            {/each}
          </div>
        </div>

        {#if f.engine === "sqlite"}
          <label for="dbf-file">File</label>
          <div class="row">
            <input id="dbf-file" class="field mono" bind:value={f.file} placeholder="C:\path\to\app.db" spellcheck="false" />
            {#if inTauri}<button class="btn" onclick={browse}>Browse…</button>{/if}
          </div>
        {:else}
          <label for="dbf-host">Host</label>
          <div class="row">
            <input id="dbf-host" class="field mono" bind:value={f.host} placeholder={f.engine === "mssql" ? "localhost or server\\SQLEXPRESS" : "localhost"} spellcheck="false" />
            <input class="field mono port" type="number" min="0" max="65535" bind:value={f.port} aria-label="Port" />
          </div>
          <label for="dbf-db">{f.engine === "redis" ? "Database number" : "Database"}</label>
          <input id="dbf-db" class="field mono" bind:value={f.database} placeholder={f.engine === "redis" ? "0" : f.engine === "mssql" ? "master" : "postgres"} spellcheck="false" />
          {#if f.engine === "mssql"}
            <span class="lbl" id="dbf-signin">Sign in with</span>
            <div class="seg signin" role="group" aria-labelledby="dbf-signin">
              <button class:on={signin === "sql"} aria-pressed={signin === "sql"} onclick={() => pickSignin("sql")}>SQL login</button>
              <button class:on={signin === "windows"} aria-pressed={signin === "windows"} onclick={() => pickSignin("windows")}>Windows</button>
              <button class:on={signin === "entra"} aria-pressed={signin === "entra"} onclick={() => pickSignin("entra")}>Microsoft Entra</button>
            </div>
            {#if signin === "windows"}
              <span></span><p class="hint faint">Uses the Windows account you are signed in with. The server must trust your computer's domain.</p>
            {:else if signin === "entra"}
              <span></span><p class="hint faint">Uses your Azure CLI sign-in. Run <code>az login</code> once in a terminal. Tern asks it for a token and stores no password. The server must have a login for your account or one of your groups.</p>
            {/if}
          {/if}
          {#if showLogin}
            <label for="dbf-user">User</label>
            <input id="dbf-user" class="field mono" bind:value={f.user} spellcheck="false" autocomplete="off"
              placeholder={f.engine === "mssql" ? "sa" : f.engine === "redis" ? "Empty for the default user" : "postgres"} />
            <label for="dbf-pw">Password</label>
            <div class="row">
              <input id="dbf-pw" class="field" type="password" bind:value={password} autocomplete="new-password"
                placeholder={f.hasPassword && !clearPassword ? "Saved in the system keychain. Type to replace it." : ""} />
              {#if f.hasPassword && !password}
                <label class="check"><input type="checkbox" bind:checked={clearPassword} /> Remove</label>
              {/if}
            </div>
          {/if}
          <label for="dbf-tls">Encryption</label>
          <select id="dbf-tls" class="field" bind:value={f.tls}>
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
    </div>

    <div class="dialog-foot">
      {#if conn}<button class="btn danger" onclick={remove}>Delete…</button>{/if}
      <span class="hint"><kbd>Ctrl Enter</kbd> saves</span>
      <button class="btn" onclick={test} disabled={testing}>{testing ? "Testing…" : "Test"}</button>
      <button class="btn" onclick={onclose}>Cancel</button>
      <button class="btn primary" onclick={save} disabled={saving || (f.engine === "sqlite" && !f.file.trim())}>{saving ? "Saving…" : "Save"}</button>
    </div>
  </div>
</div>

<style>
  .engines {
    margin-bottom: var(--s5);
  }
  .form {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: var(--s3) var(--s4);
    align-items: center;
  }
  .form > label:not(.check),
  .form > .lbl {
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .signin {
    justify-self: start;
  }
  .hint {
    margin: calc(var(--s2) * -1) 0 0;
    font-size: var(--fs-sm);
    line-height: 1.45;
    text-wrap: pretty;
  }
  .row {
    display: flex;
    gap: var(--s3);
    align-items: center;
    min-width: 0;
  }
  .row > .field {
    flex: 1;
  }
  .row > .field.port {
    flex: 0 0 84px;
  }
  .colors {
    display: flex;
    gap: var(--s1);
  }
  .swatch {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--c);
    border: 1px solid var(--line-strong);
    padding: 0;
  }
  .swatch.none {
    background: linear-gradient(135deg, transparent 45%, var(--faint) 45% 55%, transparent 55%);
  }
  .swatch.on {
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }
  .check {
    white-space: nowrap;
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
    margin: var(--s4) 0 0;
    padding: var(--s3) var(--s4);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
    white-space: pre-wrap;
    color: var(--del);
    background: var(--del-bg);
    user-select: text;
  }
  .result.ok {
    color: var(--add);
    background: var(--add-bg);
  }
</style>
