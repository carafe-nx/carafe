const palettes = [
  ["#4f8f5a", "#1e4a5c"],
  ["#b8862f", "#6b1c1c"],
  ["#7a1414", "#1c0b0b"],
  ["#8a8634", "#2d3a1e"],
  ["#3e5c9a", "#1b2340"],
  ["#8e2846", "#3b1020"],
] as const;

function hash(text: string): number {
  let value = 0;
  for (const char of text) {
    value = (value * 31 + char.charCodeAt(0)) >>> 0;
  }
  return value;
}

export function artPalette(title: string): readonly [string, string] {
  return palettes[hash(title) % palettes.length] ?? palettes[0];
}
