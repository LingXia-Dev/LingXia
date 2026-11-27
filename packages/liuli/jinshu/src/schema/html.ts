// Pasted HTML (a web page, a word processor) read into Jinshu blocks. The HTML is untrusted: only a
// fixed set of structure and marks is kept, written as Jinshu Markup and read back by `parseAgentMarkup`,
// so links pass the same validation as an agent's. Everything else unwraps to its text or is dropped.
import { parseAgentMarkup } from './markup';
import { documentToMarkdown, markdownToBlocks } from './markdown';
import { createDocument } from './helpers';
import { validateLink } from './validate';
import type { CheckOptions, JinshuBlock, JinshuFormat } from './types';

interface HtmlNode {
  readonly tag: string;
  readonly attrs: Readonly<Record<string, string>>;
  readonly children: (HtmlNode | string)[];
}

// Skipped with everything inside.
const DROPPED = new Set([
  'head', 'script', 'style', 'title', 'template', 'noscript', 'iframe', 'object', 'embed', 'svg', 'math', 'canvas',
  'audio', 'video', 'select', 'textarea', 'button', 'frameset', 'frame', 'noembed', 'noframes', 'applet', 'xml', 'img', 'picture',
]);
// Their body is text up to the closing tag, never markup.
const RAW_TEXT = new Set(['script', 'style', 'textarea', 'title', 'xmp', 'noscript', 'iframe', 'noembed', 'noframes']);
const VOID = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr']);
const BLOCK = new Set([
  'address', 'article', 'aside', 'blockquote', 'center', 'dd', 'details', 'dialog', 'div', 'dl', 'dt', 'fieldset', 'figcaption',
  'figure', 'footer', 'form', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'header', 'hgroup', 'hr', 'li', 'main', 'nav', 'ol', 'p', 'pre',
  'section', 'summary', 'table', 'tbody', 'td', 'tfoot', 'th', 'thead', 'tr', 'ul', 'body', 'html', 'caption',
]);
// An open `<p>` closes when one of these starts, as in HTML.
const CLOSES_P = new Set([...BLOCK].filter(t => !['td', 'th', 'tr', 'tbody', 'thead', 'tfoot', 'caption', 'body', 'html', 'dd', 'dt'].includes(t)));
const MAX_DEPTH = 200;
const MAX_LIST_DEPTH = 6;

const ENTITIES: Readonly<Record<string, string>> = {
  amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ', ensp: ' ', emsp: ' ', thinsp: ' ', shy: '',
  mdash: '—', ndash: '–', hellip: '…', lsquo: '‘', rsquo: '’', ldquo: '“', rdquo: '”', laquo: '«', raquo: '»',
  bull: '•', middot: '·', copy: '©', reg: '®', trade: '™', deg: '°', times: '×', divide: '÷', euro: '€', pound: '£',
  yen: '¥', cent: '¢', sect: '§', para: '¶', plusmn: '±', larr: '←', rarr: '→', uarr: '↑', darr: '↓', zwj: '‍', zwnj: '‌',
};

function decodeEntities(text: string): string {
  return text.replace(/&(#[xX][0-9a-fA-F]{1,6}|#[0-9]{1,7}|[a-zA-Z]{2,8});?/g, (whole, body: string) => {
    if (body[0] === '#') {
      const code = body[1] === 'x' || body[1] === 'X' ? parseInt(body.slice(2), 16) : parseInt(body.slice(1), 10);
      if (!Number.isFinite(code) || code === 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return '�';
      return String.fromCodePoint(code);
    }
    return ENTITIES[body] ?? ENTITIES[body.toLowerCase()] ?? whole;
  });
}

/** A forgiving HTML reader: unclosed and misnested tags, unquoted attributes, comments and CDATA all read. */
function parseHtml(src: string): HtmlNode {
  const root: HtmlNode = { tag: '#root', attrs: {}, children: [] };
  const stack: HtmlNode[] = [root];
  const top = () => stack[stack.length - 1];
  const closeTo = (index: number) => {
    stack.length = Math.max(1, index);
  };
  // The nearest open `tag` above any of `bounds`, or -1.
  const openIndex = (tags: ReadonlySet<string>, bounds: ReadonlySet<string>) => {
    for (let i = stack.length - 1; i > 0; i--) {
      if (tags.has(stack[i].tag)) return i;
      if (bounds.has(stack[i].tag)) return -1;
    }
    return -1;
  };
  const TABLE_BOUNDS = new Set(['table', 'td', 'th', 'caption']);
  const implicitClose = (tag: string) => {
    if (CLOSES_P.has(tag)) {
      const p = openIndex(new Set(['p']), TABLE_BOUNDS);
      if (p > 0) closeTo(p);
    }
    let found = -1;
    if (tag === 'li') found = openIndex(new Set(['li']), new Set(['ul', 'ol', ...TABLE_BOUNDS]));
    else if (tag === 'dt' || tag === 'dd') found = openIndex(new Set(['dt', 'dd']), new Set(['dl', ...TABLE_BOUNDS]));
    else if (tag === 'tr') found = openIndex(new Set(['tr']), new Set(['table']));
    else if (tag === 'td' || tag === 'th') found = openIndex(new Set(['td', 'th']), new Set(['tr', 'table']));
    else if (tag === 'thead' || tag === 'tbody' || tag === 'tfoot') found = openIndex(new Set(['thead', 'tbody', 'tfoot']), new Set(['table']));
    else if (/^h[1-6]$/.test(tag)) found = openIndex(new Set(['h1', 'h2', 'h3', 'h4', 'h5', 'h6']), TABLE_BOUNDS);
    if (found > 0) closeTo(found);
  };

  const lower = src.toLowerCase();
  let pos = 0;
  const pushText = (text: string) => {
    if (text) top().children.push(decodeEntities(text));
  };
  while (pos < src.length) {
    const lt = src.indexOf('<', pos);
    if (lt < 0) {
      pushText(src.slice(pos));
      break;
    }
    pushText(src.slice(pos, lt));
    pos = lt;
    if (src.startsWith('<!--', pos)) {
      const end = src.indexOf('-->', pos + 4);
      pos = end < 0 ? src.length : end + 3;
      continue;
    }
    if (src[pos + 1] === '!' || src[pos + 1] === '?') {
      // Doctype, CDATA, a word processor's `<![if ...]>`, or a processing instruction.
      const end = src.indexOf('>', pos);
      pos = end < 0 ? src.length : end + 1;
      continue;
    }
    const closing = src[pos + 1] === '/';
    const nameMatch = /^[a-zA-Z][a-zA-Z0-9:_-]*/.exec(src.slice(pos + (closing ? 2 : 1), pos + (closing ? 2 : 1) + 64));
    if (!nameMatch) {
      pushText('<');
      pos += 1;
      continue;
    }
    const tag = nameMatch[0].toLowerCase();
    pos += (closing ? 2 : 1) + nameMatch[0].length;

    // Attributes, up to `>`; quotes may hold `>`.
    const attrs: Record<string, string> = {};
    let selfClosing = false;
    while (pos < src.length && src[pos] !== '>') {
      const ch = src[pos];
      if (/\s/.test(ch)) {
        pos += 1;
        continue;
      }
      if (ch === '/') {
        selfClosing = src[pos + 1] === '>';
        pos += 1;
        continue;
      }
      const attr = /^[^\s"'>/=]+/.exec(src.slice(pos, pos + 256));
      if (!attr) {
        pos += 1;
        continue;
      }
      pos += attr[0].length;
      while (pos < src.length && /\s/.test(src[pos])) pos += 1;
      let value = '';
      if (src[pos] === '=') {
        pos += 1;
        while (pos < src.length && /\s/.test(src[pos])) pos += 1;
        const q = src[pos];
        if (q === '"' || q === "'") {
          const end = src.indexOf(q, pos + 1);
          value = src.slice(pos + 1, end < 0 ? src.length : end);
          pos = end < 0 ? src.length : end + 1;
        } else {
          const unquoted = /^[^\s>]*/.exec(src.slice(pos, pos + 2048))![0];
          value = unquoted;
          pos += unquoted.length;
        }
      }
      if (!closing) attrs[attr[0].toLowerCase()] = decodeEntities(value);
    }
    pos += 1; // `>`

    if (closing) {
      for (let i = stack.length - 1; i > 0; i--) {
        if (stack[i].tag === tag) {
          closeTo(i);
          break;
        }
      }
      continue;
    }
    if (RAW_TEXT.has(tag)) {
      const end = lower.indexOf(`</${tag}`, pos);
      pos = end < 0 ? src.length : src.indexOf('>', end) + 1 || src.length;
      continue;
    }
    implicitClose(tag);
    const node: HtmlNode = { tag, attrs, children: [] };
    top().children.push(node);
    if (!VOID.has(tag) && !selfClosing && stack.length < MAX_DEPTH) stack.push(node);
  }
  return root;
}

// ---------------------------------------------------------------------------------------------
// HTML tree to Jinshu Markup

interface Marks {
  readonly bold?: boolean;
  readonly italic?: boolean;
  readonly underline?: boolean;
  readonly strike?: boolean;
  readonly script?: 'sub' | 'super';
  readonly code?: boolean;
  readonly highlight?: boolean;
  readonly link?: string;
}

type Run = { readonly text: string; readonly marks: Marks } | { readonly br: true };

interface Context {
  readonly check?: CheckOptions;
  readonly inQuote: boolean;
  readonly inItem: boolean;
  readonly listDepth: number;
}

function escapeXml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function styleOf(node: HtmlNode): Record<string, string> {
  const out: Record<string, string> = {};
  for (const decl of (node.attrs.style ?? '').split(';')) {
    const colon = decl.indexOf(':');
    if (colon > 0) out[decl.slice(0, colon).trim().toLowerCase()] = decl.slice(colon + 1).trim().toLowerCase();
  }
  return out;
}

function isDropped(node: HtmlNode): boolean {
  if (DROPPED.has(node.tag)) return true;
  if (node.attrs.hidden !== undefined || node.attrs['aria-hidden'] === 'true') return true;
  const style = styleOf(node);
  // A word processor's list bullets and hidden text.
  return /ignore/.test(style['mso-list'] ?? '') || style.display === 'none';
}

function marksOf(node: HtmlNode, marks: Marks, check: CheckOptions | undefined): Marks {
  let next: { -readonly [K in keyof Marks]: Marks[K] } = { ...marks };
  switch (node.tag) {
    case 'b':
    case 'strong':
      next.bold = true;
      break;
    case 'i':
    case 'em':
    case 'cite':
    case 'dfn':
    case 'var':
      next.italic = true;
      break;
    case 'u':
    case 'ins':
      next.underline = true;
      break;
    case 's':
    case 'strike':
    case 'del':
      next.strike = true;
      break;
    case 'sub':
      next.script = 'sub';
      break;
    case 'sup':
      next.script = 'super';
      break;
    case 'code':
    case 'kbd':
    case 'samp':
    case 'tt':
      next.code = true;
      break;
    case 'mark':
      next.highlight = true;
      break;
    case 'a': {
      const href = (node.attrs.href ?? '').trim();
      // Another page's `#anchor` points at nothing here.
      if (href && !href.startsWith('#') && validateLink(href, check) === null) next.link = href;
      break;
    }
  }
  // Inline styles, as Google Docs and word processors write marks; `font-weight: normal` undoes a `<b>` wrapper.
  const style = styleOf(node);
  const weight = style['font-weight'];
  if (weight) next.bold = /^(bold|bolder|[6-9]00)$/.test(weight) ? true : /^(normal|lighter|[1-5]00)$/.test(weight) ? false : next.bold;
  const fontStyle = style['font-style'];
  if (fontStyle) next.italic = /italic|oblique/.test(fontStyle);
  const decoration = `${style['text-decoration'] ?? ''} ${style['text-decoration-line'] ?? ''}`;
  if (/underline/.test(decoration)) next.underline = true;
  if (/line-through/.test(decoration)) next.strike = true;
  const align = style['vertical-align'];
  if (align === 'super' || align === 'sub') next.script = align;
  if (next.bold === false) {
    const { bold: _, ...rest } = next;
    next = rest;
  }
  return next;
}

function pushText(runs: Run[], raw: string, marks: Marks): void {
  let text = raw.replace(/[\t\n\r\f ]+/g, ' ');
  const last = runs[runs.length - 1];
  const atStart = !last || 'br' in last || last.text.endsWith(' ');
  if (atStart) text = text.replace(/^ /, '');
  if (text) runs.push({ text, marks });
}

function trimRuns(runs: Run[]): Run[] {
  const out = runs.map(r => ('br' in r ? r : { ...r, text: r.text.replace(/ /g, ' ') }));
  while (out.length > 0) {
    const last = out[out.length - 1];
    if ('br' in last) {
      out.pop();
      continue;
    }
    const text = last.text.replace(/\s+$/, '');
    if (text) {
      out[out.length - 1] = { ...last, text };
      break;
    }
    out.pop();
  }
  // A break's neighbours lose the space it collapsed with.
  for (let i = 0; i < out.length; i++) {
    const r = out[i];
    if ('br' in r) continue;
    let text = r.text;
    if (i > 0 && 'br' in out[i - 1]) text = text.replace(/^\s+/, '');
    if (i + 1 < out.length && 'br' in out[i + 1]) text = text.replace(/\s+$/, '');
    out[i] = { ...r, text };
  }
  const first = out.findIndex(r => 'br' in r || r.text.trim() !== '');
  return first < 0 ? [] : out.slice(first).filter(r => 'br' in r || r.text !== '');
}

function runsMarkup(runs: readonly Run[], inHeading = false): string {
  return runs
    .map(run => {
      if ('br' in run) return inHeading ? ' ' : '<br/>';
      const m = run.marks;
      let s = escapeXml(run.text);
      if (m.code) s = `<code>${s}</code>`;
      if (m.script) s = m.script === 'sub' ? `<sub>${s}</sub>` : `<sup>${s}</sup>`;
      if (m.strike) s = `<s>${s}</s>`;
      if (m.underline) s = `<u>${s}</u>`;
      if (m.italic) s = `<i>${s}</i>`;
      if (m.bold) s = `<b>${s}</b>`;
      if (m.highlight) s = `<mark>${s}</mark>`;
      if (m.link) s = `<a href="${escapeXml(m.link)}">${s}</a>`;
      return s;
    })
    .join('');
}

/** The text of a subtree as code: whitespace kept, breaks as newlines. */
function codeText(node: HtmlNode | string): string {
  if (typeof node === 'string') return node;
  if (isDropped(node)) return '';
  if (node.tag === 'br') return '\n';
  const inner = node.children.map(codeText).join('');
  return BLOCK.has(node.tag) && node.tag !== 'pre' && inner && !inner.endsWith('\n') ? `${inner}\n` : inner;
}

/** Inline content of `nodes`, block descendants flattened into the same line. */
function inlineRuns(nodes: readonly (HtmlNode | string)[], marks: Marks, ctx: Context, runs: Run[] = []): Run[] {
  for (const node of nodes) {
    if (typeof node === 'string') pushText(runs, node, marks);
    else if (isDropped(node)) continue;
    else if (node.tag === 'br') runs.push({ br: true });
    else {
      if (BLOCK.has(node.tag)) pushText(runs, ' ', marks);
      inlineRuns(node.children, marksOf(node, marks, ctx.check), ctx, runs);
      if (BLOCK.has(node.tag)) pushText(runs, ' ', marks);
    }
  }
  return runs;
}

class BlockWriter {
  readonly blocks: string[] = [];
  private runs: Run[] = [];
  private readonly ctx: Context;

  constructor(ctx: Context) {
    this.ctx = ctx;
  }

  flush(): void {
    const runs = trimRuns(this.runs);
    this.runs = [];
    if (runs.length > 0) this.blocks.push(`<p>${runsMarkup(runs)}</p>`);
  }

  walk(nodes: readonly (HtmlNode | string)[], marks: Marks): void {
    for (const node of nodes) {
      if (typeof node === 'string') pushText(this.runs, node, marks);
      else if (isDropped(node)) continue;
      else if (node.tag === 'br') this.runs.push({ br: true });
      else if (BLOCK.has(node.tag)) {
        this.flush();
        this.block(node, marks);
        this.flush();
      } else this.walk(node.children, marksOf(node, marks, this.ctx.check));
    }
  }

  private block(node: HtmlNode, marks: Marks): void {
    const { ctx } = this;
    const heading = /^h([1-6])$/.exec(node.tag);
    if (heading) {
      const runs = trimRuns(inlineRuns(node.children, marks, ctx));
      if (runs.length === 0) return;
      // A quote or a list item holds no heading.
      if (ctx.inQuote || ctx.inItem) this.blocks.push(`<p>${runsMarkup(runs.map(r => ('br' in r ? r : { ...r, marks: { ...r.marks, bold: true } })))}</p>`);
      else {
        const level = Math.min(4, Number(heading[1]));
        this.blocks.push(`<h${level}>${runsMarkup(runs, true)}</h${level}>`);
      }
      return;
    }
    switch (node.tag) {
      case 'ul':
      case 'ol':
        return this.list(node, marks);
      case 'blockquote': {
        if (ctx.inItem) return this.walk(node.children, marks);
        const inner = new BlockWriter({ ...ctx, inQuote: true });
        inner.walk(node.children, marks);
        inner.flush();
        if (inner.blocks.length > 0) this.blocks.push(`<quote>${inner.blocks.join('')}</quote>`);
        return;
      }
      case 'pre': {
        const code = codeText(node).replace(/\r\n?/g, '\n').replace(/^\n/, '').replace(/\n$/, '');
        if (!code.trim()) return;
        const classes = `${node.attrs.class ?? ''} ${node.children.map(c => (typeof c === 'string' ? '' : c.attrs.class ?? '')).join(' ')}`;
        const lang = /(?:^|\s)(?:language|lang)-([a-z0-9_+#-]{1,24})(?:\s|$)/i.exec(classes)?.[1]?.toLowerCase() ?? 'text';
        let longest = 0;
        for (const m of code.matchAll(/`+/g)) longest = Math.max(longest, m[0].length);
        const fence = '`'.repeat(Math.max(3, longest + 1));
        this.blocks.push(`<code language="${escapeXml(lang)}">\n${fence}\n${code}\n${fence}\n</code>`);
        return;
      }
      case 'hr':
        if (!ctx.inItem) this.blocks.push('<divider/>');
        return;
      case 'table':
        return this.table(node, marks);
      case 'li':
        // An item outside a list reads as its text.
        return this.walk(node.children, marks);
      default:
        return this.walk(node.children, marks);
    }
  }

  private list(node: HtmlNode, marks: Marks): void {
    const { ctx } = this;
    const items = node.children.filter((c): c is HtmlNode => typeof c !== 'string' && c.tag === 'li' && !isDropped(c));
    if (ctx.listDepth >= MAX_LIST_DEPTH) {
      // Deeper than a document nests: the items go on as paragraphs.
      for (const item of items) this.walk(item.children, marks);
      return;
    }
    const checkboxOf = (item: HtmlNode): HtmlNode | undefined => {
      for (const c of item.children) {
        if (typeof c === 'string') {
          if (c.trim()) return undefined;
          continue;
        }
        if (c.tag === 'input' && (c.attrs.type ?? '').toLowerCase() === 'checkbox') return c;
        if (c.tag === 'p' || c.tag === 'label' || c.tag === 'span') return checkboxOf(c);
        return undefined;
      }
      return undefined;
    };
    const isTask = items.some(item => checkboxOf(item));
    const written: string[] = [];
    for (const item of items) {
      const inner = new BlockWriter({ ...ctx, inItem: true, inQuote: false, listDepth: ctx.listDepth + 1 });
      inner.walk(item.children, marks);
      inner.flush();
      let runs = '';
      let rest = inner.blocks;
      if (rest[0]?.startsWith('<p>')) {
        runs = rest[0].slice(3, -4);
        rest = rest.slice(1);
      }
      const box = isTask ? checkboxOf(item) : undefined;
      const checked = box && box.attrs.checked !== undefined ? ' checked="true"' : '';
      if (!runs && rest.length === 0 && !box) continue;
      written.push(rest.length > 0 ? `<li${checked}><p>${runs}</p>${rest.join('')}</li>` : `<li${checked}>${runs}</li>`);
    }
    if (written.length === 0) return;
    const style = isTask ? 'task' : node.tag === 'ol' ? 'ordered' : 'bullet';
    const start = Number(node.attrs.start);
    const startAttr = style === 'ordered' && Number.isInteger(start) && start >= 0 && start !== 1 && start < 1e6 ? ` start="${start}"` : '';
    this.blocks.push(`<list style="${style}"${startAttr}>${written.join('')}</list>`);
  }

  private table(node: HtmlNode, marks: Marks): void {
    const { ctx } = this;
    const rows: { readonly cells: HtmlNode[]; readonly head: boolean }[] = [];
    const collect = (parent: HtmlNode, head: boolean) => {
      for (const c of parent.children) {
        if (typeof c === 'string' || isDropped(c)) continue;
        if (c.tag === 'tr') rows.push({ cells: c.children.filter((x): x is HtmlNode => typeof x !== 'string' && (x.tag === 'td' || x.tag === 'th')), head });
        else if (c.tag === 'thead' || c.tag === 'tbody' || c.tag === 'tfoot') collect(c, c.tag === 'thead');
      }
    };
    collect(node, false);
    const width = Math.max(0, ...rows.map(r => r.cells.length));
    // A layout table, or one where tables cannot go, reads as its content.
    if (width <= 1 || ctx.inItem) {
      for (const cell of rows.flatMap(row => row.cells)) {
        this.walk(cell.children, marks);
        this.flush();
      }
      return;
    }
    const headerRow = rows[0].head || rows[0].cells.every(c => c.tag === 'th');
    const body = rows
      .map(row => {
        const cells = row.cells.map(cell => runsMarkup(trimRuns(inlineRuns(cell.children, marks, ctx))));
        while (cells.length < width) cells.push('');
        return `<tr>${cells.map(c => `<td>${c}</td>`).join('')}</tr>`;
      })
      .join('');
    this.blocks.push(`<table${headerRow ? ' headerRow="true"' : ''}>${body}</table>`);
  }
}

/** Jinshu Markup for pasted HTML: the structure and marks Jinshu keeps, nothing else. */
export function htmlToMarkup(html: string, options?: { readonly check?: CheckOptions }): string {
  const root = parseHtml(String(html ?? '').replace(/\r\n?/g, '\n'));
  const writer = new BlockWriter({ check: options?.check, inQuote: false, inItem: false, listDepth: 0 });
  writer.walk(root.children, {});
  writer.flush();
  return writer.blocks.join('\n');
}

/**
 * Reads pasted HTML into blocks with fresh ids: headings, paragraphs, lists, quotes, code, tables and
 * inline marks; links only where `validateLink` allows them. In a `markdown` document what Markdown
 * cannot hold is dropped.
 */
export function htmlToBlocks(html: string, options?: { readonly format?: JinshuFormat; readonly check?: CheckOptions }): JinshuBlock[] {
  const { blocks } = parseAgentMarkup(htmlToMarkup(html, options), { check: options?.check });
  if (options?.format !== 'markdown' || blocks.length === 0) return [...blocks];
  const { text } = documentToMarkdown(createDocument({ blocks: [...blocks] }));
  return markdownToBlocks(text, { format: 'markdown' });
}
