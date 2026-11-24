// A small syntax colouring for CodeBlock: a few kinds of token, found in one pass that consumes
// everything it reads, so any input, however long or malformed, costs time in proportion to it.

export type TokenKind = 'keyword' | 'string' | 'number' | 'comment' | 'type';

/** A stretch of code and its kind; plain text has none. */
export interface CodeRun {
  text: string;
  kind?: TokenKind;
}

interface Grammar {
  /** Line comment openers. */
  line?: readonly string[];
  /** Block comment pairs. */
  block?: readonly (readonly [string, string])[];
  /** String delimiters, longest first; those in `spanning` may cross lines. */
  quotes: readonly string[];
  spanning?: readonly string[];
  keywords: ReadonlySet<string>;
  /** Literal words drawn as numbers: true, false, null. */
  constants?: ReadonlySet<string>;
  /** Built-in type names; with `capitalized`, any word starting upper-case is a type too. */
  types?: ReadonlySet<string>;
  capitalized?: boolean;
  /** Words compare without case (SQL). */
  caseless?: boolean;
  /** `-` continues a word (CSS). */
  dashed?: boolean;
  /** `'a` without a closing quote is a lifetime, not a string (Rust). */
  lifetimes?: boolean;
  /** `#` starts a comment only at a word's start (shell). */
  hashAtWord?: boolean;
  /** Tags, attributes and their values (HTML, XML). */
  markup?: boolean;
}

const words = (text: string): ReadonlySet<string> => new Set(text.split(' '));
const literals = words('true false null undefined');

const cLike = {
  line: ['//'],
  block: [['/*', '*/']] as const,
};

const script: Grammar = {
  ...cLike,
  quotes: ['`', '"', "'"],
  spanning: ['`'],
  keywords: words('as async await break case catch class const continue debugger default delete do else enum export extends finally for from function get if implements import in instanceof interface let new of package private protected public readonly return satisfies set static super switch this throw try type typeof var void while with yield keyof declare namespace abstract override'),
  constants: words('true false null undefined NaN Infinity'),
  types: words('string number boolean bigint symbol object unknown never any void'),
  capitalized: true,
};

const grammars: Record<string, Grammar> = {
  script,
  json: { quotes: ['"'], keywords: words(''), constants: literals },
  css: {
    block: [['/*', '*/']],
    quotes: ['"', "'"],
    keywords: words('@media @import @supports @keyframes @font-face @layer @container @theme @apply !important'),
    dashed: true,
  },
  markup: { block: [['<!--', '-->']], quotes: ['"', "'"], keywords: words(''), markup: true },
  python: {
    line: ['#'],
    quotes: ['"""', "'''", '"', "'"],
    spanning: ['"""', "'''"],
    keywords: words('and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield match case'),
    constants: words('True False None'),
    types: words('int float str bool bytes list dict set tuple object'),
    capitalized: true,
  },
  shell: {
    line: ['#'],
    quotes: ['"', "'"],
    keywords: words('if then else elif fi for while until do done case esac in function return local export readonly declare set unset shift exit break continue source alias'),
    hashAtWord: true,
  },
  yaml: { line: ['#'], quotes: ['"', "'"], keywords: words(''), constants: words('true false null yes no on off ~') },
  toml: { line: ['#'], quotes: ['"""', "'''", '"', "'"], spanning: ['"""', "'''"], keywords: words(''), constants: words('true false') },
  rust: {
    ...cLike,
    quotes: ['"', "'"],
    spanning: ['"'],
    keywords: words('as async await break const continue crate dyn else enum extern fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait type unsafe use where while'),
    constants: words('true false None Some Ok Err'),
    types: words('i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str String Vec Option Result Box'),
    capitalized: true,
    lifetimes: true,
  },
  kotlin: {
    ...cLike,
    quotes: ['"""', '"', "'"],
    spanning: ['"""'],
    keywords: words('as break class continue do else for fun if in interface is object package return super this throw try typealias val var when while by catch constructor finally get import init override private protected public internal set data sealed enum companion suspend lateinit open abstract'),
    constants: words('true false null'),
    capitalized: true,
  },
  swift: {
    ...cLike,
    quotes: ['"""', '"'],
    spanning: ['"""'],
    keywords: words('as associatedtype break case catch class continue default defer do else enum extension fallthrough for func guard if import in init inout internal is let operator private protocol public repeat return self Self static struct subscript super switch throw throws try typealias var where while async await some any final override mutating'),
    constants: words('true false nil'),
    capitalized: true,
  },
  java: {
    ...cLike,
    quotes: ['"""', '"', "'"],
    spanning: ['"""'],
    keywords: words('abstract assert break case catch class const continue default do else enum extends final finally for goto if implements import instanceof interface native new package private protected public return static super switch synchronized this throw throws transient try var void volatile while record'),
    constants: words('true false null'),
    types: words('boolean byte char double float int long short'),
    capitalized: true,
  },
  go: {
    ...cLike,
    quotes: ['`', '"', "'"],
    spanning: ['`'],
    keywords: words('break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var'),
    constants: words('true false nil iota'),
    types: words('bool byte complex64 complex128 error float32 float64 int int8 int16 int32 int64 rune string uint uint8 uint16 uint32 uint64 uintptr any'),
  },
  c: {
    ...cLike,
    quotes: ['"', "'"],
    keywords: words('auto break case class const constexpr continue default delete do else enum explicit extern for friend goto if inline namespace new operator private protected public return sizeof static struct switch template this throw try typedef typename union using virtual volatile while #include #define #ifdef #ifndef #endif #if #else #pragma'),
    constants: words('true false NULL nullptr'),
    types: words('bool char double float int long short signed unsigned void size_t'),
    capitalized: true,
  },
  sql: {
    line: ['--'],
    block: [['/*', '*/']],
    quotes: ["'", '"'],
    keywords: words('select from where and or not insert into values update set delete create table index view drop alter add join left right inner outer full on as group by order having limit offset union all distinct case when then else end is in like between exists primary key foreign references default returning with'),
    constants: words('true false null'),
    types: words('int integer bigint smallint text varchar char boolean date timestamp numeric real float double serial uuid json jsonb'),
    caseless: true,
  },
};

const names: Record<string, string> = {
  ts: 'script', typescript: 'script', tsx: 'script', js: 'script', javascript: 'script', jsx: 'script', mjs: 'script', cjs: 'script',
  json: 'json', jsonc: 'json', css: 'css', scss: 'css', less: 'css',
  html: 'markup', xml: 'markup', svg: 'markup',
  python: 'python', py: 'python', bash: 'shell', sh: 'shell', shell: 'shell', zsh: 'shell',
  yaml: 'yaml', yml: 'yaml', toml: 'toml', rust: 'rust', rs: 'rust', kotlin: 'kotlin', kt: 'kotlin', kts: 'kotlin',
  swift: 'swift', java: 'java', go: 'go', golang: 'go', c: 'c', h: 'c', cpp: 'c', 'c++': 'c', cc: 'c', hpp: 'c', sql: 'sql',
};

/** The grammar for a language's name or file extension, or undefined: such code stays plain. */
function grammarOf(language: string | undefined): Grammar | undefined {
  const name = language === undefined ? undefined : names[language.trim().toLowerCase()];
  return name === undefined ? undefined : grammars[name];
}

const space = /\s+/y;
const number = /(?:0[xXbBoO][\da-fA-F_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)[a-zA-Z%]*/y;
const word = /[A-Za-z_$][\w$]*/y;
const dashedWord = /[A-Za-z_$-][\w$-]*/y;
const lifetime = /'[A-Za-z_]\w*(?!')/y;
const tagName = /[A-Za-z][\w:.-]*/y;

function sticky(pattern: RegExp, code: string, at: number): string | undefined {
  pattern.lastIndex = at;
  return pattern.exec(code)?.[0];
}

/** Where a string opened at `at` with `quote` ends: after its closing quote, or at its line's end when it never closes. */
function stringEnd(code: string, at: number, quote: string, spans: boolean): number {
  let index = at + quote.length;
  while (index < code.length) {
    const char = code[index];
    if (char === '\\') { index += 2; continue; }
    if (char === '\n' && !spans) return index;
    if (code.startsWith(quote, index)) return index + quote.length;
    index += 1;
  }
  return code.length;
}

/** Code as runs of one kind each, neighbours of the same kind merged; plain text for an unknown language. */
export function highlight(code: string, language: string | undefined): CodeRun[] {
  const grammar = grammarOf(language);
  if (grammar === undefined) return code === '' ? [] : [{ text: code }];
  const runs: CodeRun[] = [];
  const push = (text: string, kind?: TokenKind) => {
    if (text === '') return;
    const last = runs[runs.length - 1];
    if (last !== undefined && last.kind === kind) last.text += text;
    else runs.push(kind === undefined ? { text } : { text, kind });
  };
  const fold = (text: string) => (grammar.caseless ? text.toLowerCase() : text);
  let inTag = false;
  let at = 0;
  while (at < code.length) {
    const rest = (open: string) => code.startsWith(open, at);
    const blank = sticky(space, code, at);
    if (blank) { push(blank); at += blank.length; continue; }
    const line = grammar.line?.find(rest);
    if (line !== undefined && !(grammar.hashAtWord && at > 0 && !/\s/.test(code[at - 1]))) {
      const end = code.indexOf('\n', at);
      const stop = end < 0 ? code.length : end;
      push(code.slice(at, stop), 'comment');
      at = stop;
      continue;
    }
    const block = grammar.block?.find(([open]) => rest(open));
    if (block !== undefined) {
      const end = code.indexOf(block[1], at + block[0].length);
      const stop = end < 0 ? code.length : end + block[1].length;
      push(code.slice(at, stop), 'comment');
      at = stop;
      continue;
    }
    if (grammar.markup && !inTag && code[at] === '<' && /[A-Za-z/!?]/.test(code[at + 1] ?? '')) {
      const open = code[at + 1] === '/' ? '</' : '<';
      push(open);
      at += open.length;
      const name = sticky(tagName, code, at);
      if (name) { push(name, 'type'); at += name.length; }
      inTag = true;
      continue;
    }
    if (grammar.markup && !inTag) {
      const next = code.indexOf('<', at + 1);
      const stop = next < 0 ? code.length : next;
      push(code.slice(at, stop));
      at = stop;
      continue;
    }
    if (inTag && code[at] === '>') { push('>'); at += 1; inTag = false; continue; }
    if (grammar.lifetimes) {
      const mark = sticky(lifetime, code, at);
      if (mark) { push(mark); at += mark.length; continue; }
    }
    const quote = grammar.quotes.find(rest);
    if (quote !== undefined) {
      const stop = stringEnd(code, at, quote, grammar.spanning?.includes(quote) ?? false);
      push(code.slice(at, stop), 'string');
      at = stop;
      continue;
    }
    if (/\d/.test(code[at]) || (code[at] === '.' && /\d/.test(code[at + 1] ?? ''))) {
      const digits = sticky(number, code, code[at] === '.' ? at + 1 : at);
      const stop = (code[at] === '.' ? at + 1 : at) + (digits?.length ?? 1);
      push(code.slice(at, stop), 'number');
      at = stop;
      continue;
    }
    // `@media`, `#include`, `!important`: a mark and a word, a keyword only together.
    if ('@#!'.includes(code[at])) {
      const marked = sticky(grammar.dashed ? dashedWord : word, code, at + 1);
      if (marked && grammar.keywords.has(fold(code[at] + marked))) { push(code[at] + marked, 'keyword'); at += 1 + marked.length; continue; }
    }
    const name = sticky(grammar.dashed || inTag ? dashedWord : word, code, at);
    if (name) {
      const key = fold(name);
      const kind: TokenKind | undefined = inTag ? undefined
        : grammar.keywords.has(key) ? 'keyword'
        : grammar.constants?.has(key) ? 'number'
        : grammar.types?.has(key) || (grammar.capitalized && /^[A-Z]/.test(name)) ? 'type'
        : undefined;
      push(name, kind);
      at += name.length;
      continue;
    }
    push(code[at]);
    at += 1;
  }
  return runs;
}

/** The languages `highlight` colours, by the names and file extensions it takes. */
export const languages: readonly string[] = Object.keys(names);
