import { inlineKind } from '../schema/helpers';
import { normalizeInlines, validateLink } from '../schema/validate';
import type { Inline, InlineAtom, InlineRun } from '../schema/types';

/**
 * Lossless inline shorthand for in-place editing (Typora / quiet-md feel):
 * `**bold**`, `*italic*`, `~~strike~~`, `++underline++`, `` `code` ``, `==highlight==` (`=={warning}…==` for
 * other tones), `^sup^`, `~sub~`, `[text](url)`, `$math$`, and `⟦fallback#id⟧` for an inline atom.
 * Literal delimiter characters are backslash-escaped, so `shorthandToInlines(inlinesToShorthand(x))` is `x`.
 */

const ESCAPED = new Set(['\\', '*', '_', '~', '=', '+', '^', '`', '[', ']', '$', '⟦', '⟧']);
/** Characters a backslash may escape when reading; `{` only matters right after `==`, `#` and `)` inside tokens. */
const UNESCAPABLE = new Set([...ESCAPED, '{', '#', ')']);
const TONES = new Set(['accent', 'warning', 'success', 'danger']);

function escapeLiteral(text: string): string {
  let out = '';
  for (const ch of text) out += ESCAPED.has(ch) ? `\\${ch}` : ch;
  return out;
}

function escapeOnly(text: string, chars: string): string {
  let out = '';
  for (const ch of text) out += ch === '\\' || chars.includes(ch) ? `\\${ch}` : ch;
  return out;
}

/** All inline atoms in `inlines`, for `shorthandToInlines(..., { existingAtoms })`. */
export function atomsIn(inlines: readonly Inline[] | undefined): InlineAtom[] {
  return Array.isArray(inlines) ? (inlines.filter(i => inlineKind(i) === 'atom') as InlineAtom[]) : [];
}

export function inlinesToShorthand(inlines: readonly Inline[]): string {
  let out = '';
  for (const item of inlines) {
    switch (inlineKind(item)) {
      case 'break':
        out += '\n';
        break;
      case 'math':
        out += `$${(item as { math: string }).math}$`;
        break;
      case 'atom': {
        const atom = item as InlineAtom;
        out += `⟦${escapeOnly(atom.fallback, '⟧#')}#${atom.id}⟧`;
        break;
      }
      case 'text': {
        const run = item as InlineRun;
        let s = run.code ? `\`${escapeOnly(run.text, '`')}\`` : escapeLiteral(run.text);
        if (run.script === 'super') s = `^${s}^`;
        if (run.script === 'sub') s = `~${s}~`;
        if (run.underline) s = `++${s}++`;
        if (run.strike) s = `~~${s}~~`;
        if (run.italic) s = `*${s}*`;
        if (run.bold) s = `**${s}**`;
        if (run.highlight) {
          const tone = run.highlight;
          s = tone === 'accent' ? `==${s.startsWith('{') ? `\\${s}` : s}==` : `=={${tone}}${s}==`;
        }
        if (run.link) s = `[${s}](${escapeOnly(run.link, ')')})`;
        out += s;
        break;
      }
    }
  }
  return out;
}

type Marks = Partial<Omit<InlineRun, 'text'>>;

interface Delim {
  readonly open: string;
  readonly close: string;
  readonly marks: Marks;
  /** Single-line spans (script). */
  readonly inline?: boolean;
}

// Longer delimiters first so `**` wins over `*` and `~~` over `~`.
const DELIMS: readonly Delim[] = [
  { open: '**', close: '**', marks: { bold: true } },
  { open: '~~', close: '~~', marks: { strike: true } },
  { open: '++', close: '++', marks: { underline: true } },
  { open: '==', close: '==', marks: { highlight: 'accent' } },
  { open: '*', close: '*', marks: { italic: true } },
  { open: '~', close: '~', marks: { script: 'sub' }, inline: true },
  { open: '^', close: '^', marks: { script: 'super' }, inline: true },
];

/** Index of the first `ch` at or after `from` that is not backslash-escaped, or -1. */
function findUnescaped(src: string, ch: string, from: number): number {
  for (let i = from; i < src.length; i++) {
    if (src[i] === '\\' && i + 1 < src.length) {
      i += 1;
      continue;
    }
    if (src.startsWith(ch, i)) return i;
  }
  return -1;
}

function unescapeIn(text: string): string {
  let out = '';
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '\\' && i + 1 < text.length && UNESCAPABLE.has(text[i + 1])) {
      out += text[i + 1];
      i += 1;
    } else {
      out += text[i];
    }
  }
  return out;
}

/**
 * Parses inline shorthand back into canonical `Inline[]`. Never throws; an unmatched delimiter stays literal text.
 * Atom tokens resolve against `existingAtoms` by id; an unknown token stays literal text.
 */
export function shorthandToInlines(
  source: string,
  options?: { readonly inHeading?: boolean; readonly existingAtoms?: readonly InlineAtom[] },
): Inline[] {
  const src = String(source ?? '').replace(/\r\n?/g, '\n');
  const atoms = new Map((options?.existingAtoms ?? []).map(a => [a.id, a] as const));
  // (pos|close) pairs already known to have no closing delimiter, so failed spans are scanned once.
  const failed = new Set<string>();

  const withMarks = (items: readonly Inline[], marks: Marks): Inline[] =>
    items.map(item => (inlineKind(item) === 'text' ? ({ ...marks, ...item } as Inline) : item));

  /** Parses from `pos` until `close` (or end when `close` is null). Returns null when `close` never appears. */
  function parseSeq(pos: number, close: string | null, singleLine: boolean): { items: Inline[]; end: number } | null {
    const items: Inline[] = [];
    let text = '';
    const flush = () => {
      if (!text) return;
      const parts = text.split('\n');
      parts.forEach((part, p) => {
        if (part) items.push({ text: part });
        if (p < parts.length - 1) items.push({ break: true });
      });
      text = '';
    };
    let i = pos;
    while (i < src.length) {
      if (close !== null && src.startsWith(close, i)) {
        flush();
        return { items, end: i + close.length };
      }
      const ch = src[i];
      if (singleLine && ch === '\n') return null;
      if (ch === '\\' && i + 1 < src.length && UNESCAPABLE.has(src[i + 1])) {
        text += src[i + 1];
        i += 2;
        continue;
      }
      if (ch === '`') {
        const end = findUnescaped(src, '`', i + 1);
        if (end > i + 1) {
          flush();
          items.push({ text: unescapeIn(src.slice(i + 1, end)), code: true });
          i = end + 1;
          continue;
        }
      }
      if (ch === '$') {
        const end = src.indexOf('$', i + 1);
        if (end > i + 1 && !src.slice(i + 1, end).includes('\n')) {
          flush();
          items.push({ math: src.slice(i + 1, end) });
          i = end + 1;
          continue;
        }
      }
      if (ch === '⟦') {
        const end = findUnescaped(src, '⟧', i + 1);
        const hash = end > 0 ? src.lastIndexOf('#', end) : -1;
        const atom = hash > i ? atoms.get(src.slice(hash + 1, end)) : undefined;
        if (atom) {
          flush();
          items.push(atom);
          i = end + 1;
          continue;
        }
      }
      if (ch === '[') {
        const label = !failed.has(`${i}|]`) ? parseSeq(i + 1, ']', false) : null;
        if (!label) failed.add(`${i}|]`);
        if (label && src[label.end] === '(') {
          const closeParen = findUnescaped(src, ')', label.end + 1);
          if (closeParen > label.end + 1) {
            const href = unescapeIn(src.slice(label.end + 1, closeParen)).trim();
            flush();
            // An unsafe or malformed target keeps its text without the link, so the edit still saves.
            const safe = validateLink(href) === null;
            items.push(...(safe ? withMarks(label.items, { link: href }) : label.items));
            i = closeParen + 1;
            continue;
          }
        }
      }
      const delim = DELIMS.find(d => src.startsWith(d.open, i));
      if (delim) {
        let start = i + delim.open.length;
        let marks = delim.marks;
        if (delim.open === '==') {
          const tone = /^\{([a-z]+)\}/.exec(src.slice(start));
          if (tone && TONES.has(tone[1])) {
            marks = { highlight: tone[1] as InlineRun['highlight'] };
            start += tone[0].length;
          }
        }
        const key = `${start}|${delim.close}`;
        const inner = !failed.has(key) && !src.startsWith(delim.close, start) ? parseSeq(start, delim.close, delim.inline === true) : null;
        if (inner) {
          flush();
          items.push(...withMarks(inner.items, marks));
          i = inner.end;
          continue;
        }
        failed.add(key);
        text += delim.open;
        i += delim.open.length;
        continue;
      }
      text += ch;
      i += 1;
    }
    if (close !== null) return null;
    flush();
    return { items, end: i };
  }

  const parsed = parseSeq(0, null, false)!;
  return normalizeInlines(parsed.items, { inHeading: options?.inHeading });
}

const CLAUSE_BREAK_RE = /[\s，。！？；：、,.!?;:（）()【】「」『』\n]/;

/** Checks whether the trailing token or whole short text currently carries `prefix...suffix`. */
export function hasTrailingShorthandMark(text: string, prefix: string, suffix = prefix): boolean {
  const trimmedEnd = text.replace(/\s+$/, '');
  if (!trimmedEnd.endsWith(suffix) || trimmedEnd.length <= prefix.length + suffix.length) return false;
  if (prefix === '*' && trimmedEnd.endsWith('**')) return false;
  if (prefix === '~' && trimmedEnd.endsWith('~~')) return false;
  const openAt = trimmedEnd.lastIndexOf(prefix, trimmedEnd.length - suffix.length - 1);
  if (openAt < 0) return false;
  if (prefix === '*' && openAt > 0 && trimmedEnd[openAt - 1] === '*') return false;
  if (prefix === '~' && openAt > 0 && trimmedEnd[openAt - 1] === '~') return false;
  return openAt < trimmedEnd.length - suffix.length;
}

/**
 * Smart toolbar toggle:
 * - Never wraps an entire multi-sentence paragraph by accident!
 * - If `wholeText` is true (e.g. a table cell) or the whole text is a single short phrase (<= 14 chars, no clause breaks), toggles the whole phrase.
 * - If the trailing token is already wrapped in `prefix...suffix`, unwraps that trailing token.
 * - Otherwise wraps ONLY the trailing word / CJK clause at the caret end (or appends a marked placeholder if at whitespace/empty).
 */
export function toggleShorthandWrap(
  text: string,
  prefix: string,
  suffix = prefix,
  options?: { readonly wholeText?: boolean; readonly placeholder?: string },
): string {
  const placeholder = options?.placeholder ?? '文本';
  const trimmed = text.trim();
  if (!trimmed) return `${prefix}${placeholder}${suffix}`;

  // 1. Whole-cell or short single-token text
  const isSingleShortToken = trimmed.length <= 14 && !CLAUSE_BREAK_RE.test(trimmed);
  if (options?.wholeText || isSingleShortToken) {
    if (
      trimmed.startsWith(prefix) &&
      trimmed.endsWith(suffix) &&
      trimmed.length > prefix.length + suffix.length &&
      !(prefix === '*' && trimmed.startsWith('**')) &&
      !(prefix === '~' && trimmed.startsWith('~~'))
    ) {
      return trimmed.slice(prefix.length, trimmed.length - suffix.length);
    }
    return `${prefix}${trimmed}${suffix}`;
  }

  // 2. If text ends with whitespace or punctuation, append a fresh marked placeholder
  const trailingWsMatch = /\s+$/.exec(text);
  const trailingWs = trailingWsMatch ? trailingWsMatch[0] : '';
  const core = trailingWs ? text.slice(0, -trailingWs.length) : text;
  if (!core || CLAUSE_BREAK_RE.test(core[core.length - 1])) {
    return `${text}${prefix}${placeholder}${suffix}`;
  }

  // 3. If trailing segment is already wrapped in `prefix...suffix`, unwrap just that trailing segment!
  if (hasTrailingShorthandMark(core, prefix, suffix)) {
    const innerEnd = core.length - suffix.length;
    const openAt = core.lastIndexOf(prefix, innerEnd - 1);
    if (openAt >= 0) {
      const inner = core.slice(openAt + prefix.length, innerEnd);
      return `${core.slice(0, openAt)}${inner}${trailingWs}`;
    }
  }

  // 4. Isolate trailing ASCII word or trailing CJK phrase after last punctuation/whitespace
  const asciiTail = /[A-Za-z0-9_./+-]+$/.exec(core);
  if (asciiTail) {
    const start = asciiTail.index;
    return `${core.slice(0, start)}${prefix}${asciiTail[0]}${suffix}${trailingWs}`;
  }

  let cut = core.length - 1;
  while (cut > 0 && !CLAUSE_BREAK_RE.test(core[cut - 1])) {
    cut -= 1;
  }
  // Keep CJK auto-target concise (at most last 6 code points if user wrote a long unpunctuated clause)
  const clause = core.slice(cut);
  const cps = Array.from(clause);
  if (cps.length > 6) {
    const tail = cps.slice(-4).join('');
    const headLen = core.length - tail.length;
    return `${core.slice(0, headLen)}${prefix}${tail}${suffix}${trailingWs}`;
  }
  return `${core.slice(0, cut)}${prefix}${clause}${suffix}${trailingWs}`;
}
