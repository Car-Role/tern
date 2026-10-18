// Runs once, before anything else reads saved settings: view settings saved under the old "scout." names
// (from before Tern had its name) move to "tern." names. Imported first in main.ts.

export function migrateStorage(store: Storage, from = "scout.", to = "tern.") {
  const old: string[] = [];
  for (let i = 0; i < store.length; i++) {
    const k = store.key(i);
    if (k?.startsWith(from)) old.push(k);
  }
  for (const k of old) {
    const next = to + k.slice(from.length);
    const v = store.getItem(k);
    // A value already saved under the new name wins.
    if (v !== null && store.getItem(next) === null) store.setItem(next, v);
    store.removeItem(k);
  }
  return old.length;
}

try {
  migrateStorage(localStorage);
} catch {
  // No storage (private window, blocked site data): nothing to move.
}
