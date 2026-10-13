// Runs the Rust tests on any system. On Windows it goes through msvc.cmd first.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const cwd = path.join(here, "..", "src-tauri");
const r = process.platform === "win32"
  ? spawnSync(`"${path.join(here, "msvc.cmd")}"`, ["cargo", "test"], { stdio: "inherit", shell: true, cwd })
  : spawnSync("cargo", ["test"], { stdio: "inherit", cwd });
process.exit(r.status ?? 1);
