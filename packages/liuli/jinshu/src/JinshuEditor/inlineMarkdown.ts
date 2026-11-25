/**
 * The editor's inline shorthand. Parsing and printing live in the schema's one inline codec (shared with
 * Markdown files); this module re-exports them and adds the format bar's toggles.
 */
export { atomsIn, inlinesToShorthand, shorthandToInlines } from '../schema/inlineMarkdown';
export type { InlineDialect } from '../schema/inlineMarkdown';

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
