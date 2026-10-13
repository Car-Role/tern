// Runs the Tauri CLI on any system.
// On Windows it goes through msvc.cmd, which loads the Visual Studio 2022 build tools first.
// On Windows, `build` makes only the NSIS installer unless you pass --bundles yourself.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const args = process.argv.slice(2);
const win = process.platform === "win32";
if (win && args[0] === "build" && !args.some((a) => a === "--bundles" || a === "-b")) args.push("--bundles", "nsis");

const here = path.dirname(fileURLToPath(import.meta.url));
const r = win
  ? spawnSync(`"${path.join(here, "msvc.cmd")}"`, ["tauri", ...args], { stdio: "inherit", shell: true })
  : spawnSync("npx", ["tauri", ...args], { stdio: "inherit" });
process.exit(r.status ?? 1);
