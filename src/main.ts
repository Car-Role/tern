// First: move old saved settings to their new names, before the store reads them.
import "./lib/migrate";
import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { app } from "./lib/store.svelte";

// Set the theme before the first paint, so the window does not flash the default colors.
app.applyTheme();

// Development builds only: lets the browser console reach the app state for debugging.
if (import.meta.env.DEV) {
  (window as unknown as { __tern: typeof app }).__tern = app;
  import("./lib/api").then(({ api }) => ((window as unknown as { __ternApi: typeof api }).__ternApi = api));
}

export default mount(App, { target: document.getElementById("app")! });
