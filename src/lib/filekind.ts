// Which view a file gets, from its name.

export type RichKind = "markdown" | "image" | "svg" | "pdf" | "json" | "yaml" | "toml" | "csv" | "tsv" | "log";

const BY_EXT: Record<string, RichKind> = {
  md: "markdown", markdown: "markdown", mdx: "markdown",
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image", bmp: "image", ico: "image", avif: "image",
  svg: "svg",
  pdf: "pdf",
  json: "json", jsonc: "json", json5: "json", geojson: "json", webmanifest: "json",
  yaml: "yaml", yml: "yaml",
  toml: "toml",
  csv: "csv",
  tsv: "tsv", tab: "tsv",
  log: "log",
};

/** Files whose whole name says what they are. */
const BY_NAME: Record<string, RichKind> = {
  "package.json": "json", "tsconfig.json": "json", "composer.lock": "json", ".babelrc": "json", ".eslintrc": "json",
  "cargo.lock": "toml", "pipfile": "toml",
};

export function richKind(rel: string): RichKind | null {
  const name = rel.split("/").pop()!.toLowerCase();
  if (BY_NAME[name]) return BY_NAME[name];
  // Rotated logs: app.log.1, app.log.2026-10-17
  if (/\.log(\.[\w-]+)?$/.test(name)) return "log";
  const ext = name.includes(".") ? name.split(".").pop()! : "";
  return BY_EXT[ext] ?? null;
}

/** Kinds that are pictures or documents, not text: the code view never loads them. */
export const isBinaryKind = (k: RichKind | null) => k === "image" || k === "pdf";

/** Kinds with a Page view next to the Source (code) view. */
export const hasPage = (k: RichKind | null) => k !== null && k !== "image" && k !== "pdf";

export const kindLabel: Record<RichKind, string> = {
  markdown: "Markdown", image: "Image", svg: "SVG", pdf: "PDF", json: "JSON", yaml: "YAML", toml: "TOML", csv: "CSV", tsv: "TSV", log: "Log",
};
