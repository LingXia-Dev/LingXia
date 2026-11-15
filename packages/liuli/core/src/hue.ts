/** One of the theme's eight categorical hues, `hue-1` to `hue-8`: a colour for a kind of thing, never a status. */
export type Hue = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;

/** The same hue for the same text every time: a person's name, a tag, a project key. */
export function hueFor(text: string): Hue {
  let hash = 0;
  for (const char of text) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) % 9973;
  return ((hash % 8) + 1) as Hue;
}
