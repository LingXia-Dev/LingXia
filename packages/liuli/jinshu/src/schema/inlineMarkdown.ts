/**
 * The one inline codec: Jinshu inlines to and from Markdown-style text, used by the editor's shorthand
 * and by Markdown files.
 *
 * - Dialect `markdown`: CommonMark + GFM inlines (`**`, `*`, `~~`, `` ` ``, links, autolinks) and `$math$`.
 * - Dialect `jinshu`: the same plus `++underline++`, `==mark==` / `=={tone}…==`, `^sup^`, `~sub~` and
 *   `⟦fallback#id⟧` for an inline atom; no autolinks.
 *
 * Shorthand (the editor) and file text differ only in mode: in a file a newline is a soft break, `\`
 * before a newline is a hard one, entities are read and delimiters follow CommonMark's flanking rules
 * exactly; in shorthand every newline is a line break and a delimiter only has to touch text.
 */
import { inlineKind } from './helpers';
import { mathWritable } from './profile';
import { normalizeInlines, validateLink, validateSrc } from './validate';
import type { FormatRule, HighlightTone, Inline, InlineAtom, InlineMath, InlineRun } from './types';

export type InlineDialect = 'jinshu' | 'markdown';

type Marks = { -readonly [K in keyof Omit<InlineRun, 'text'>]?: InlineRun[K] };

export interface LinkReference {
  readonly href: string;
  readonly title?: string;
}

/** @internal Options of the full parser; the public shorthand functions fill them in. */
export interface InlineParseOptions {
  readonly dialect: InlineDialect;
  /** File text: CommonMark flanking, entities, soft line breaks. */
  readonly strict: boolean;
  readonly inHeading?: boolean;
  readonly existingAtoms?: readonly InlineAtom[];
  readonly refs?: ReadonlyMap<string, LinkReference>;
  readonly warn?: (message: string, rule: FormatRule) => void;
  /** Read symbols (emoji, `€`) as ordinary characters, as older CommonMark did; the writer checks both. */
  readonly symbolsAsText?: boolean;
}

/** An image met inside text; the block reader decides whether it stands alone. */
export interface InlineImage {
  readonly image: true;
  readonly src: string;
  readonly alt: string;
  readonly source: string;
  readonly link?: string;
}

// ============================================================================
// Characters
// ============================================================================

const ASCII_PUNCT_RE = /[!-/:-@[-`{-~]/;
const PUNCT_RE = /[\p{P}\p{S}]/u;
const PUNCT_P_RE = /\p{P}/u;
const WS_RE = /[\t\n\f\r\p{Zs}]/u;
const CJK_RE = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\u3000-\u303f\uff00-\uffef]/u;
const TONES: ReadonlySet<string> = new Set(['accent', 'warning', 'success', 'danger']);

const isWs = (c: string | undefined) => c === undefined || WS_RE.test(c);
const isAsciiPunct = (c: string | undefined) => c !== undefined && c.length === 1 && ASCII_PUNCT_RE.test(c);
type PunctFn = (c: string | undefined) => boolean;
const punctPS: PunctFn = c => c !== undefined && PUNCT_RE.test(c);
// Older CommonMark (and some renderers) count only P as punctuation; the writer satisfies both.
const punctP: PunctFn = c => c !== undefined && (PUNCT_P_RE.test(c) || isAsciiPunct(c));

function lastChar(s: string): string | undefined {
  if (!s) return undefined;
  const cp = s.codePointAt(s.length - 1);
  if (cp !== undefined && cp >= 0xdc00 && cp <= 0xdfff && s.length >= 2) return s.slice(-2);
  return s[s.length - 1];
}

function charAt(s: string, i: number): string | undefined {
  if (i < 0 || i >= s.length) return undefined;
  const cp = s.codePointAt(i)!;
  return cp > 0xffff ? s.slice(i, i + 2) : s[i];
}

function charBefore(s: string, i: number): string | undefined {
  return i <= 0 ? undefined : lastChar(s.slice(Math.max(0, i - 2), i));
}

interface Flank {
  readonly left: boolean;
  readonly right: boolean;
}

function flanking(prev: string | undefined, next: string | undefined, strict: boolean, punct: PunctFn): Flank {
  if (!strict) return { left: !isWs(next), right: !isWs(prev) };
  return {
    left: !isWs(next) && (!punct(next) || isWs(prev) || punct(prev)),
    right: !isWs(prev) && (!punct(prev) || isWs(next) || punct(next)),
  };
}

function canOpenClose(ch: string, prev: string | undefined, next: string | undefined, strict: boolean, punct: PunctFn): { open: boolean; close: boolean } {
  const f = flanking(prev, next, strict, punct);
  if (ch === '_') return { open: f.left && (!f.right || punct(prev)), close: f.right && (!f.left || punct(next)) };
  return { open: f.left, close: f.right };
}

/** Delimiter characters of a dialect and the run lengths that can act. */
function delimiterRunOk(ch: string, len: number, dialect: InlineDialect): boolean {
  switch (ch) {
    case '*':
    case '_':
      return true;
    case '~':
      return dialect === 'markdown' ? len <= 2 : len <= 3;
    case '+':
    case '=':
      return dialect === 'jinshu' && len === 2;
    case '^':
      return dialect === 'jinshu' && len === 1;
    default:
      return false;
  }
}

const DELIM_CHARS = new Set(['*', '_', '~', '+', '=', '^']);

// ============================================================================
// Entities
// ============================================================================

const NAMED_ENTITIES: Readonly<Record<string, string>> = {
  amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0', copy: '©', reg: '®', trade: '™',
  hellip: '…', mdash: '—', ndash: '–', lsquo: '‘', rsquo: '’', ldquo: '“', rdquo: '”', laquo: '«',
  raquo: '»', middot: '·', bull: '•', deg: '°', plusmn: '±', times: '×', divide: '÷', para: '¶',
  sect: '§', euro: '€', pound: '£', yen: '¥', cent: '¢', larr: '←', rarr: '→', uarr: '↑', darr: '↓',
  harr: '↔', le: '≤', ge: '≥', ne: '≠', infin: '∞', minus: '−', frac12: '½', frac14: '¼', frac34: '¾',
  sup1: '¹', sup2: '²', sup3: '³', iexcl: '¡', iquest: '¿', shy: '\u00ad', zwj: '\u200d',
  zwnj: '\u200c', ensp: '\u2002', emsp: '\u2003', thinsp: '\u2009', dagger: '†', Dagger: '‡',
  permil: '‰', prime: '′', Prime: '″', micro: 'µ', alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ',
  pi: 'π', sigma: 'σ', lambda: 'λ', mu: 'μ', theta: 'θ', omega: 'ω', Delta: 'Δ', Omega: 'Ω', Sigma: 'Σ',
};

const ENTITY_RE = /^&(?:#[xX]([0-9a-fA-F]{1,6})|#([0-9]{1,7})|([A-Za-z][A-Za-z0-9]{1,31}));/;
/** Anything a renderer might read as an entity, known to us or not. */
const ENTITY_LIKE_RE = /^&(?:#[xX][0-9a-fA-F]{1,6}|#[0-9]{1,7}|[A-Za-z][A-Za-z0-9]{1,31});/;

function fromCodePoint(cp: number): string {
  return cp === 0 || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff) ? '\ufffd' : String.fromCodePoint(cp);
}

/** Decodes the entity at the start of `s`: its text and source length, or null. */
function readEntity(s: string): { text: string; length: number; name?: string } | null {
  const m = ENTITY_RE.exec(s);
  if (!m) return null;
  if (m[1] !== undefined) return { text: fromCodePoint(parseInt(m[1], 16)), length: m[0].length };
  if (m[2] !== undefined) return { text: fromCodePoint(parseInt(m[2], 10)), length: m[0].length };
  const named = NAMED_ENTITIES[m[3]];
  return named === undefined ? null : { text: named, length: m[0].length, name: m[3] };
}

/** Resolves backslash escapes (ASCII punctuation) and, in file text, entities. */
export function unescapeMarkdown(s: string, strict: boolean): string {
  let out = '';
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '\\' && isAsciiPunct(s[i + 1])) {
      out += s[i + 1];
      i += 1;
    } else if (c === '&' && strict) {
      const e = readEntity(s.slice(i));
      if (e) {
        out += e.text;
        i += e.length - 1;
      } else out += c;
    } else out += c;
  }
  return out;
}

// ============================================================================
// Links: destinations, titles, labels, autolinks, raw HTML
// ============================================================================

/** Parses a link destination at `pos`: `<…>` or a raw one with balanced parentheses. */
export function scanDestination(s: string, pos: number, strict: boolean): { dest: string; end: number } | null {
  if (s[pos] === '<') {
    for (let i = pos + 1; i < s.length; i++) {
      const c = s[i];
      if (c === '\\' && isAsciiPunct(s[i + 1])) {
        i += 1;
        continue;
      }
      if (c === '\n' || c === '<') return null;
      if (c === '>') return { dest: unescapeMarkdown(s.slice(pos + 1, i), strict), end: i + 1 };
    }
    return null;
  }
  let depth = 0;
  let i = pos;
  for (; i < s.length; i++) {
    const c = s[i];
    if (c === '\\' && isAsciiPunct(s[i + 1])) {
      i += 1;
      continue;
    }
    if (c.charCodeAt(0) <= 0x20 || c === '\u007f') break;
    if (c === '(') {
      depth += 1;
      if (depth > 32) return null;
    } else if (c === ')') {
      if (depth === 0) break;
      depth -= 1;
    }
  }
  if (i === pos || depth !== 0) return null;
  return { dest: unescapeMarkdown(s.slice(pos, i), strict), end: i };
}

/** Parses a link title (`"…"`, `'…'` or `(…)`) at `pos`. */
export function scanTitle(s: string, pos: number, strict: boolean): { title: string; end: number } | null {
  const open = s[pos];
  const close = open === '"' ? '"' : open === "'" ? "'" : open === '(' ? ')' : '';
  if (!close) return null;
  for (let i = pos + 1; i < s.length; i++) {
    const c = s[i];
    if (c === '\\' && isAsciiPunct(s[i + 1])) {
      i += 1;
      continue;
    }
    if (open === '(' && c === '(') return null;
    if (c === close) return { title: unescapeMarkdown(s.slice(pos + 1, i), strict), end: i + 1 };
  }
  return null;
}

/** Spaces, tabs and at most one line ending from `pos`. */
function skipSpaceNl(s: string, pos: number): number {
  let i = pos;
  let nl = false;
  while (i < s.length) {
    if (s[i] === ' ' || s[i] === '\t') i++;
    else if (s[i] === '\n' && !nl) {
      nl = true;
      i++;
    } else break;
  }
  return i;
}

/** Parses a link label `[…]` at `pos`; the raw text between the brackets. */
export function scanLabel(s: string, pos: number): { label: string; end: number } | null {
  if (s[pos] !== '[') return null;
  for (let i = pos + 1; i < s.length && i - pos <= 1000; i++) {
    const c = s[i];
    if (c === '\\' && isAsciiPunct(s[i + 1])) {
      i += 1;
      continue;
    }
    if (c === '[') return null;
    if (c === ']') {
      const label = s.slice(pos + 1, i);
      return /[^ \t\r\n]/.test(label) ? { label, end: i + 1 } : null;
    }
  }
  return null;
}

export function normalizeLabel(label: string): string {
  return label.replace(/^[ \t\r\n]+|[ \t\r\n]+$/g, '').replace(/[ \t\r\n]+/g, ' ').toLowerCase().toUpperCase();
}

const AUTOLINK_URI_RE = /^<([A-Za-z][A-Za-z0-9+.-]{1,31}:[^<>\x00-\x20]*)>/;
const AUTOLINK_EMAIL_RE = /^<([a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*)>/;
const ATTRIBUTE = `(?:\\s+[A-Za-z_:][A-Za-z0-9_.:-]*(?:\\s*=\\s*(?:[^"'=<>\`\\x00-\\x20]+|'[^']*'|"[^"]*"))?)`;
const HTML_TAG_RE = new RegExp(
  `^(?:<[A-Za-z][A-Za-z0-9-]*${ATTRIBUTE}*\\s*/?>|</[A-Za-z][A-Za-z0-9-]*\\s*>|<!---?>|<!--[\\s\\S]*?-->|<\\?[\\s\\S]*?\\?>|<![A-Za-z][^>]*>|<!\\[CDATA\\[[\\s\\S]*?\\]\\]>)`,
);

const EXT_URL_RE = /(?:https?:\/\/|www\.)[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*[^\s<]*/iy;
const EXT_EMAIL_RE = /[A-Za-z0-9._+-]+@[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+/y;
const EMAIL_CHAR_RE = /[A-Za-z0-9._+-]/;

/** GFM extended autolink at `pos`: www., http(s):// and e-mail addresses. */
function scanExtendedAutolink(s: string, pos: number): { text: string; href: string } | null {
  const prev = charBefore(s, pos);
  const c = s[pos];
  if ((c === 'h' || c === 'H' || c === 'w' || c === 'W') && (prev === undefined || isWs(prev) || '*_~('.includes(prev))) {
    EXT_URL_RE.lastIndex = pos;
    const m = EXT_URL_RE.exec(s);
    if (m) {
      let text = m[0];
      for (;;) {
        const before = text;
        if (/[?!.,:*_~'"]$/.test(text)) text = text.slice(0, -1);
        else if (text.endsWith(')') && text.split(')').length > text.split('(').length) text = text.slice(0, -1);
        else if (/&[A-Za-z0-9]+;$/.test(text)) text = text.replace(/&[A-Za-z0-9]+;$/, '');
        if (text === before) break;
      }
      const domain = /^(?:https?:\/\/|www\.)([A-Za-z0-9_.-]+)/i.exec(text)?.[1] ?? '';
      const segs = domain.split('.');
      if (segs.length >= 2 && !segs.slice(-2).some(seg => seg.includes('_')) && segs.every(Boolean)) {
        return { text, href: /^www\./i.test(text) ? `http://${text}` : text };
      }
    }
  }
  if (EMAIL_CHAR_RE.test(c) && (prev === undefined || !EMAIL_CHAR_RE.test(prev))) {
    EXT_EMAIL_RE.lastIndex = pos;
    const m = EXT_EMAIL_RE.exec(s);
    if (m) {
      let text = m[0];
      while (text.endsWith('.')) text = text.slice(0, -1);
      if (!/[-_]$/.test(text) && text.includes('.', text.indexOf('@'))) return { text, href: `mailto:${text}` };
    }
  }
  return null;
}

// ============================================================================
// Parsing
// ============================================================================

type PNode =
  | { k: 'text'; text: string; marks: Marks; dead?: true }
  | { k: 'break'; dead?: true }
  | { k: 'math'; math: string; dead?: true }
  | { k: 'atom'; atom: InlineAtom; dead?: true }
  | { k: 'image'; src: string; alt: string; source: string; marks: Marks; dead?: true };

interface Delim {
  node: number;
  ch: string;
  len: number;
  orig: number;
  canOpen: boolean;
  canClose: boolean;
  tone?: HighlightTone;
  prev: Delim | null;
  next: Delim | null;
}

interface Bracket {
  node: number;
  image: boolean;
  active: boolean;
  delimBottom: Delim | null;
  pos: number;
  prev: Bracket | null;
}

/** @internal Parses inline text into inlines and images; never throws. */
export function parseInlineNodes(source: string, options: InlineParseOptions): (Inline | InlineImage)[] {
  const src = String(source ?? '').replace(/\r\n?/g, '\n');
  const { dialect, strict } = options;
  const jinshu = dialect === 'jinshu';
  const atoms = new Map((options.existingAtoms ?? []).map(a => [a.id, a] as const));
  const nodes: PNode[] = [];
  let delims: Delim | null = null;
  let brackets: Bracket | null = null;
  let text = '';

  const flush = () => {
    if (text) nodes.push({ k: 'text', text, marks: {} });
    text = '';
  };
  const pushText = (t: string, marks: Marks = {}) => {
    flush();
    nodes.push({ k: 'text', text: t, marks });
    return nodes.length - 1;
  };
  const escapable = (c: string | undefined) =>
    c !== undefined && (isAsciiPunct(c) || (!strict && (c === ' ' || c === '\t' || (jinshu && (c === '⟦' || c === '⟧')))));

  const removeDelim = (d: Delim) => {
    if (d.prev) d.prev.next = d.next;
    if (d.next) d.next.prev = d.prev;
    else delims = d.prev;
  };

  const markFor = (opener: Delim, use: number): Marks => {
    switch (opener.ch) {
      case '*':
      case '_':
        return use === 2 ? { bold: true } : { italic: true };
      case '~':
        return jinshu && use === 1 ? { script: 'sub' } : { strike: true };
      case '+':
        return { underline: true };
      case '=':
        return { highlight: opener.tone ?? 'accent' };
      default:
        return { script: 'super' };
    }
  };

  const applyMarks = (from: number, to: number, marks: Marks) => {
    for (let n = from + 1; n < to; n++) {
      const node = nodes[n];
      if (node.k === 'text' || node.k === 'image') node.marks = { ...marks, ...node.marks };
    }
  };

  function processEmphasis(bottom: Delim | null): void {
    const openersBottom = new Map<string, Delim | null>();
    let closer: Delim | null = delims;
    while (closer !== null && closer.prev !== bottom) closer = closer.prev;
    while (closer !== null) {
      if (!closer.canClose) {
        closer = closer.next;
        continue;
      }
      const star = closer.ch === '*' || closer.ch === '_';
      const key = star ? `${closer.ch}${closer.canOpen ? 1 : 0}${closer.orig % 3}` : `${closer.ch}${closer.orig}`;
      const floor = openersBottom.has(key) ? openersBottom.get(key)! : bottom;
      let opener: Delim | null = closer.prev;
      let found = false;
      while (opener !== null && opener !== bottom && opener !== floor) {
        if (opener.ch === closer.ch && opener.canOpen) {
          let ok: boolean;
          if (star) {
            const odd = (closer.canOpen || opener.canClose) && closer.orig % 3 !== 0 && (opener.orig + closer.orig) % 3 === 0;
            ok = !odd;
          } else if (closer.ch === '~' && !jinshu) ok = opener.len === closer.len;
          else if (closer.ch === '+' || closer.ch === '=') ok = opener.len === 2 && closer.len === 2;
          else if (closer.ch === '^') ok = opener.len === 1 && closer.len === 1;
          // `~` is sub and `~~` strike; only a run of three (both at once) may pair with a shorter one.
          else ok = opener.len === closer.len || opener.len === 3 || closer.len === 3;
          if (ok) {
            found = true;
            break;
          }
        }
        opener = opener.prev;
      }
      const oldCloser: Delim = closer;
      if (found && opener) {
        const use =
          closer.ch === '~'
            ? jinshu ? (opener.len === closer.len ? Math.min(closer.len, 2) : Math.min(opener.len, closer.len)) : closer.len
            : closer.ch === '+' || closer.ch === '=' ? 2 : closer.len >= 2 && opener.len >= 2 ? 2 : 1;
        applyMarks(opener.node, closer.node, markFor(opener, use));
        opener.len -= use;
        closer.len -= use;
        const on = nodes[opener.node] as { text: string };
        const cn = nodes[closer.node] as { text: string };
        on.text = opener.ch === '=' ? '' : on.text.slice(0, on.text.length - use);
        cn.text = cn.text.slice(use);
        opener.next = closer;
        closer.prev = opener;
        if (opener.len === 0) removeDelim(opener);
        if (closer.len === 0) {
          const next: Delim | null = closer.next;
          removeDelim(closer);
          closer = next;
        }
      } else {
        closer = closer.next;
        openersBottom.set(key, oldCloser.prev);
        if (!oldCloser.canOpen) removeDelim(oldCloser);
      }
    }
    while (delims !== null && delims !== bottom) removeDelim(delims);
  }

  const plainOf = (from: number): string => {
    let out = '';
    for (let n = from + 1; n < nodes.length; n++) {
      const node = nodes[n];
      if (node.dead) continue;
      if (node.k === 'text') out += node.text;
      else if (node.k === 'math') out += node.math;
      else if (node.k === 'atom') out += node.atom.fallback;
      else if (node.k === 'image') out += node.alt;
      else out += ' ';
    }
    return out;
  };

  /** Tries to close the innermost bracket at `pos` (a `]`); returns the new position or -1. */
  function closeBracket(pos: number): number {
    const opener = brackets;
    if (!opener) return -1;
    brackets = opener.prev;
    if (!opener.active) return -1;
    let dest: string | undefined;
    let title: string | undefined;
    let end = pos + 1;
    let matched = false;
    if (src[end] === '(') {
      let i = skipSpaceNl(src, end + 1);
      if (src[i] === ')') {
        dest = '';
        end = i + 1;
        matched = true;
      } else {
        const d = scanDestination(src, i, strict);
        if (d) {
          i = d.end;
          const beforeTitle = i;
          i = skipSpaceNl(src, i);
          const t = i > beforeTitle ? scanTitle(src, i, strict) : null;
          if (t) i = skipSpaceNl(src, t.end);
          if (src[i] === ')') {
            dest = d.dest;
            title = t?.title;
            end = i + 1;
            matched = true;
          }
        }
      }
    }
    if (!matched && options.refs) {
      const raw = src.slice(opener.pos + (opener.image ? 2 : 1), pos);
      const full = scanLabel(src, pos + 1);
      let label: string | undefined;
      if (full) {
        label = full.label;
        end = full.end;
      } else {
        label = raw;
        end = src.startsWith('[]', pos + 1) ? pos + 3 : pos + 1;
      }
      const ref = label.length <= 999 ? options.refs.get(normalizeLabel(label)) : undefined;
      if (ref) {
        dest = ref.href;
        title = ref.title;
        matched = true;
      }
    }
    if (!matched || dest === undefined) return -1;
    const sourceText = src.slice(opener.pos, end);
    const href = dest.trim();
    const problem = opener.image ? validateSrc(href) : validateLink(href);
    if (problem !== null) {
      options.warn?.(`${opener.image ? 'Image' : 'Link'} "${sourceText}": "${dest}" is not a ${opener.image ? 'source' : 'link'} Jinshu can keep; kept as text.`, 'linkAsText');
      return -1;
    }
    if (title !== undefined) options.warn?.(`${opener.image ? 'Image' : 'Link'} "${sourceText}": its title "${title}" is dropped.`, 'linkTitle');
    flush();
    processEmphasis(opener.delimBottom);
    if (opener.image) {
      const alt = plainOf(opener.node);
      for (let n = opener.node + 1; n < nodes.length; n++) nodes[n].dead = true;
      nodes[opener.node] = { k: 'image', src: href, alt, source: sourceText, marks: {} };
    } else {
      applyMarks(opener.node, nodes.length, { link: href });
      (nodes[opener.node] as { text: string }).text = '';
      for (let b = brackets; b; b = b.prev) if (!b.image) b.active = false;
    }
    return end;
  }

  let pos = 0;
  while (pos < src.length) {
    const c = src[pos];

    if (c === '\n') {
      if (!strict) {
        flush();
        nodes.push({ k: 'break' });
        pos += 1;
        continue;
      }
      const trailing = /[ ]*$/.exec(text)![0].length;
      const hard = trailing >= 2;
      text = text.replace(/[ \t]+$/, '');
      const before = lastChar(text);
      pos += 1;
      while (src[pos] === ' ' || src[pos] === '\t') pos++;
      if (hard) {
        flush();
        nodes.push({ k: 'break' });
      } else if (pos < src.length) {
        const after = charAt(src, pos);
        if (!(before && after && CJK_RE.test(before) && CJK_RE.test(after))) text += ' ';
      }
      continue;
    }

    if (c === '\\') {
      const n = src[pos + 1];
      if (strict && n === '\n') {
        flush();
        nodes.push({ k: 'break' });
        pos += 2;
        while (src[pos] === ' ' || src[pos] === '\t') pos++;
        continue;
      }
      if (escapable(n)) {
        text += n;
        pos += 2;
        continue;
      }
      text += c;
      pos += 1;
      continue;
    }

    if (c === '`') {
      let n = 0;
      while (src[pos + n] === '`') n++;
      let close = -1;
      for (let i = pos + n; i < src.length; ) {
        if (src[i] !== '`') {
          i++;
          continue;
        }
        let m = 0;
        while (src[i + m] === '`') m++;
        if (m === n) {
          close = i;
          break;
        }
        i += m;
      }
      if (close < 0) {
        text += '`'.repeat(n);
        pos += n;
        continue;
      }
      let code = src.slice(pos + n, close).replace(/\n/g, ' ');
      if (code.length >= 2 && code.startsWith(' ') && code.endsWith(' ') && /[^ ]/.test(code)) code = code.slice(1, -1);
      pushText(code, { code: true });
      pos = close + n;
      continue;
    }

    if (c === '$') {
      let n = 0;
      while (src[pos + n] === '$') n++;
      const m = scanMath(src, pos, n, dialect);
      if (m) {
        flush();
        nodes.push({ k: 'math', math: m.math });
        pos = m.end;
      } else {
        text += '$'.repeat(n);
        pos += n;
      }
      continue;
    }

    if (DELIM_CHARS.has(c) && (c === '*' || c === '_' || c === '~' || jinshu)) {
      let n = 0;
      while (src[pos + n] === c) n++;
      let suffix = '';
      let tone: HighlightTone | undefined;
      if (c === '=' && n === 2) {
        const t = /^\{([a-z]+)\}/.exec(src.slice(pos + 2, pos + 12));
        if (t && TONES.has(t[1])) {
          suffix = t[0];
          tone = t[1] as HighlightTone;
        }
      }
      // Adjacent jinshu marks (`==a===={warning}b==`) meet as one run; split it into pairs (singles for `^`).
      const piece = c === '^' ? 1 : 2;
      if (jinshu && (c === '+' || c === '=' || c === '^') && n > piece && n % piece === 0) n = piece;
      if (c === '=' && n === 2 && !suffix) {
        const t = /^\{([a-z]+)\}/.exec(src.slice(pos + 2, pos + 12));
        if (t && TONES.has(t[1])) {
          suffix = t[0];
          tone = t[1] as HighlightTone;
        }
      }
      const prev = pos === 0 ? undefined : charBefore(src, pos);
      const next = charAt(src, pos + n + suffix.length);
      const run = c.repeat(n) + suffix;
      if (!delimiterRunOk(c, n, dialect)) {
        text += run;
        pos += run.length;
        continue;
      }
      // `=={tone}` only opens; what follows the `}` is content, a space included.
      const { open, close } = suffix ? { open: true, close: false } : canOpenClose(c, prev, next, strict, options.symbolsAsText ? punctP : punctPS);
      const node = pushText(run);
      if (open || close) {
        const d: Delim = { node, ch: c, len: n, orig: n, canOpen: open, canClose: close, tone, prev: delims, next: null };
        if (delims) delims.next = d;
        delims = d;
      }
      pos += run.length;
      continue;
    }

    if (c === '[' || (c === '!' && src[pos + 1] === '[')) {
      const image = c === '!';
      const node = pushText(image ? '![' : '[');
      brackets = { node, image, active: true, delimBottom: delims, pos, prev: brackets };
      pos += image ? 2 : 1;
      continue;
    }

    if (c === ']') {
      const end = closeBracket(pos);
      if (end < 0) {
        text += ']';
        pos += 1;
      } else pos = end;
      continue;
    }

    if (c === '<') {
      const rest = src.slice(pos);
      const uri = AUTOLINK_URI_RE.exec(rest);
      const email = uri ? null : AUTOLINK_EMAIL_RE.exec(rest);
      const m = uri ?? email;
      if (m) {
        const href = email ? `mailto:${m[1]}` : m[1];
        if (validateLink(href) === null) {
          pushText(m[1], { link: href });
        } else {
          options.warn?.(`Link "${m[0]}" is not a link Jinshu can keep; kept as text.`, 'linkAsText');
          text += m[0];
        }
        pos += m[0].length;
        continue;
      }
      const html = HTML_TAG_RE.exec(rest);
      if (html) {
        options.warn?.(`Raw HTML "${html[0].length > 40 ? `${html[0].slice(0, 40)}…` : html[0]}" is kept as text.`, 'rawHtml');
        text += html[0];
        pos += html[0].length;
        continue;
      }
      text += c;
      pos += 1;
      continue;
    }

    if (c === '&' && strict) {
      const e = readEntity(src.slice(pos));
      if (e) {
        const end = pos + e.length;
        text += e.text;
        pos = end;
        continue;
      }
      text += c;
      pos += 1;
      continue;
    }

    if (c === '⟦' && jinshu) {
      const end = findUnescaped(src, '⟧', pos + 1);
      const hash = end > 0 ? src.lastIndexOf('#', end) : -1;
      const atom = hash > pos ? atoms.get(src.slice(hash + 1, end)) : undefined;
      if (atom) {
        flush();
        nodes.push({ k: 'atom', atom });
        pos = end + 1;
        continue;
      }
    }

    if (!jinshu && EMAIL_CHAR_RE.test(c)) {
      const auto = scanExtendedAutolink(src, pos);
      if (auto && validateLink(auto.href) === null) {
        pushText(auto.text, { link: auto.href });
        pos += auto.text.length;
        continue;
      }
    }

    text += c;
    pos += 1;
  }
  flush();
  processEmphasis(null);

  const out: (Inline | InlineImage)[] = [];
  for (const node of nodes) {
    if (node.dead) continue;
    switch (node.k) {
      case 'text':
        if (node.text) out.push({ text: node.text, ...node.marks } as InlineRun);
        break;
      case 'break':
        out.push({ break: true });
        break;
      case 'math':
        out.push({ math: node.math });
        break;
      case 'atom':
        out.push(node.atom);
        break;
      case 'image':
        out.push({ image: true, src: node.src, alt: node.alt, source: node.source, ...(node.marks.link ? { link: node.marks.link } : {}) });
        break;
    }
  }
  return out;
}

/** Inline math at `pos` (a run of `n` dollars), or null. */
function scanMath(src: string, pos: number, n: number, dialect: InlineDialect): { math: string; end: number } | null {
  if (dialect === 'jinshu') {
    if (n !== 1) return null;
    const end = src.indexOf('$', pos + 1);
    if (end <= pos + 1 || src.slice(pos + 1, end).includes('\n')) return null;
    return { math: src.slice(pos + 1, end), end: end + 1 };
  }
  if (n > 2) return null;
  const startCh = src[pos + n];
  if (startCh === undefined || (n === 1 && isWs(startCh))) return null;
  for (let i = pos + n; i < src.length; i++) {
    const c = src[i];
    if (c === '\n' || (c === '\\' && src[i + 1] === '\n')) return null;
    if (c === '\\') {
      i += 1;
      continue;
    }
    if (c !== '$') continue;
    let m = 0;
    while (src[i + m] === '$') m++;
    // `$a$$b$` is two maths: a single-dollar math closes at the first dollar of a longer run.
    if ((m === n || (n === 1 && m === 2)) && !(n === 1 && isWs(src[i - 1])) && i > pos + n) return { math: src.slice(pos + n, i), end: i + n };
    i += m - 1;
  }
  return null;
}

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

/** Text an image stands for when it cannot be one: its own source. */
function imagesAsText(items: readonly (Inline | InlineImage)[]): Inline[] {
  return items.map(item => ('image' in item ? ({ text: item.source, ...(item.link ? { link: item.link } : {}) } as Inline) : item));
}

/** @internal Parses inline text and canonicalizes it; images become their source text. */
export function parseInlines(source: string, options: InlineParseOptions): Inline[] {
  const items = parseInlineNodes(source, options);
  if (options.warn && items.some(i => 'image' in i)) options.warn('An image inside text is kept as its Markdown source.', 'inlineImage');
  return normalizeInlines(imagesAsText(items), { inHeading: options.inHeading });
}

// ============================================================================
// Writing
// ============================================================================

/** @internal Where written inline text goes; each place has its own escaping. */
export type InlineContext = 'shorthand' | 'block' | 'heading' | 'cell' | 'label';

export interface InlineWriteOptions {
  readonly dialect: InlineDialect;
  readonly context: InlineContext;
  /** Called once per thing the dialect cannot write (a mark, an atom, a break in a cell). */
  readonly onLoss?: (what: string) => void;
}

interface Variant {
  readonly bare: boolean;
  readonly italic: '*' | '_';
  readonly bold: '*' | '_';
  /** Open `~sub~` outside `~~strike~~`, for when their tildes would run together. */
  readonly subOuter: boolean;
  /** Reopen emphasis with the other character after a `*` or `_` delimiter, so the runs stay apart. */
  readonly alternate: boolean;
}

const EMPHASIS = ['*', '_'] as const;

/** Equivalent spellings, tried in order; the first that reads back exactly wins. */
const VARIANTS: readonly Variant[] = [false, true].flatMap(subOuter =>
  [false, true].flatMap(alternate =>
    EMPHASIS.flatMap(bold => [true, false].flatMap(bare => EMPHASIS.map(italic => ({ bare, italic, bold, subOuter, alternate })))),
  ),
);

type Cell =
  | { kind: 'text'; ch: string; out: string; encoded?: boolean; inLink: boolean; seg: number }
  | { kind: 'syn'; out: string; delim?: { ch: string; role: 'open' | 'close' }; role?: 'lopen' | 'lclose' | 'math' | 'code' | 'atom' | 'bare'; inLink: boolean; seg: number }
  | { kind: 'brk'; out: string; seg: number };

const MARK_ORDER = ['link', 'hl', 'b', 'i', 'u', 's', 'sup', 'sub'] as const;

function markKeys(run: InlineRun, dialect: InlineDialect): string[] {
  const keys: string[] = [];
  if (run.link) keys.push(`link:${run.link}`);
  if (dialect === 'jinshu' && run.highlight) keys.push(`hl:${run.highlight}`);
  if (run.bold) keys.push('b');
  if (run.italic) keys.push('i');
  if (dialect === 'jinshu' && run.underline) keys.push('u');
  if (run.strike) keys.push('s');
  if (dialect === 'jinshu' && run.script) keys.push(run.script === 'super' ? 'sup' : 'sub');
  return keys;
}

const orderOf = (key: string) => MARK_ORDER.indexOf(key.split(':')[0] as (typeof MARK_ORDER)[number]);

/** Writes a link destination so it reads back exactly. */
export function writeDestination(dest: string): string {
  if (dest === '' || /[\s<>\x00-\x1f\x7f]/.test(dest)) return `<${dest.replace(/[<>\\]/g, '\\$&')}>`;
  let depth = 0;
  let balanced = true;
  for (const c of dest) {
    if (c === '(') depth++;
    else if (c === ')' && --depth < 0) balanced = false;
  }
  if (depth !== 0) balanced = false;
  let out = '';
  for (let i = 0; i < dest.length; i++) {
    const c = dest[i];
    if (c === '\\' || (!balanced && (c === '(' || c === ')'))) out += `\\${c}`;
    else if (c === '&' && ENTITY_LIKE_RE.test(dest.slice(i))) out += '\\&';
    else out += c;
  }
  return out;
}

/** A code span for `code`: the shortest fence it does not contain, padded when needed. */
function codeSpan(code: string): string {
  const runs = new Set((code.match(/`+/g) ?? []).map(r => r.length));
  let n = 1;
  while (runs.has(n)) n++;
  const fence = '`'.repeat(n);
  const pad = code.startsWith('`') || code.endsWith('`') || (code.length >= 2 && code.startsWith(' ') && code.endsWith(' ') && /[^ ]/.test(code));
  return pad ? `${fence} ${code} ${fence}` : `${fence}${code}${fence}`;
}

/** What the dialect can hold: marks it cannot write are dropped, atoms become their fallback text. */
function fitInlines(inlines: readonly Inline[], options: InlineWriteOptions): Inline[] {
  const markdown = options.dialect === 'markdown';
  const out: Inline[] = [];
  for (const item of inlines) {
    const kind = inlineKind(item);
    if (kind === 'text') {
      const run = item as InlineRun;
      if (markdown && (run.underline || run.highlight || run.script)) {
        if (run.underline) options.onLoss?.('underline');
        if (run.highlight) options.onLoss?.('highlight');
        if (run.script) options.onLoss?.(run.script === 'super' ? 'superscript' : 'subscript');
        const { underline: _u, highlight: _h, script: _s, ...rest } = run;
        out.push(rest as Inline);
      } else out.push(run);
    } else if (kind === 'atom') {
      if (markdown) {
        options.onLoss?.('inline atom');
        out.push({ text: (item as InlineAtom).fallback });
      } else out.push(item);
    } else if (kind === 'break') {
      if (options.context === 'heading' || options.context === 'cell' || options.context === 'label') {
        if (options.context === 'cell') options.onLoss?.('line break in a table cell');
        out.push({ text: ' ' });
      } else out.push(item);
    } else if (kind === 'math') {
      const math = (item as InlineMath).math;
      if (!mathWritable(math, options.dialect)) {
        options.onLoss?.('inline math');
        out.push({ text: math, code: true });
      } else out.push(item);
    }
  }
  const fitted = normalizeInlines(out);
  return options.context === 'block' || options.context === 'heading' || options.context === 'cell' ? trimLineEdges(fitted) : fitted;
}

const isPlainText = (item: Inline | undefined): item is InlineRun => item !== undefined && inlineKind(item) === 'text' && !(item as InlineRun).code;

/** Drops what a Markdown line cannot say: spaces at the edges of each line, and breaks at the edges of the text. */
export function trimLineEdges(inlines: readonly Inline[]): Inline[] {
  const out = inlines.map(i => ({ ...i })) as Inline[];
  const trim = (at: number, step: 1 | -1) => {
    for (let i = at; i >= 0 && i < out.length && isPlainText(out[i]); i += step) {
      const run = out[i] as { text: string };
      run.text = step === 1 ? run.text.replace(/^[ \t]+/, '') : run.text.replace(/[ \t]+$/, '');
      if (run.text !== '') break;
    }
  };
  trim(0, 1);
  trim(out.length - 1, -1);
  out.forEach((item, i) => {
    if (inlineKind(item) !== 'break') return;
    trim(i - 1, -1);
    trim(i + 1, 1);
  });
  const kept = normalizeInlines(out.filter(i => !(isPlainText(i) && i.text === '')));
  while (kept.length && inlineKind(kept[0]) === 'break') kept.shift();
  while (kept.length && inlineKind(kept[kept.length - 1]) === 'break') kept.pop();
  return kept;
}

function buildCells(inlines: readonly Inline[], options: InlineWriteOptions, variant: Variant): Cell[] {
  const { dialect } = options;
  const shorthand = options.context === 'shorthand';
  const cells: Cell[] = [];
  let seg = 0;
  let inLink = options.context === 'label';
  const stack: string[] = [];
  const syn = (out: string, extra: Partial<Extract<Cell, { kind: 'syn' }>> = {}) =>
    cells.push({ kind: 'syn', out, inLink, seg, ...extra });

  const delimOf = (key: string): { open: string; close: string; ch: string } | null => {
    if (key === 's') return { open: '~~', close: '~~', ch: '~' };
    if (key === 'u') return { open: '++', close: '++', ch: '+' };
    if (key === 'sup') return { open: '^', close: '^', ch: '^' };
    if (key === 'sub') return { open: '~', close: '~', ch: '~' };
    if (key.startsWith('hl:')) {
      const tone = key.slice(3);
      return { open: tone === 'accent' ? '==' : `=={${tone}}`, close: '==', ch: '=' };
    }
    return null;
  };
  // Emphasis opened right after a `*` or `_` delimiter would merge into its run; the other character keeps them apart.
  const emphasisChars: string[] = [];
  const open = (key: string) => {
    if (key === 'i' || key === 'b') {
      const prev = cells[cells.length - 1];
      const base = key === 'i' ? variant.italic : variant.bold;
      const other = base === '*' ? '_' : '*';
      const ch = variant.alternate && prev?.kind === 'syn' && prev.out.endsWith(base) ? other : base;
      emphasisChars.push(ch);
      syn(key === 'b' ? ch + ch : ch, { delim: { ch, role: 'open' } });
      stack.push(key);
      return;
    }
    if (key.startsWith('link:')) {
      syn('[', { role: 'lopen' });
      inLink = true;
    } else {
      const d = delimOf(key)!;
      syn(d.open, { delim: { ch: d.ch, role: 'open' } });
    }
    stack.push(key);
  };
  const close = () => {
    const key = stack.pop()!;
    if (key.startsWith('link:')) {
      inLink = options.context === 'label';
      syn(`](${writeDestination(key.slice(5))})`, { role: 'lclose' });
    } else if (key === 'i' || key === 'b') {
      const ch = emphasisChars.pop()!;
      syn(key === 'b' ? ch + ch : ch, { delim: { ch, role: 'close' } });
    } else {
      const d = delimOf(key)!;
      syn(d.close, { delim: { ch: d.ch, role: 'close' } });
    }
  };

  const items = inlines;
  const keysOf = items.map(item => (inlineKind(item) === 'text' ? markKeys(item as InlineRun, dialect) : []));
  for (let idx = 0; idx < items.length; idx++) {
    const item = items[idx];
    const keys = keysOf[idx];
    let keep = 0;
    while (keep < stack.length && keys.includes(stack[keep])) keep++;
    while (stack.length > keep) close();
    const span = (key: string) => {
      let n = 0;
      while (idx + n < items.length && keysOf[idx + n].includes(key)) n++;
      return n;
    };
    const rank = (k: string) => (variant.subOuter && k === 'sub' ? orderOf('s') - 0.5 : orderOf(k));
    const missing = keys.filter(k => !stack.includes(k)).sort((a, b) => span(b) - span(a) || rank(a) - rank(b));
    const run = item as InlineRun;
    // A plain URL written bare reads back as the same link.
    if (
      variant.bare &&
      dialect === 'markdown' &&
      !shorthand &&
      options.context === 'block' &&
      missing.length === 1 &&
      keys.length === 1 &&
      !run.code &&
      run.link === run.text &&
      /^https?:\/\//i.test(run.text) &&
      span(keys[0]) === 1
    ) {
      syn(run.text, { role: 'bare' });
      continue;
    }
    for (const key of missing) open(key);
    switch (inlineKind(item)) {
      case 'break':
        cells.push({ kind: 'brk', out: shorthand ? '\n' : '\\\n', seg });
        seg += 1;
        break;
      case 'math':
        syn(`$${(item as InlineMath).math}$`, { role: 'math' });
        break;
      case 'atom': {
        const atom = item as InlineAtom;
        let fallback = '';
        for (const ch of atom.fallback) fallback += ch === '\\' || ch === '⟧' || ch === '#' ? `\\${ch}` : ch;
        syn(`⟦${fallback}#${atom.id}⟧`, { role: 'atom' });
        break;
      }
      case 'text':
        if (run.code) syn(codeSpan(run.text), { role: 'code' });
        else for (const ch of run.text) cells.push({ kind: 'text', ch, out: ch, inLink, seg });
        break;
    }
  }
  while (stack.length) close();
  return cells;
}

const first = (cell: Cell | undefined): string | undefined => (cell ? charAt(cell.out, 0) : undefined);
const last = (cell: Cell | undefined): string | undefined => (cell ? lastChar(cell.out) : undefined);

function entityOf(ch: string): string {
  return `&#${ch.codePointAt(0)};`;
}

/** Renders cells with the escaping each character needs in its context. */
function renderCells(cells: Cell[], options: InlineWriteOptions): string {
  const strict = options.context !== 'shorthand';
  const jinshu = options.dialect === 'jinshu';
  const prevCell = (i: number) => (i > 0 && cells[i - 1].kind !== 'brk' ? cells[i - 1] : undefined);
  const nextCell = (i: number) => (i + 1 < cells.length && cells[i + 1].kind !== 'brk' ? cells[i + 1] : undefined);
  const encode = (cell: Cell | undefined): boolean => {
    if (!cell || cell.kind !== 'text' || cell.encoded) return false;
    if (strict) cell.out = entityOf(cell.ch);
    else if (cell.ch === ' ' || cell.ch === '\t') cell.out = `\\${cell.ch}`;
    else return false;
    cell.encoded = true;
    return true;
  };

  // Delimiters must flank as openers and closers; where they cannot, encode the character in the way.
  const punctFns: readonly PunctFn[] = strict ? [punctPS, punctP] : [punctPS];
  for (let pass = 0; pass < 4; pass++) {
    let changed = false;
    for (let i = 0; i < cells.length; i++) {
      const cell = cells[i];
      if (cell.kind !== 'syn' || !cell.delim || cell.out.startsWith('=={')) continue;
      const { ch, role } = cell.delim;
      for (const punct of punctFns) {
        const prev = last(prevCell(i));
        const next = first(nextCell(i));
        const oc = canOpenClose(ch, prev, next, strict, punct);
        if (role === 'open' && !oc.open) {
          const n = nextCell(i);
          if (n?.kind === 'text' && isWs(n.ch) && encode(n)) changed = true;
          else if (encode(prevCell(i))) changed = true;
        } else if (role === 'close' && !oc.close) {
          const p = prevCell(i);
          if (p?.kind === 'text' && isWs(p.ch) && encode(p)) changed = true;
          else if (encode(nextCell(i))) changed = true;
        }
      }
    }
    if (!changed) break;
  }

  // Characters that would read as syntax get a backslash.
  const laterClose: boolean[] = new Array(cells.length + 1).fill(false);
  for (let i = cells.length - 1; i >= 0; i--) laterClose[i] = laterClose[i + 1] || cells[i].out.includes(']');
  const segDollars = new Map<number, number>();
  for (const cell of cells) {
    if (cell.kind === 'text' && cell.ch === '$') segDollars.set(cell.seg, (segDollars.get(cell.seg) ?? 0) + 1);
    if (cell.kind === 'syn') segDollars.set(cell.seg, (segDollars.get(cell.seg) ?? 0) + (cell.role === 'math' ? 2 : cell.out.split('$').length - 1));
  }
  const autolinkEscapes = new Set<number>();
  // Inside a link's text nothing autolinks.
  if (!jinshu && options.context !== 'label') {
    for (let i = 0; i < cells.length; ) {
      if (cells[i].kind !== 'text') {
        i++;
        continue;
      }
      let j = i;
      let s = '';
      const at: number[] = [];
      while (j < cells.length && cells[j].kind === 'text') {
        const c = cells[j] as Extract<Cell, { kind: 'text' }>;
        for (let u = 0; u < c.ch.length; u++) at.push(j);
        s += c.ch;
        j++;
      }
      for (const m of s.matchAll(/https?(:)\/\//gi)) autolinkEscapes.add(at[m.index! + m[0].indexOf(':')]);
      for (const m of s.matchAll(/www(\.)/gi)) autolinkEscapes.add(at[m.index! + 3]);
      for (const m of s.matchAll(/[A-Za-z0-9._+-](@)[A-Za-z0-9_-]/g)) autolinkEscapes.add(at[m.index! + 1]);
      i = j;
    }
  }
  const escapableNext = (c: string | undefined) =>
    c === undefined || isAsciiPunct(c) || (!strict && (c === ' ' || c === '\t' || c === '\n' || (jinshu && (c === '⟦' || c === '⟧'))));

  for (let i = 0; i < cells.length; i++) {
    const cell = cells[i];
    if (cell.kind !== 'text' || cell.encoded) continue;
    const c = cell.ch;
    let esc = false;
    if (c === '\\') esc = escapableNext(first(nextCell(i)));
    else if (DELIM_CHARS.has(c) && (c === '*' || c === '_' || c === '~' || jinshu)) {
      let a = i;
      while (a > 0 && cells[a - 1].kind === 'text' && (cells[a - 1] as { ch: string }).ch === c && !(cells[a - 1] as { encoded?: boolean }).encoded) a--;
      let b = i;
      while (b + 1 < cells.length && cells[b + 1].kind === 'text' && (cells[b + 1] as { ch: string }).ch === c && !(cells[b + 1] as { encoded?: boolean }).encoded) b++;
      const len = b - a + 1;
      const before = prevCell(a);
      const after = nextCell(b);
      const touchesSyntax = (before?.kind === 'syn' && before.out.endsWith(c)) || (after?.kind === 'syn' && after.out.startsWith(c));
      let active = touchesSyntax;
      if (!active && (c === '*' || c === '_' || c === '~' || len >= (c === '^' ? 1 : 2))) {
        for (const punct of punctFns) {
          const oc = canOpenClose(c, last(before), first(after), strict, punct);
          if (oc.open || oc.close) active = true;
        }
      }
      esc = active;
    } else if (c === '`') esc = true;
    else if (c === '[') esc = options.context === 'label' || laterClose[i + 1];
    else if (c === ']') esc = cell.inLink;
    else if (c === '!') esc = nextCell(i)?.kind === 'syn' && (nextCell(i) as { role?: string }).role === 'lopen';
    else if (c === '<') esc = /[A-Za-z/!?]/.test(first(nextCell(i)) ?? '');
    else if (c === '&') {
      if (strict) {
        let s = '&';
        for (let j = i + 1; j < cells.length && cells[j].kind === 'text' && s.length < 40; j++) s += (cells[j] as { ch: string }).ch;
        esc = ENTITY_LIKE_RE.test(s);
      }
    } else if (c === '$') esc = (segDollars.get(cell.seg) ?? 0) >= 2;
    else if (c === '⟦') esc = jinshu;
    else if (c === '{') {
      const p = prevCell(i);
      esc = jinshu && p?.kind === 'syn' && p.delim?.ch === '=' && p.delim.role === 'open' && p.out === '==';
    } else if (autolinkEscapes.has(i)) esc = true;
    if (esc) cell.out = `\\${c}`;
  }

  // A line that would start a block gets its first character escaped; an empty one is just its break.
  const segCount = cells.filter(c => c.kind === 'brk').length + 1;
  const lines: string[] = Array.from({ length: segCount }, () => '');
  const breaks: string[] = [];
  for (const cell of cells) {
    if (cell.kind === 'brk') breaks.push(cell.out);
    else lines[cell.seg] += cell.out;
  }
  let out = '';
  lines.forEach((line, n) => {
    let l = line;
    if (options.context === 'block') {
      if (l !== '') l = escapeLineStart(l, n === 0);
    }
    out += l + (n < breaks.length ? breaks[n] : '');
  });
  if (options.context === 'heading') out = escapeClosingHashes(out);
  if (options.context === 'cell') out = out.replace(/\|/g, '\\|');
  return out;
}

const THEMATIC_RE = /^(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$/;
const DELIM_ROW_RE = /^\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;

/** Escapes what would make a line of a paragraph start a block. */
function escapeLineStart(line: string, firstLine: boolean): string {
  if (/^#{1,6}(?:[ \t]|$)/.test(line)) return escapeLead(line);
  if (/^[>+]/.test(line) && (line[0] === '>' || /^\+(?:[ \t]|$)/.test(line))) return escapeLead(line);
  if (/^[-*](?:[ \t]|$)/.test(line) || THEMATIC_RE.test(line)) return escapeLead(line);
  const ordered = /^(\d{1,9})([.)])(?:[ \t]|$)/.exec(line);
  if (ordered) return `${ordered[1]}\\${line.slice(ordered[1].length)}`;
  if (/^(?:```|~~~)/.test(line)) return escapeLead(line);
  // Checked on a first line too: a callout's alert line comes before it.
  if (/^(?:=+|-+)[ \t]*$/.test(line) || (!firstLine && line.includes('|') && DELIM_ROW_RE.test(line))) return escapeLead(line);
  return line;
}

/** Escapes a line's first character; a leading `*`, `_` or `~` run is escaped whole so no shorter run is left to pair. */
function escapeLead(line: string): string {
  const run = /^([*_~])\1*/.exec(line);
  if (!run) return `\\${line}`;
  return run[0].replace(/./g, '\\$&') + line.slice(run[0].length);
}

/** A heading's trailing `#`s would be read as its closing sequence. */
function escapeClosingHashes(content: string): string {
  const m = /(^|[ \t])(#+)$/.exec(content);
  if (!m) return content;
  const at = content.length - m[2].length;
  return `${content.slice(0, at)}\\${content.slice(at)}`;
}

/** @internal Reads back what `renderCells` wrote, the way its context reads it. */
export function readInlineContext(text: string, options: InlineParseOptions, context: InlineContext): Inline[] {
  if (context === 'shorthand') return parseInlines(text, options);
  if (context === 'label') return normalizeInlines(parseInlines(text, options).map(i => ('link' in i ? { ...i, link: undefined } : i)));
  if (context === 'block') return parseInlines(paragraphSource(text.split('\n')), options);
  if (context === 'heading') return parseInlines(headingSource(text), options);
  // A pipe the row splitter would take as a cell edge cannot be written in this cell.
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '\\') i += 1;
    else if (text[i] === '|') return [{ text: '\u0000' }];
  }
  return parseInlines(cellSource(text), options);
}

/** A paragraph's inline source from its lines, as CommonMark strips them. */
export function paragraphSource(lines: readonly string[]): string {
  return lines
    .map(l => l.replace(/^[ \t]+/, ''))
    .join('\n')
    .replace(/[ \t]+$/, '');
}

/** An ATX heading's content: trimmed, without the closing `#` sequence. */
export function headingSource(content: string): string {
  let s = content.replace(/^[ \t]+/, '').replace(/[ \t]+$/, '');
  if (/^#+$/.test(s)) return '';
  s = s.replace(/[ \t]+#+[ \t]*$/, '');
  return s.replace(/[ \t]+$/, '');
}

/** A table cell's inline source: trimmed, with `\|` read as `|`. */
export function cellSource(raw: string): string {
  return raw.replace(/^[ \t]+|[ \t]+$/g, '').replace(/\\\|/g, '|');
}

/**
 * @internal Writes inlines in a dialect and context. Tries a few equivalent spellings and keeps the
 * first that reads back exactly; `exact` is false when none did.
 */
export function writeInlines(inlines: readonly Inline[], options: InlineWriteOptions): { text: string; exact: boolean } {
  const fitted = fitInlines(normalizeInlines(inlines as Inline[]), options);
  const strict = options.context !== 'shorthand';
  const parseOptions: InlineParseOptions = {
    dialect: options.dialect,
    strict,
    existingAtoms: fitted.filter(i => inlineKind(i) === 'atom') as InlineAtom[],
    inHeading: options.context === 'heading',
  };
  const want = JSON.stringify(normalizeInlines(fitted, { inHeading: options.context === 'heading' }));
  const variants = options.dialect === 'markdown' && options.context === 'block' ? VARIANTS : VARIANTS.filter(v => !v.bare);
  let fallback: string | undefined;
  for (const variant of variants) {
    const text = renderCells(buildCells(fitted, options, variant), options);
    const reads = (o: InlineParseOptions) => JSON.stringify(readInlineContext(text, o, options.context)) === want;
    if (reads(parseOptions) && (!strict || reads({ ...parseOptions, symbolsAsText: true }))) return { text, exact: true };
    fallback ??= variant.bare ? undefined : text;
  }
  return { text: fallback!, exact: false };
}

// ============================================================================
// Shorthand: the editor's text form
// ============================================================================

/** All inline atoms in `inlines`, for `shorthandToInlines(..., { existingAtoms })`. */
export function atomsIn(inlines: readonly Inline[] | undefined): InlineAtom[] {
  return Array.isArray(inlines) ? (inlines.filter(i => inlineKind(i) === 'atom') as InlineAtom[]) : [];
}

/** Inlines as editable shorthand; each line break is a newline. `markdown` writes only what Markdown holds. */
export function inlinesToShorthand(inlines: readonly Inline[], options?: { readonly dialect?: InlineDialect }): string {
  return writeInlines(inlines, { dialect: options?.dialect ?? 'jinshu', context: 'shorthand' }).text;
}

/**
 * Parses shorthand back into canonical inlines. Never throws; an unmatched delimiter or a link whose
 * target Jinshu cannot keep stays literal text. Atom tokens resolve against `existingAtoms` by id.
 */
export function shorthandToInlines(
  source: string,
  options?: { readonly inHeading?: boolean; readonly existingAtoms?: readonly InlineAtom[]; readonly dialect?: InlineDialect },
): Inline[] {
  return parseInlines(source, {
    dialect: options?.dialect ?? 'jinshu',
    strict: false,
    inHeading: options?.inHeading,
    existingAtoms: options?.existingAtoms,
  });
}
