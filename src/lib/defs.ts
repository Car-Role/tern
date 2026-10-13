// "Go to definition" without a language server: a text search for the common ways code defines a name.
// It covers TypeScript/JavaScript, C#, Java, Rust, Go, Python, and similar languages. It is a good guess,
// not a compiler, so it can miss a definition or find two.

const CODE = /\.(ts|tsx|js|jsx|mjs|cjs|svelte|vue|cs|java|kt|rs|go|py|rb|php|swift|c|h|cpp|hpp|cc|scala|dart|lua|ex|exs|fs|vb|sql|ps1|sh)$/i;

export function isCodeFile(path: string) {
  return CODE.test(path);
}

/** A name that can be searched for: letters, digits, `_`, and `$`. */
export function cleanWord(word: string) {
  return word.match(/[A-Za-z_$][\w$]*/)?.[0] ?? "";
}

/** One Perl-style pattern (for `git grep -P`) that matches lines that define `word`. */
export function definitionPattern(word: string) {
  const w = cleanWord(word).replace(/\$/g, "\\$");
  return [
    // function x, class x, interface x, fn x, def x, struct x, enum x, type x, ...
    `\\b(?:function\\*?|class|interface|type|enum|struct|trait|impl|def|fn|func|module|namespace|record|object|protocol|typedef|macro|union)\\s+${w}\\b`,
    // const x =, let x: T =, var x =, val x =
    `\\b(?:const|let|var|val)\\s+${w}\\b\\s*[:=]`,
    // x = function, x = (a) =>, x: async () =>
    `\\b${w}\\s*[:=]\\s*(?:async\\s*)?(?:function\\b|\\([^)]*\\)\\s*=>|[A-Za-z_$][\\w$]*\\s*=>)`,
    // C#, Java, TypeScript members: public static Task<T> X(  /  private async x(
    `\\b(?:public|private|protected|internal|static|async|override|virtual|abstract|sealed|export|readonly)\\b[^=;(]*?\\b${w}\\s*(?:<[^>]*>)?\\s*\\(`,
    // A class method written as `x(args) {`
    `^\\s*(?:async\\s+)?${w}\\s*\\([^)]*\\)\\s*(?::[^{]*)?\\{`,
    // C# properties: public int X { get
    `\\b${w}\\s*\\{\\s*(?:get|set|init)\\b`,
  ].join("|");
}
