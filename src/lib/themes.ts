// The themes. Colors live in app.css. These swatches are only for the pickers.

export type ThemeInfo = { id: string; name: string; note: string; swatch: [string, string, string, string, string] };

export const THEMES: ThemeInfo[] = [
  { id: "system", name: "Follow Windows", note: "Slate or Daylight, like Windows", swatch: ["#161a21", "#fbfbfc", "#f0b54a", "#5fbf8b", "#e0716f"] },
  { id: "slate", name: "Slate", note: "Blue-gray dark", swatch: ["#161a21", "#1b2029", "#f0b54a", "#5fbf8b", "#e0716f"] },
  { id: "daylight", name: "Daylight", note: "Clean light", swatch: ["#fbfbfc", "#f2f3f6", "#b7791f", "#22884f", "#c2413e"] },
  { id: "forest", name: "Forest", note: "Deep green, mint accent", swatch: ["#0f1a15", "#122019", "#5ed19a", "#a6d96a", "#ef7b72"] },
  { id: "pine", name: "Pine", note: "Almost black green, emerald accent", swatch: ["#0a110e", "#0e1713", "#34d399", "#a3e635", "#f87171"] },
  { id: "moss", name: "Moss", note: "Olive green, soft and warm", swatch: ["#171a14", "#1b1f17", "#a7c957", "#7fcf8c", "#e0786b"] },
  { id: "ocean", name: "Ocean", note: "Deep teal blue", swatch: ["#0d1b24", "#102230", "#4fc3d9", "#6fd39a", "#f07a7a"] },
  { id: "plum", name: "Plum", note: "Dark purple, orchid accent", swatch: ["#1a1520", "#1f1926", "#e28ad9", "#7fd3a0", "#f27d8a"] },
  { id: "ink", name: "Ink", note: "Near black, violet accent", swatch: ["#0b0c0f", "#111317", "#a78bfa", "#4fd18b", "#f47174"] },
  { id: "fjord", name: "Fjord", note: "Cool blue, soft frost colors", swatch: ["#2e3440", "#292e39", "#88c0d0", "#a3be8c", "#bf616a"] },
  { id: "ember", name: "Ember", note: "Warm dark, orange accent", swatch: ["#1c1917", "#211d1a", "#f59e5b", "#9ccf7a", "#ef6f5e"] },
  { id: "sand", name: "Sand", note: "Warm light, like paper", swatch: ["#fbf6ea", "#f3ecdc", "#b5651d", "#4d7c2a", "#b8392f"] },
  { id: "contrast", name: "High contrast", note: "Black and bright, easiest to read", swatch: ["#000000", "#171717", "#ffd24a", "#62ec9e", "#ff7373"] },
];
