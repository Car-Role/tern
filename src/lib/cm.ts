// CodeMirror setup shared by the file viewer and the diff viewer.

import { EditorState, Compartment, StateEffect, StateField, type Extension } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  drawSelection,
  keymap,
  highlightSpecialChars,
} from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting, LanguageDescription, bracketMatching, foldGutter } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { searchKeymap, highlightSelectionMatches, search } from "@codemirror/search";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap } from "@codemirror/autocomplete";
import { tags as t } from "@lezer/highlight";

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.operatorKeyword], color: "var(--syn-keyword)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--syn-string)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--syn-number)" },
  // Inside diff lines a brighter comment color is used (see --syn-comment-use below).
  { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: "var(--syn-comment-use, var(--syn-comment))", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "var(--syn-func)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--syn-type)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--syn-prop)" },
  { tag: [t.tagName, t.heading], color: "var(--syn-tag)", fontWeight: "600" },
  { tag: [t.meta, t.annotation, t.processingInstruction], color: "var(--syn-meta)" },
  { tag: [t.link, t.url], color: "var(--syn-string)", textDecoration: "underline" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "700" },
  { tag: t.invalid, color: "var(--del)" },
]);

export const theme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "var(--bg)", color: "var(--text)", fontSize: "var(--code-size)" },
  ".cm-scroller": { fontFamily: "var(--mono)", lineHeight: "1.55" },
  ".cm-content": { caretColor: "var(--amber)", padding: "8px 0 40vh" },
  // The drawn cursor. Without this it stays black, which is invisible on dark themes.
  ".cm-cursor, .cm-dropCursor": { borderLeft: "2px solid var(--amber)", marginLeft: "-1px" },
  "&.cm-focused .cm-cursor": { borderLeftColor: "var(--amber)" },
  ".cm-gutters": { backgroundColor: "var(--bg)", color: "var(--faint)", border: "none", paddingLeft: "6px" },
  ".cm-lineNumbers .cm-gutterElement": { padding: "0 12px 0 8px", minWidth: "44px", cursor: "pointer" },
  ".cm-activeLine": { backgroundColor: "var(--hover)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--muted)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--amber-sel) !important",
  },
  ".cm-selectionMatch": { backgroundColor: "var(--amber-soft)" },
  ".cm-matchingBracket": { outline: "1px solid var(--line-strong)", backgroundColor: "transparent" },
  ".cm-searchMatch": { backgroundColor: "var(--amber-soft)", outline: "1px solid var(--amber)" },
  ".cm-searchMatch-selected": { backgroundColor: "var(--amber-sel)" },
  ".cm-panels": { backgroundColor: "var(--panel)", color: "var(--text)", borderColor: "var(--line)" },
  ".cm-panels input, .cm-panels button": { fontFamily: "var(--ui)", fontSize: "var(--fs-sm)" },
  ".cm-foldGutter .cm-gutterElement": { color: "var(--faint)", padding: "0 4px" },
  ".cm-foldPlaceholder": { backgroundColor: "var(--raised)", border: "none", color: "var(--muted)" },
  // Diff colors (@codemirror/merge). Changed words get a light tint plus an underline, so the text on
  // them stays readable in every theme. Comments on diff lines move halfway toward the text color.
  ".cm-changedLine, .cm-insertedLine": { backgroundColor: "var(--add-bg) !important" },
  ".cm-deletedChunk, .cm-deletedLine": { backgroundColor: "var(--del-bg) !important" },
  ".cm-changedLine, .cm-insertedLine, .cm-deletedChunk, .cm-deletedLine, .cm-merge-a .cm-changedLine": {
    "--syn-comment-use": "color-mix(in srgb, var(--syn-comment) 50%, var(--text))",
  },
  ".cm-changedText, .cm-insertedText": {
    background: "color-mix(in srgb, var(--add) 15%, transparent) !important",
    textDecoration: "none",
    boxShadow: "inset 0 -1.5px 0 color-mix(in srgb, var(--add) 70%, transparent)",
  },
  ".cm-deletedText": {
    background: "color-mix(in srgb, var(--del) 15%, transparent) !important",
    textDecoration: "none",
    boxShadow: "inset 0 -1.5px 0 color-mix(in srgb, var(--del) 70%, transparent)",
  },
  ".cm-merge-a .cm-changedLine, .cm-merge-a .cm-changedLineGutter": { backgroundColor: "var(--del-bg) !important" },
  ".cm-merge-a .cm-changedText": {
    background: "color-mix(in srgb, var(--del) 15%, transparent) !important",
    boxShadow: "inset 0 -1.5px 0 color-mix(in srgb, var(--del) 70%, transparent)",
  },
  ".cm-changedLineGutter, .cm-insertedLineGutter": { backgroundColor: "var(--add) !important", width: "3px" },
  ".cm-deletedLineGutter": { backgroundColor: "var(--del) !important" },
  ".cm-collapsedLines": {
    backgroundColor: "var(--panel)",
    color: "var(--muted)",
    fontFamily: "var(--ui)",
    fontSize: "var(--fs-sm)",
    padding: "4px 12px",
    background: "var(--panel)",
    cursor: "pointer",
  },
  ".cm-collapsedLines:before, .cm-collapsedLines:after": { display: "none" },
});

// A short flash on one line, so the eye finds the spot after a jump. It fades out and is removed.
const setFlash = StateEffect.define<number | null>();
const flashField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    for (const e of tr.effects) {
      if (e.is(setFlash)) {
        if (e.value === null) return Decoration.none;
        const line = tr.state.doc.lineAt(e.value);
        return Decoration.set([Decoration.line({ class: "cm-flash" }).range(line.from)]);
      }
    }
    return deco.map(tr.changes);
  },
  provide: (f) => EditorView.decorations.from(f),
});

const flashTheme = EditorView.theme({
  ".cm-flash": { animation: "cm-flash 1.6s ease-out forwards" },
  "@keyframes cm-flash": {
    "0%": { backgroundColor: "var(--amber-sel)" },
    "60%": { backgroundColor: "var(--amber-soft)" },
    "100%": { backgroundColor: "transparent" },
  },
});

export const flash: Extension = [flashField, flashTheme];

export type Find = { text: string; regex: boolean; matchCase: boolean };

/**
 * Scrolls to a line, selects the match on it (or puts the cursor at the line start), and flashes the line.
 */
export function jumpTo(view: EditorView, lineNo: number, find?: Find) {
  const doc = view.state.doc;
  const line = doc.line(Math.max(1, Math.min(lineNo, doc.lines)));
  let from = line.from;
  let to = line.from;
  if (find?.text) {
    try {
      const pattern = find.regex ? find.text : find.text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      const m = new RegExp(pattern, find.matchCase ? "" : "i").exec(line.text);
      if (m && m[0]) {
        from = line.from + m.index;
        to = from + m[0].length;
      }
    } catch {
      /* a bad pattern: just go to the line */
    }
  }
  view.dispatch({
    selection: { anchor: from, head: to },
    effects: [EditorView.scrollIntoView(from, { y: "center" }), setFlash.of(line.from)],
  });
  view.focus();
  setTimeout(() => {
    if (view.dom.isConnected) view.dispatch({ effects: setFlash.of(null) });
  }, 1700);
}

/**
 * A drag that starts inside the selection normally moves the selected text (drag and drop). In a viewer
 * that feels broken: after a search jump the found word is selected, and dragging from it highlights
 * strange places. So a plain press inside the selection clears it first, and every drag selects.
 */
const noTextDrag = EditorView.domEventHandlers({
  mousedown(e, view) {
    if (e.button !== 0 || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || e.detail > 1) return false;
    const sel = view.state.selection.main;
    if (sel.empty) return false;
    const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
    if (pos == null || pos < sel.from || pos > sel.to) return false;
    view.dispatch({ selection: { anchor: pos } });
    return false; // CodeMirror goes on with a normal click or drag from here
  },
  dragstart(e) {
    e.preventDefault();
    return true;
  },
});

/** Clicking a line number selects that line; shift-click extends the selection. */
const gutterSelect = lineNumbers({
  domEventHandlers: {
    mousedown(view, block, event) {
      const e = event as MouseEvent;
      const line = view.state.doc.lineAt(block.from);
      const anchor = e.shiftKey ? view.state.selection.main.anchor : line.from;
      const head = e.shiftKey && anchor > line.from ? line.from : line.to;
      view.dispatch({ selection: { anchor, head } });
      view.focus();
      return true;
    },
  },
});

export function baseExtensions(opts: { editable: Compartment; language: Compartment; editing: boolean }): Extension[] {
  return [
    gutterSelect,
    foldGutter({ openText: "▾", closedText: "▸" }),
    highlightSpecialChars(),
    drawSelection(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    highlightSelectionMatches(),
    bracketMatching(),
    history(),
    search({ top: true }),
    syntaxHighlighting(highlight),
    keymap.of([...searchKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
    theme,
    flash,
    noTextDrag,
    opts.language.of([]),
    opts.editable.of(editableState(opts.editing)),
  ];
}

export function diffExtensions(language: Extension): Extension[] {
  return [
    lineNumbers(),
    highlightSpecialChars(),
    drawSelection(),
    highlightSelectionMatches(),
    search({ top: true }),
    keymap.of([...searchKeymap, ...defaultKeymap]),
    syntaxHighlighting(highlight),
    theme,
    noTextDrag,
    language,
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
  ];
}

export function editableState(on: boolean): Extension {
  return [EditorState.readOnly.of(!on), EditorView.editable.of(true)];
}

const cache = new Map<string, Promise<Extension>>();

/** Loads syntax support for a file name. Language packs are loaded only when first needed. */
export function languageFor(fileName: string): Promise<Extension> {
  const desc = LanguageDescription.matchFilename(languages, fileName);
  if (!desc) return Promise.resolve([]);
  let p = cache.get(desc.name);
  if (!p) {
    p = desc.load().then((l) => l as Extension).catch(() => [] as Extension);
    cache.set(desc.name, p);
  }
  return p;
}

/** The database tool's query editor: SQL (or Redis commands), completion, and Ctrl+Enter to run. */
export function queryExtensions(opts: { language: Compartment; run: () => void; change: (text: string) => void }): Extension[] {
  return [
    lineNumbers(),
    highlightSpecialChars(),
    drawSelection(),
    highlightActiveLine(),
    bracketMatching(),
    closeBrackets(),
    history(),
    autocompletion({ activateOnTyping: true }),
    syntaxHighlighting(highlight),
    keymap.of([
      { key: "Mod-Enter", run: () => (opts.run(), true) },
      { key: "Shift-Enter", run: () => (opts.run(), true) },
      ...closeBracketsKeymap,
      ...completionKeymap,
      ...defaultKeymap,
      ...historyKeymap,
      indentWithTab,
    ]),
    theme,
    EditorView.lineWrapping,
    EditorView.updateListener.of((u) => u.docChanged && opts.change(u.state.doc.toString())),
    opts.language.of([]),
  ];
}
