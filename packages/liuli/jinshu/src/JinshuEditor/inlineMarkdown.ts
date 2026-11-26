/**
 * The editor's inline shorthand. Parsing and printing live in the schema's one inline codec (shared with
 * Markdown files); this module re-exports them and adds the format bar's toggles.
 */
export { atomsIn, inlinesToShorthand, shorthandToInlines } from '../schema/inlineMarkdown';
export type { InlineDialect } from '../schema/inlineMarkdown';

/** A selection in a field's text: UTF-16 offsets, `start <= end`, equal for a caret. */
export interface TextRange {
  readonly start: number;
  readonly end: number;
}

/** A field's text after a format, with the selection to show in it. */
export interface FormattedText extends TextRange {
  readonly text: string;
}

/**
 * Where `mark` sits in each delimiter on a line, in order. `*` and `~` come in runs: a run of one or
 * two is that mark, a run of three holds both, the double one outside (`***x***` is bold around italic).
 */
function delimitersOf(text: string, from: number, to: number, mark: string): number[] {
  const found: number[] = [];
  const n = mark.length;
  const twinned = mark[0] === '*' || mark[0] === '~';
  for (let at = from; at < to; ) {
    if (twinned) {
      if (text[at] !== mark[0]) {
        at += 1;
        continue;
      }
      let run = 1;
      while (text[at + run] === mark[0]) run += 1;
      if (run === n || run === 3) {
        const opening = found.length % 2 === 0;
        // Outer marks sit at the run's outside edge: the start of an opener, the end of a closer.
        const outer = n === 2;
        found.push(opening === outer ? at : at + run - n);
      }
      at += run;
    } else if (text.startsWith(mark, at)) {
      found.push(at);
      at += n;
    } else at += 1;
  }
  return found;
}

/**
 * The `mark...mark` pair on the selection's line that holds the selection (or exactly is it), as
 * the offsets of its two delimiters; null when the selection is not inside one.
 */
export function markSpanAt(text: string, range: TextRange, mark: string): { readonly open: number; readonly close: number } | null {
  const lineStart = text.lastIndexOf('\n', range.start - 1) + 1;
  const newline = text.indexOf('\n', range.end);
  const lineEnd = newline < 0 ? text.length : newline;
  const delimiters = delimitersOf(text, lineStart, lineEnd, mark);
  for (let i = 0; i + 1 < delimiters.length; i += 2) {
    const open = delimiters[i];
    const close = delimiters[i + 1];
    if (close === open + mark.length) continue;
    const inside = open + mark.length <= range.start && range.end <= close;
    const whole = range.start <= open && close + mark.length <= range.end && range.end - range.start <= close + mark.length - open;
    if (inside || whole) return { open, close };
  }
  return null;
}

const WORD_RE = /[\p{Script=Latin}\p{Script=Greek}\p{Script=Cyrillic}\p{N}_'’-]/u;

/**
 * Turns a mark on or off where the person is: inside a marked span it comes off that span; a
 * selection gets wrapped; a caret in a word wraps that word; elsewhere a selected placeholder goes
 * in, so typing replaces it. The selection stays on the same text.
 */
export function toggleMarkAt(text: string, range: TextRange, mark: string, placeholder: string): FormattedText {
  const n = mark.length;
  const span = markSpanAt(text, range, mark);
  if (span) {
    const { open, close } = span;
    const moved = (at: number) => (at <= open ? at : at <= open + n ? open : at <= close ? at - n : at <= close + n ? close - n : at - 2 * n);
    return { text: text.slice(0, open) + text.slice(open + n, close) + text.slice(close + n), start: moved(range.start), end: moved(range.end) };
  }
  let { start, end } = range;
  if (start === end) {
    while (start > 0 && WORD_RE.test(text[start - 1])) start -= 1;
    while (end < text.length && WORD_RE.test(text[end])) end += 1;
    if (start === end) {
      const out = text.slice(0, start) + mark + placeholder + mark + text.slice(end);
      return { text: out, start: start + n, end: start + n + placeholder.length };
    }
    const out = text.slice(0, start) + mark + text.slice(start, end) + mark + text.slice(end);
    return { text: out, start: range.start + n, end: range.end + n };
  }
  // A double-click takes the space after a word too; the mark stays on the word.
  while (end > start && /\s/.test(text[end - 1])) end -= 1;
  while (start < end && /\s/.test(text[start])) start += 1;
  if (start === end) return { text, start: range.start, end: range.end };
  const out = text.slice(0, start) + mark + text.slice(start, end) + mark + text.slice(end);
  return { text: out, start: start + n, end: end + n };
}

/** Replaces the selection with a link to `href` showing `label`, else the selected text, else `href`; the link's text ends selected. */
export function linkAt(text: string, range: TextRange, href: string, label: string): FormattedText {
  const shown = label || text.slice(range.start, range.end).trim() || href;
  const out = `${text.slice(0, range.start)}[${shown}](${href})${text.slice(range.end)}`;
  return { text: out, start: range.start + 1, end: range.start + 1 + shown.length };
}

/**
 * Breaks a line at the selection, which goes. A mark open across the break is closed before it and
 * opened again after, so both halves keep their formatting; a half left with nothing inside a mark
 * drops that mark.
 */
export function splitAt(text: string, range: TextRange, marks: readonly string[]): readonly [string, string] {
  const held = marks
    .map(mark => ({ mark, span: markSpanAt(text, { start: range.start, end: range.start }, mark) }))
    .flatMap(({ mark, span }) => (span && range.end <= span.close ? [{ mark, ...span }] : []))
    .sort((a, b) => b.open - a.open);
  let before = text.slice(0, range.start);
  let after = text.slice(range.end);
  // Innermost first: closing it, then the one around it, nests the closers as the openers were.
  for (const { mark, open, close } of held) {
    before = range.start === open + mark.length ? before.slice(0, -mark.length) : before + mark;
    after = range.end === close ? after.slice(mark.length) : mark + after;
  }
  return [before, after];
}
