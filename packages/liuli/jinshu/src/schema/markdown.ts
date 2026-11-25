/**
 * Markdown as a storage encoding of a Jinshu document. There is one model; a Markdown file holds the
 * subset Markdown can say (see `profile.ts`) in ordinary CommonMark + GFM syntax, `$math$`, `$$` blocks
 * and alert-syntax callouts, with no syntax of Jinshu's own.
 *
 * Markdown has no ids, so reading derives them from content and, given the previous document, keeps the
 * ids of blocks that are unchanged or stand in the same place.
 */
import { hashString, newBlockId, paragraph } from './helpers';
import {
  cellSource,
  headingSource,
  normalizeLabel,
  paragraphSource,
  parseInlineNodes,
  scanDestination,
  scanLabel,
  scanTitle,
  unescapeMarkdown,
  writeDestination,
  writeInlines,
  type InlineContext,
  type InlineDialect,
  type InlineImage,
  type LinkReference,
} from './inlineMarkdown';
import { CALLOUT_ALERTS, checkMarkdownProfile } from './profile';
import { emptyRegistry } from './registry';
import type {
  CalloutTone,
  FormatRule,
  Inline,
  InlineRun,
  JinshuBlock,
  JinshuDocument,
  JinshuFormat,
  JinshuRegistry,
  ListBlock,
  Problem,
  TableAlign,
} from './types';
import { JINSHU_DOC_VERSION } from './types';
import { checkDocument, normalizeDocument, normalizeInlines } from './validate';

// ============================================================================
// Block parser (CommonMark container/leaf algorithm, plus GFM tables and `$$` math)
// ============================================================================

type MdType =
  | 'document'
  | 'block_quote'
  | 'list'
  | 'item'
  | 'paragraph'
  | 'heading'
  | 'thematic_break'
  | 'code_block'
  | 'html_block'
  | 'math_block'
  | 'table';

interface ListData {
  readonly type: 'bullet' | 'ordered';
  readonly bulletChar: string;
  readonly start: number;
  readonly delimiter: string;
  padding: number;
  readonly markerOffset: number;
}

interface MdNode {
  type: MdType;
  parent: MdNode | null;
  children: MdNode[];
  open: boolean;
  line: number;
  content: string;
  level?: number;
  fenced?: boolean;
  fenceChar?: string;
  fenceLength?: number;
  fenceOffset?: number;
  info?: string;
  list?: ListData;
  htmlType?: number;
  align?: (TableAlign | null)[];
  rows?: string[][];
  extraCells?: boolean;
  empty?: boolean;
}

const CODE_INDENT = 4;
const MAYBE_SPECIAL_RE = /^[#`~*+_=<>0-9$|:-]/;
const ATX_RE = /^#{1,6}(?:[ \t]+|$)/;
const FENCE_RE = /^`{3,}(?!.*`)|^~{3,}/;
const CLOSING_FENCE_RE = /^(?:`{3,}|~{3,})(?=[ \t]*$)/;
const SETEXT_RE = /^(?:=+|-+)[ \t]*$/;
const THEMATIC_RE = /^(?:\*[ \t]*){3,}$|^(?:_[ \t]*){3,}$|^(?:-[ \t]*){3,}$/;
const BULLET_RE = /^[*+-]/;
const ORDERED_RE = /^(\d{1,9})([.)])/;
const MATH_FENCE_RE = /^\$\$[ \t]*$/;
const MATH_ONE_LINE_RE = /^\$\$(.*\S.*)\$\$[ \t]*$/;
const DELIM_ROW_RE = /^\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;
const ATTR = `(?:\\s+[A-Za-z_:][A-Za-z0-9_.:-]*(?:\\s*=\\s*(?:[^"'=<>\`\\x00-\\x20]+|'[^']*'|"[^"]*"))?)`;
const HTML_OPEN: readonly RegExp[] = [
  /./,
  /^<(?:script|pre|textarea|style)(?:\s|>|$)/i,
  /^<!--/,
  /^<[?]/,
  /^<![A-Za-z]/,
  /^<!\[CDATA\[/,
  /^<[/]?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[123456]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?:\s|[/]?[>]|$)/i,
  new RegExp(`^(?:<[A-Za-z][A-Za-z0-9-]*${ATTR}*\\s*/?>|</[A-Za-z][A-Za-z0-9-]*\\s*>)\\s*$`, 'i'),
];
const HTML_CLOSE: readonly RegExp[] = [/./, /<\/(?:script|pre|textarea|style)>/i, /-->/, /\?>/, />/, /\]\]>/];

const acceptsLines = (t: MdType) => t === 'paragraph' || t === 'code_block' || t === 'html_block' || t === 'math_block';
const canContain = (parent: MdType, child: MdType) =>
  parent === 'document' || parent === 'block_quote' || parent === 'item' ? child !== 'item' : parent === 'list' ? child === 'item' : false;
const isSpaceOrTab = (c: string | undefined) => c === ' ' || c === '\t';
/** Markdown trims only spaces, tabs and newlines; other Unicode spaces are content. */
const trimMd = (s: string) => s.replace(/^[ \t\n]+|[ \t\n]+$/g, '');
const hasContent = (s: string) => /[^ \t\r\n]/.test(s);

function newNode(type: MdType, line: number): MdNode {
  return { type, parent: null, children: [], open: true, line, content: '' };
}

/** Splits a table row into raw cells on unescaped pipes. */
function splitRow(line: string): string[] {
  let s = trimMd(line);
  if (s.startsWith('|')) s = s.slice(1);
  const cells: string[] = [];
  let cur = '';
  let closed = false;
  for (let i = 0; i < s.length; i++) {
    closed = false;
    if (s[i] === '\\' && i + 1 < s.length) {
      cur += s[i] + s[i + 1];
      i += 1;
    } else if (s[i] === '|') {
      cells.push(cur);
      cur = '';
      closed = true;
    } else cur += s[i];
  }
  if (!closed || cells.length === 0) cells.push(cur);
  return cells;
}

interface RefDef {
  readonly label: string;
  readonly ref: LinkReference;
  readonly length: number;
}

/** A link reference definition at the start of `s`. */
function parseRefDef(s: string): RefDef | null {
  const label = scanLabel(s, 0);
  if (!label || s[label.end] !== ':' || label.label.startsWith('^')) return null;
  let i = label.end + 1;
  while (s[i] === ' ' || s[i] === '\t') i++;
  if (s[i] === '\n') i++;
  while (s[i] === ' ' || s[i] === '\t') i++;
  const dest = scanDestination(s, i, true);
  if (!dest) return null;
  const afterDest = dest.end;
  const lineEnd = (from: number) => {
    let j = from;
    while (s[j] === ' ' || s[j] === '\t') j++;
    return j >= s.length ? j : s[j] === '\n' ? j + 1 : -1;
  };
  let j = afterDest;
  while (s[j] === ' ' || s[j] === '\t') j++;
  let nl = false;
  if (s[j] === '\n') {
    nl = true;
    j++;
    while (s[j] === ' ' || s[j] === '\t') j++;
  }
  if (j > afterDest) {
    const title = scanTitle(s, j, true);
    if (title) {
      const end = lineEnd(title.end);
      if (end >= 0) return { label: label.label, ref: { href: dest.dest, title: title.title }, length: end };
    }
  }
  const end = lineEnd(afterDest);
  if (end < 0 || (nl && end < 0)) return null;
  return { label: label.label, ref: { href: dest.dest }, length: end };
}

class BlockParser {
  readonly doc: MdNode = newNode('document', 0);
  readonly refs = new Map<string, LinkReference>();
  private tip: MdNode = this.doc;
  private oldtip: MdNode = this.doc;
  private line = '';
  private lineNumber = 0;
  private offset = 0;
  private column = 0;
  private nextNonspace = 0;
  private nextNonspaceColumn = 0;
  private indent = 0;
  private indented = false;
  private blank = false;
  private partiallyConsumedTab = false;
  private allClosed = true;
  private lastMatched: MdNode = this.doc;

  private readonly lineBase: number;

  constructor(lineBase: number) {
    this.lineBase = lineBase;
  }

  parse(text: string): MdNode {
    const lines = text.split('\n');
    if (text.endsWith('\n')) lines.pop();
    for (const l of lines) this.incorporate(l);
    while (this.tip !== this.doc) this.finalize(this.tip);
    this.finalize(this.doc);
    return this.doc;
  }

  private findNextNonspace(): void {
    let i = this.offset;
    let cols = this.column;
    let c: string | undefined;
    while ((c = this.line[i]) !== undefined) {
      if (c === ' ') {
        i++;
        cols++;
      } else if (c === '\t') {
        i++;
        cols += 4 - (cols % 4);
      } else break;
    }
    this.blank = c === undefined;
    this.nextNonspace = i;
    this.nextNonspaceColumn = cols;
    this.indent = this.nextNonspaceColumn - this.column;
    this.indented = this.indent >= CODE_INDENT;
  }

  private advanceNextNonspace(): void {
    this.offset = this.nextNonspace;
    this.column = this.nextNonspaceColumn;
    this.partiallyConsumedTab = false;
  }

  private advanceOffset(count: number, columns: boolean): void {
    let n = count;
    let c: string | undefined;
    while (n > 0 && (c = this.line[this.offset]) !== undefined) {
      if (c === '\t') {
        const toTab = 4 - (this.column % 4);
        if (columns) {
          this.partiallyConsumedTab = toTab > n;
          const adv = toTab > n ? n : toTab;
          this.column += adv;
          this.offset += this.partiallyConsumedTab ? 0 : 1;
          n -= adv;
        } else {
          this.partiallyConsumedTab = false;
          this.column += toTab;
          this.offset += 1;
          n -= 1;
        }
      } else {
        this.partiallyConsumedTab = false;
        this.offset += 1;
        this.column += 1;
        n -= 1;
      }
    }
  }

  private addLine(): void {
    if (this.partiallyConsumedTab) {
      this.offset += 1;
      this.tip.content += ' '.repeat(4 - (this.column % 4));
    }
    this.tip.content += `${this.line.slice(this.offset)}\n`;
  }

  private addChild(type: MdType): MdNode {
    while (!canContain(this.tip.type, type)) this.finalize(this.tip);
    const node = newNode(type, this.lineNumber + this.lineBase);
    node.parent = this.tip;
    this.tip.children.push(node);
    this.tip = node;
    return node;
  }

  private closeUnmatched(): void {
    if (this.allClosed) return;
    while (this.oldtip !== this.lastMatched) {
      const parent = this.oldtip.parent!;
      this.finalize(this.oldtip);
      this.oldtip = parent;
    }
    this.allClosed = true;
  }

  private finalize(block: MdNode): void {
    block.open = false;
    switch (block.type) {
      case 'paragraph': {
        let content = block.content;
        let def: RefDef | null;
        while (content.startsWith('[') && (def = parseRefDef(content))) {
          const key = normalizeLabel(def.label);
          if (!this.refs.has(key)) this.refs.set(key, def.ref);
          content = content.slice(def.length);
        }
        block.content = content;
        if (!hasContent(content)) block.empty = true;
        break;
      }
      case 'code_block':
        if (block.fenced) {
          const nl = block.content.indexOf('\n');
          block.info = unescapeMarkdown(block.content.slice(0, nl).trim(), true);
          block.content = block.content.slice(nl + 1);
        } else block.content = block.content.replace(/(\n *)+$/, '\n');
        break;
      case 'math_block':
        if (block.fenced) block.content = block.content.slice(block.content.indexOf('\n') + 1);
        break;
    }
    this.tip = block.parent ?? this.doc;
  }

  /** 0 matched, 1 not matched, 2 the line is used up. */
  private continues(c: MdNode): 0 | 1 | 2 {
    const ln = this.line;
    switch (c.type) {
      case 'document':
      case 'list':
        return 0;
      case 'block_quote':
        if (!this.indented && ln[this.nextNonspace] === '>') {
          this.advanceNextNonspace();
          this.advanceOffset(1, false);
          if (isSpaceOrTab(ln[this.offset])) this.advanceOffset(1, true);
          return 0;
        }
        return 1;
      case 'item':
        if (this.blank) {
          if (c.children.length === 0) return 1;
          this.advanceNextNonspace();
        } else if (this.indent >= c.list!.markerOffset + c.list!.padding) {
          this.advanceOffset(c.list!.markerOffset + c.list!.padding, true);
        } else return 1;
        return 0;
      case 'heading':
      case 'thematic_break':
        return 1;
      case 'code_block':
      case 'math_block': {
        if (c.fenced) {
          const rest = ln.slice(this.nextNonspace);
          const closing =
            this.indent <= 3 &&
            (c.type === 'math_block' ? MATH_FENCE_RE.test(rest) : rest[0] === c.fenceChar && (CLOSING_FENCE_RE.exec(rest)?.[0].length ?? 0) >= c.fenceLength!);
          if (closing) {
            this.finalize(c);
            return 2;
          }
          let i = c.fenceOffset!;
          while (i > 0 && isSpaceOrTab(ln[this.offset])) {
            this.advanceOffset(1, true);
            i--;
          }
          return 0;
        }
        if (this.indent >= CODE_INDENT) this.advanceOffset(CODE_INDENT, true);
        else if (this.blank) this.advanceNextNonspace();
        else return 1;
        return 0;
      }
      case 'html_block':
        return this.blank && (c.htmlType === 6 || c.htmlType === 7) ? 1 : 0;
      case 'paragraph':
      case 'table':
        return this.blank ? 1 : 0;
    }
  }

  /** Tries each block start; 0 none, 1 a container, 2 a leaf. */
  private starts(container: MdNode): 0 | 1 | 2 {
    const ln = this.line;
    const rest = ln.slice(this.nextNonspace);
    const indented = this.indented;
    // Block quote
    if (!indented && rest[0] === '>') {
      this.advanceNextNonspace();
      this.advanceOffset(1, false);
      if (isSpaceOrTab(ln[this.offset])) this.advanceOffset(1, true);
      this.closeUnmatched();
      this.addChild('block_quote');
      return 1;
    }
    // ATX heading
    const atx = !indented ? ATX_RE.exec(rest) : null;
    if (atx) {
      this.advanceNextNonspace();
      this.advanceOffset(atx[0].length, false);
      this.closeUnmatched();
      const h = this.addChild('heading');
      h.level = atx[0].trim().length;
      h.content = headingSource(ln.slice(this.offset));
      this.advanceOffset(ln.length - this.offset, false);
      return 2;
    }
    // Fenced code
    const fence = !indented ? FENCE_RE.exec(rest) : null;
    if (fence) {
      this.closeUnmatched();
      const code = this.addChild('code_block');
      code.fenced = true;
      code.fenceLength = fence[0].length;
      code.fenceChar = fence[0][0];
      code.fenceOffset = this.indent;
      this.advanceNextNonspace();
      this.advanceOffset(fence[0].length, false);
      return 2;
    }
    // `$$` math, on its own lines or in one
    if (!indented && rest.startsWith('$$')) {
      const one = MATH_ONE_LINE_RE.exec(rest);
      if (one || MATH_FENCE_RE.test(rest)) {
        this.closeUnmatched();
        const math = this.addChild('math_block');
        math.fenced = true;
        math.fenceOffset = this.indent;
        this.advanceNextNonspace();
        if (one) {
          math.content = `\n${one[1].trim()}\n`;
          this.advanceOffset(ln.length - this.offset, false);
          this.finalize(math);
        } else this.advanceOffset(2, false);
        return 2;
      }
    }
    // HTML block
    if (!indented && rest[0] === '<') {
      for (let t = 1; t <= 7; t++) {
        if (HTML_OPEN[t].test(rest) && (t < 7 || (container.type !== 'paragraph' && !(!this.allClosed && !this.blank && this.tip.type === 'paragraph')))) {
          this.closeUnmatched();
          const html = this.addChild('html_block');
          html.htmlType = t;
          return 2;
        }
      }
    }
    // GFM table: a delimiter row under a paragraph line with as many cells
    if (!indented && container.type === 'paragraph' && rest.includes('|') && DELIM_ROW_RE.test(rest)) {
      const lines = container.content.replace(/\n$/, '').split('\n');
      const header = splitRow(lines[lines.length - 1]);
      const align = splitRow(rest).map(cell => {
        const c = cell.trim();
        return c.startsWith(':') && c.endsWith(':') ? 'center' : c.endsWith(':') ? 'end' : c.startsWith(':') ? 'start' : null;
      });
      if (header.length === align.length) {
        this.closeUnmatched();
        const parent = container.parent!;
        container.content = lines.length > 1 ? `${lines.slice(0, -1).join('\n')}\n` : '';
        this.finalize(container);
        if (container.empty) parent.children.splice(parent.children.indexOf(container), 1);
        const table = newNode('table', this.lineNumber + this.lineBase - 1);
        table.parent = parent;
        table.align = align;
        table.rows = [header];
        parent.children.push(table);
        this.tip = table;
        this.advanceOffset(ln.length - this.offset, false);
        return 2;
      }
    }
    // Setext heading
    if (!indented && container.type === 'paragraph' && SETEXT_RE.test(rest)) {
      this.closeUnmatched();
      let content = container.content;
      let def: RefDef | null;
      while (content.startsWith('[') && (def = parseRefDef(content))) {
        const key = normalizeLabel(def.label);
        if (!this.refs.has(key)) this.refs.set(key, def.ref);
        content = content.slice(def.length);
      }
      if (hasContent(content)) {
        const parent = container.parent!;
        const h = newNode('heading', container.line);
        h.level = rest[0] === '=' ? 1 : 2;
        h.content = content.replace(/\n$/, '');
        h.parent = parent;
        h.open = false;
        parent.children[parent.children.indexOf(container)] = h;
        this.tip = parent;
        this.advanceOffset(ln.length - this.offset, false);
        return 2;
      }
      container.content = content;
    }
    // Thematic break
    if (!indented && THEMATIC_RE.test(rest)) {
      this.closeUnmatched();
      this.addChild('thematic_break');
      this.advanceOffset(ln.length - this.offset, false);
      return 2;
    }
    // List item
    if (!indented || container.type === 'list') {
      const data = this.listMarker(container);
      if (data) {
        this.closeUnmatched();
        const t = this.tip;
        if (t.type !== 'list' || t.list!.type !== data.type || t.list!.delimiter !== data.delimiter || t.list!.bulletChar !== data.bulletChar) {
          this.addChild('list').list = data;
        }
        this.addChild('item').list = data;
        return 1;
      }
    }
    // Indented code
    if (indented && this.tip.type !== 'paragraph' && !this.blank) {
      this.advanceOffset(CODE_INDENT, true);
      this.closeUnmatched();
      this.addChild('code_block');
      return 2;
    }
    return 0;
  }

  private listMarker(container: MdNode): ListData | null {
    if (this.indent >= 4) return null;
    const rest = this.line.slice(this.nextNonspace);
    let type: 'bullet' | 'ordered';
    let m = BULLET_RE.exec(rest);
    let bulletChar = '';
    let start = 1;
    let delimiter = '';
    if (m) {
      type = 'bullet';
      bulletChar = m[0];
    } else if ((m = ORDERED_RE.exec(rest)) && (container.type !== 'paragraph' || m[1] === '1')) {
      type = 'ordered';
      start = parseInt(m[1], 10);
      delimiter = m[2];
    } else return null;
    const next = this.line[this.nextNonspace + m[0].length];
    if (!(next === undefined || next === ' ' || next === '\t')) return null;
    if (container.type === 'paragraph' && !hasContent(this.line.slice(this.nextNonspace + m[0].length))) return null;
    const markerOffset = this.indent;
    this.advanceNextNonspace();
    this.advanceOffset(m[0].length, true);
    const startCol = this.column;
    const startOffset = this.offset;
    do {
      this.advanceOffset(1, true);
    } while (this.column - startCol < 5 && isSpaceOrTab(this.line[this.offset]));
    const blankItem = this.line[this.offset] === undefined;
    const spaces = this.column - startCol;
    let padding: number;
    if (spaces >= 5 || spaces < 1 || blankItem) {
      padding = m[0].length + 1;
      this.column = startCol;
      this.offset = startOffset;
      if (isSpaceOrTab(this.line[this.offset])) this.advanceOffset(1, true);
    } else padding = m[0].length + spaces;
    return { type, bulletChar, start, delimiter, padding, markerOffset };
  }

  private incorporate(raw: string): void {
    let container = this.doc;
    this.oldtip = this.tip;
    this.offset = 0;
    this.column = 0;
    this.blank = false;
    this.partiallyConsumedTab = false;
    this.lineNumber += 1;
    this.line = raw.replace(/\0/g, '\ufffd');
    const ln = this.line;

    for (;;) {
      const lastChild = container.children[container.children.length - 1];
      if (!lastChild || !lastChild.open) break;
      container = lastChild;
      this.findNextNonspace();
      const res = this.continues(container);
      if (res === 2) return;
      if (res === 1) {
        container = container.parent!;
        break;
      }
    }
    this.allClosed = container === this.oldtip;
    this.lastMatched = container;

    let matchedLeaf = container.type !== 'paragraph' && container.type !== 'table' && acceptsLines(container.type);
    while (!matchedLeaf) {
      this.findNextNonspace();
      if (!this.indented && !MAYBE_SPECIAL_RE.test(ln.slice(this.nextNonspace))) {
        this.advanceNextNonspace();
        break;
      }
      const res = this.starts(container);
      if (res === 0) {
        this.advanceNextNonspace();
        break;
      }
      container = this.tip;
      if (res === 2) matchedLeaf = true;
    }

    if (!this.allClosed && !this.blank && this.tip.type === 'paragraph') {
      this.addLine();
      return;
    }
    this.closeUnmatched();
    const t = container.type;
    if (t === 'table') {
      if (container.open && !this.blank && this.offset < ln.length) container.rows!.push(splitRow(ln.slice(this.offset)));
    } else if (acceptsLines(t) && container.open) {
      this.addLine();
      if (t === 'html_block' && container.htmlType! <= 5 && HTML_CLOSE[container.htmlType!].test(ln.slice(this.offset))) this.finalize(container);
    } else if (this.offset < ln.length && !this.blank) {
      this.addChild('paragraph');
      this.advanceNextNonspace();
      this.addLine();
    }
  }
}

// ============================================================================
// Markdown tree → Jinshu blocks
// ============================================================================

interface ReadContext {
  readonly format: JinshuFormat;
  readonly dialect: InlineDialect;
  readonly refs: ReadonlyMap<string, LinkReference>;
  readonly warn: (message: string, rule: FormatRule, line?: number) => void;
  readonly id: (prefix: string) => string;
}

interface Where {
  readonly inQuote?: boolean;
  readonly inItem?: boolean;
  readonly listDepth: number;
}

const ALERT_RE = /^\[!([A-Za-z]+)\][+-]?[ \t]*(.*)$/;
const TASK_RE = /^\[([ xX])\](?:[ \t]+(?=\S)|[ \t]*$)/;
const ITEM_CHILD_TYPES = new Set(['paragraph', 'list', 'code', 'image']);
const MAX_LIST_DEPTH = 6;

function inlineNodes(source: string, ctx: ReadContext, line: number, inHeading = false): (Inline | InlineImage)[] {
  return parseInlineNodes(source, {
    dialect: ctx.dialect,
    strict: true,
    inHeading,
    refs: ctx.refs,
    warn: (message, rule) => ctx.warn(message, rule, line),
  });
}

function asText(items: readonly (Inline | InlineImage)[], ctx: ReadContext, line: number, inHeading = false): Inline[] {
  const out: Inline[] = [];
  let images = false;
  for (const item of items) {
    if ('image' in item) {
      images = true;
      out.push({ text: item.source, ...(item.link ? { link: item.link } : {}) } as Inline);
    } else out.push(item);
  }
  if (images) ctx.warn('An image inside text is kept as its Markdown source.', 'inlineImage', line);
  return normalizeInlines(out, { inHeading });
}

/** A paragraph that is only images (and the spaces between them) becomes image blocks. */
function imageBlocks(items: readonly (Inline | InlineImage)[], ctx: ReadContext, line: number): JinshuBlock[] | null {
  const images = items.filter((i): i is InlineImage => 'image' in i);
  if (images.length === 0) return null;
  const onlyImages = items.every(i => 'image' in i || ('text' in i && typeof i.text === 'string' && !hasContent(i.text) && Object.keys(i).length === 1));
  if (!onlyImages || images.some(i => i.link)) return null;
  return images.map(img => {
    if (img.alt === '') ctx.warn(`Image "${img.source}" has an empty alt, read as no alt.`, 'emptyAlt', line);
    return { id: ctx.id('img'), type: 'image', src: img.src, ...(img.alt ? { alt: img.alt.replace(/\s*\n\s*/g, ' ') } : {}) } as JinshuBlock;
  });
}

function textLines(content: string): string {
  return paragraphSource(content.replace(/\n$/, '').split('\n'));
}

function lowerForItem(block: JinshuBlock, ctx: ReadContext, line: number): JinshuBlock[] {
  if (ITEM_CHILD_TYPES.has(block.type)) return [block];
  ctx.warn(`A ${block.type} inside a list item is not kept as one; ${block.type === 'divider' ? 'it is dropped' : 'its content is kept as text'}.`, 'blockInList', line);
  switch (block.type) {
    case 'heading':
      return [{ id: block.id, type: 'paragraph', runs: block.runs }];
    case 'quote':
    case 'callout':
      return block.blocks.flatMap(b => lowerForItem(b, ctx, line));
    case 'math':
      return [{ id: block.id, type: 'code', language: 'math', code: block.latex }];
    case 'table':
      return block.rows.map(row => ({
        id: ctx.id('p'),
        type: 'paragraph',
        runs: row.cells.flatMap((cell, i) => (i === 0 ? [...cell] : [{ text: ' | ' }, ...cell])),
      }));
    default:
      return [];
  }
}

function convertChildren(nodes: readonly MdNode[], ctx: ReadContext, where: Where): JinshuBlock[] {
  const out: JinshuBlock[] = [];
  for (const node of nodes) {
    const blocks = convertNode(node, ctx, where);
    for (const b of blocks) out.push(...(where.inItem ? lowerForItem(b, ctx, node.line) : [b]));
  }
  return out;
}

function convertNode(node: MdNode, ctx: ReadContext, where: Where): JinshuBlock[] {
  const line = node.line;
  switch (node.type) {
    case 'paragraph': {
      if (node.empty) return [];
      const items = inlineNodes(textLines(node.content), ctx, line);
      return imageBlocks(items, ctx, line) ?? [{ id: ctx.id('p'), type: 'paragraph', runs: asText(items, ctx, line) }];
    }
    case 'heading': {
      const runs = asText(inlineNodes(node.content.split('\n').map(trimMd).join('\n'), ctx, line, true), ctx, line, true);
      if (where.inQuote) {
        ctx.warn('A heading inside a quote is read as a paragraph.', 'headingInQuote', line);
        return [{ id: ctx.id('p'), type: 'paragraph', runs }];
      }
      let level = node.level!;
      if (level > 4) {
        ctx.warn(`A level-${level} heading is read as level 4.`, 'headingLevel', line);
        level = 4;
      }
      return [{ id: ctx.id('h'), type: 'heading', level: level as 1 | 2 | 3 | 4, runs }];
    }
    case 'thematic_break':
      return [{ id: ctx.id('hr'), type: 'divider' }];
    case 'code_block': {
      const code = node.content.endsWith('\n') ? node.content.slice(0, -1) : node.content;
      const words = (node.info ?? '').split(/[ \t]+/).filter(Boolean);
      if (words.length > 1) ctx.warn(`Code block info "${node.info}": only the language "${words[0]}" is kept.`, 'codeInfo', line);
      const language = words[0] ?? 'text';
      if (language === 'math') return [{ id: ctx.id('m'), type: 'math', latex: code }];
      return [{ id: ctx.id('code'), type: 'code', language, code }];
    }
    case 'math_block': {
      const latex = node.content.endsWith('\n') ? node.content.slice(0, -1) : node.content;
      return [{ id: ctx.id('m'), type: 'math', latex }];
    }
    case 'html_block': {
      ctx.warn('Raw HTML is kept as text.', 'rawHtml', line);
      const lines = node.content.replace(/\n$/, '').split('\n');
      const runs: Inline[] = [];
      lines.forEach((l, i) => {
        if (i > 0) runs.push({ break: true });
        if (l) runs.push({ text: l });
      });
      return [{ id: ctx.id('p'), type: 'paragraph', runs: normalizeInlines(runs) }];
    }
    case 'block_quote':
      return [convertQuote(node, ctx, where)];
    case 'list':
      return convertList(node, ctx, where);
    case 'table':
      return [convertTable(node, ctx)];
    default:
      return convertChildren(node.children, ctx, where);
  }
}

function convertQuote(node: MdNode, ctx: ReadContext, where: Where): JinshuBlock {
  const first = node.children[0];
  const alert = first?.type === 'paragraph' && !first.empty ? ALERT_RE.exec(trimMd(first.content.split('\n')[0])) : null;
  if (!alert) {
    const blocks = convertChildren(node.children, ctx, { ...where, inQuote: true });
    return { id: ctx.id('q'), type: 'quote', blocks: blocks.length ? blocks : [paragraph([], ctx.id('p'))] };
  }
  const kind = alert[1].toUpperCase();
  const tone = (Object.keys(CALLOUT_ALERTS) as CalloutTone[]).find(t => CALLOUT_ALERTS[t] === kind);
  if (!tone) ctx.warn(`Alert type "${alert[1]}" has no callout tone; read as a note.`, 'alertType', first.line);
  const rest = first.content.slice(first.content.indexOf('\n') + 1);
  const body: JinshuBlock[] = [];
  let title: Inline[] | undefined;
  if (hasContent(alert[2])) {
    const runs = asText(inlineNodes(trimMd(alert[2]), ctx, first.line), ctx, first.line);
    if (ctx.format === 'json') title = runs;
    else {
      ctx.warn('A callout title is kept as the callout\'s first paragraph.', 'calloutTitle', first.line);
      body.push({ id: ctx.id('p'), type: 'paragraph', runs });
    }
  }
  if (first.content.includes('\n') && hasContent(rest)) body.push(...convertNode({ ...first, content: rest }, ctx, { ...where, inQuote: false }));
  body.push(...convertChildren(node.children.slice(1), ctx, { ...where, inQuote: false }));
  return {
    id: ctx.id('c'),
    type: 'callout',
    tone: tone ?? 'info',
    ...(title ? { title } : {}),
    blocks: body.length ? body : [paragraph([], ctx.id('p'))],
  };
}

function convertList(node: MdNode, ctx: ReadContext, where: Where): JinshuBlock[] {
  const depth = where.listDepth + 1;
  const data = node.list!;
  const firstParas = node.children.map(item => (item.children[0]?.type === 'paragraph' && !item.children[0].empty ? item.children[0] : undefined));
  // A task list only when every item has a box; otherwise a leading `[x] ` is the item's text.
  const task = data.type === 'bullet' && firstParas.every(p => p !== undefined && TASK_RE.test(textLines(p.content)));
  if (depth > MAX_LIST_DEPTH) {
    ctx.warn(`Lists nest at most ${MAX_LIST_DEPTH} deep; a deeper list is read as paragraphs.`, 'listDepth', node.line);
    return node.children.flatMap(item => convertChildren(item.children, ctx, { ...where, inItem: false }));
  }
  const items = node.children.map((item, i) => {
    let children = item.children;
    let runs: Inline[] = [];
    let checked: boolean | undefined;
    const para = firstParas[i];
    if (para) {
      let content = textLines(para.content);
      if (task) {
        const m = TASK_RE.exec(content)!;
        checked = m[1] !== ' ';
        content = content.slice(m[0].length);
      }
      const nodes = inlineNodes(content, ctx, para.line);
      const images = imageBlocks(nodes, ctx, para.line);
      children = children.slice(1);
      if (images) return { item, runs, checked, blocks: [...images, ...convertChildren(children, ctx, { listDepth: depth, inItem: true })] };
      runs = asText(nodes, ctx, para.line);
    }
    return { item, runs, checked, blocks: convertChildren(children, ctx, { listDepth: depth, inItem: true }) };
  });
  const list: ListBlock = {
    id: ctx.id('l'),
    type: 'list',
    style: data.type === 'ordered' ? 'ordered' : task ? 'task' : 'bullet',
    ...(data.type === 'ordered' ? { start: data.start } : {}),
    items: items.map(it => ({
      id: ctx.id('li'),
      ...(task ? { checked: it.checked === true } : {}),
      runs: it.runs,
      ...(it.blocks.length ? { blocks: it.blocks } : {}),
    })),
  };
  return [list];
}

function convertTable(node: MdNode, ctx: ReadContext): JinshuBlock {
  const align = node.align!;
  const cols = align.length;
  let extra = false;
  const rows = node.rows!.map(raw => {
    if (raw.length > cols) extra = true;
    const cells = Array.from({ length: cols }, (_, i) => asText(inlineNodes(cellSource(raw[i] ?? ''), ctx, node.line), ctx, node.line));
    return { id: ctx.id('r'), cells };
  });
  if (extra) ctx.warn('Table cells beyond the header\'s columns are dropped.', 'tableCells', node.line);
  const aligned = align.some(a => a !== null);
  return {
    id: ctx.id('t'),
    type: 'table',
    ...(aligned ? { align: align.map(a => a ?? 'start') } : {}),
    headerRow: true,
    rows,
  };
}

// ============================================================================
// Ids
// ============================================================================

type Rec = Record<string, unknown>;

const ID_PREFIX: Readonly<Record<string, string>> = {
  paragraph: 'p', heading: 'h', quote: 'q', callout: 'c', list: 'l', code: 'code', math: 'm', table: 't',
  divider: 'hr', image: 'img', item: 'li', row: 'r',
};

function stripIds(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stripIds);
  if (value !== null && typeof value === 'object') {
    const out: Rec = {};
    for (const [k, v] of Object.entries(value as Rec)) if (k !== 'id') out[k] = stripIds(v);
    return out;
  }
  return value;
}

const keyOf = (node: unknown) => JSON.stringify(stripIds(node));
const typeOf = (node: Rec) => (typeof node.type === 'string' ? node.type : 'item');

/** Pairs nodes of two sibling lists: equal content in order, then same-type nodes between those anchors. */
function matchSiblings(next: readonly Rec[], prev: readonly Rec[]): [number, number][] {
  const a = next.map(keyOf);
  const b = prev.map(keyOf);
  const n = a.length;
  const m = b.length;
  const anchors: [number, number][] = [];
  if (n * m <= 4_000_000) {
    const w = m + 1;
    const dp = new Uint16Array((n + 1) * w);
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        dp[i * w + j] = a[i] === b[j] ? dp[(i + 1) * w + j + 1] + 1 : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);
      }
    }
    for (let i = 0, j = 0; i < n && j < m; ) {
      if (a[i] === b[j]) {
        anchors.push([i, j]);
        i++;
        j++;
      } else if (dp[(i + 1) * w + j] >= dp[i * w + j + 1]) i++;
      else j++;
    }
  } else {
    const positions = new Map<string, number[]>();
    b.forEach((k, j) => positions.set(k, [...(positions.get(k) ?? []), j]));
    let floor = -1;
    a.forEach((k, i) => {
      const j = positions.get(k)?.find(p => p > floor);
      if (j !== undefined) {
        anchors.push([i, j]);
        floor = j;
      }
    });
  }
  const pairs: [number, number][] = [];
  let pi = 0;
  let pj = 0;
  for (const [ai, aj] of [...anchors, [n, m] as [number, number]]) {
    let j = pj;
    for (let i = pi; i < ai; i++) {
      for (let k = j; k < aj; k++) {
        if (typeOf(next[i]) === typeOf(prev[k])) {
          pairs.push([i, k]);
          j = k + 1;
          break;
        }
      }
    }
    if (ai < n) pairs.push([ai, aj]);
    pi = ai + 1;
    pj = aj + 1;
  }
  return pairs;
}

/** Gives every block, item and row a final id: the previous one where matched, else one derived from content. */
function assignIds(blocks: Rec[], previous: readonly JinshuBlock[] | undefined): void {
  const taken = new Set<string>();
  const reuse = (node: Rec, old: Rec) => {
    if (typeof old.id === 'string' && !taken.has(old.id)) {
      node.id = old.id;
      taken.add(old.id);
      return true;
    }
    return false;
  };
  const matched = new Set<Rec>();
  const walkMatch = (next: Rec[], prev: readonly Rec[]) => {
    for (const [i, j] of matchSiblings(next, prev)) {
      const node = next[i];
      const old = prev[j];
      if (!reuse(node, old)) continue;
      matched.add(node);
      if (Array.isArray(node.blocks) && Array.isArray(old.blocks)) walkMatch(node.blocks as Rec[], old.blocks as Rec[]);
      if (Array.isArray(node.items) && Array.isArray(old.items)) walkMatch(node.items as Rec[], old.items as Rec[]);
      if (Array.isArray(node.rows) && Array.isArray(old.rows)) walkMatch(node.rows as Rec[], old.rows as Rec[]);
    }
  };
  if (previous) walkMatch(blocks, previous as unknown as Rec[]);
  const generate = (list: Rec[], kind?: 'item' | 'row') => {
    for (const node of list) {
      if (!matched.has(node)) {
        const type = kind ?? String(node.type);
        const base = `${ID_PREFIX[type] ?? 'b'}-${hashString(type + keyOf(node))}`;
        let id = base;
        for (let n = 2; taken.has(id); n++) id = `${base}-${n}`;
        node.id = id;
        taken.add(id);
      }
      if (Array.isArray(node.blocks)) generate(node.blocks as Rec[]);
      if (Array.isArray(node.items)) generate(node.items as Rec[], 'item');
      if (Array.isArray(node.rows)) generate(node.rows as Rec[], 'row');
    }
  };
  generate(blocks);
}

// ============================================================================
// Reading
// ============================================================================

const FRONT_MATTER_RE = /^---[ \t]*\n(?:(?:---|\.\.\.)[ \t]*(?:\n|$)|([\s\S]*?)\n(?:---|\.\.\.)[ \t]*(?:\n|$))/;

/** Lines (1-based, inclusive) a top-level part of the file came from, and how many blocks it made. */
interface Span {
  readonly kind: 'front' | 'title' | 'blocks';
  readonly first: number;
  readonly last: number;
  readonly count: number;
}

interface Parsed {
  blocks: JinshuBlock[];
  title?: string;
  frontMatter?: string;
  problems: Problem[];
  spans: Span[];
}

function parseMarkdown(
  input: string,
  format: JinshuFormat,
  options: { readonly whole: boolean; readonly previous?: JinshuDocument },
): Parsed {
  let text = String(input ?? '').replace(/^\ufeff/, '').replace(/\r\n?/g, '\n');
  const problems: Problem[] = [];
  let lineBase = 0;
  let frontMatter: string | undefined;
  if (options.whole) {
    // Front matter is kept verbatim in meta.frontMatter, so saving writes it back unchanged.
    const fm = FRONT_MATTER_RE.exec(text);
    if (fm) {
      frontMatter = fm[1] ?? '';
      lineBase = fm[0].split('\n').length - (fm[0].endsWith('\n') ? 1 : 0);
      text = text.slice(fm[0].length);
    }
  }
  const parser = new BlockParser(lineBase);
  const tree = parser.parse(text);
  let counter = 0;
  const ctx: ReadContext = {
    format,
    dialect: format === 'markdown' ? 'markdown' : 'jinshu',
    refs: parser.refs,
    warn: (message, rule, line) => problems.push({ code: 'FORMAT', severity: 'warning', message: line ? `Line ${line}: ${message}` : message, subject: { rule } }),
    id: prefix => `${prefix}-tmp${counter++}`,
  };
  const lines = text.split('\n');
  const lineCount = lineBase + lines.length - (text.endsWith('\n') || text === '' ? 1 : 0);
  const blankLine = (n: number) => !hasContent(lines[n - lineBase - 1] ?? '');
  const spans: Span[] = lineBase > 0 ? [{ kind: 'front', first: 1, last: lineBase, count: 0 }] : [];
  const spanOf = (i: number): [number, number] => {
    const first = tree.children[i].line;
    let last = i + 1 < tree.children.length ? tree.children[i + 1].line - 1 : lineCount;
    while (last > first && blankLine(last)) last--;
    return [first, last];
  };
  const nodes = tree.children.filter(n => !(n.type === 'paragraph' && n.empty));
  let title: string | undefined;
  const head = nodes[0];
  if (options.whole && head?.type === 'heading' && head.level === 1) {
    const runs = asText(inlineNodes(head.content, { ...ctx, warn: () => {} }, head.line), ctx, head.line);
    const plain = runs.length === 1 && Object.keys(runs[0]).length === 1 && typeof (runs[0] as InlineRun).text === 'string';
    const prev = options.previous;
    // A leading plain `# X` is the title, unless the previous document had a level-1 heading there instead.
    const keptHeading = prev !== undefined && prev.title === undefined && prev.blocks?.[0]?.type === 'heading' && (prev.blocks[0] as { level?: number }).level === 1;
    if (plain && !keptHeading) title = (runs[0] as InlineRun).text;
  }
  const blocks: JinshuBlock[] = [];
  tree.children.forEach((node, i) => {
    const [first, last] = spanOf(i);
    if (title !== undefined && node === head) {
      spans.push({ kind: 'title', first, last, count: 0 });
      return;
    }
    const made = node.type === 'paragraph' && node.empty ? [] : convertNode(node, ctx, { listDepth: 0 });
    blocks.push(...made);
    spans.push({ kind: 'blocks', first, last, count: made.length });
  });
  return { blocks, title, frontMatter, problems, spans };
}

/**
 * Parses Markdown into a document. `problems` are `FORMAT` warnings for anything that could not be kept
 * exactly, each saying what happened. Ids come from `previous` where a block is unchanged or stands in
 * the same place, and are otherwise derived from content. `source` lets `documentToMarkdown` keep the
 * bytes of unchanged parts. Never throws.
 */
export function markdownToDocument(
  text: string,
  options?: { readonly registry?: JinshuRegistry; readonly previous?: JinshuDocument },
): { doc: JinshuDocument; problems: Problem[]; source: MarkdownSource } {
  const registry = options?.registry ?? emptyRegistry;
  try {
    const previous = options?.previous ? normalizeDocument(options.previous, registry) : undefined;
    const parsed = parseMarkdown(text, 'markdown', { whole: true, previous });
    const blocks = parsed.blocks.length ? parsed.blocks : [paragraph([], 'p-tmp')];
    const doc = normalizeDocument(
      {
        version: JINSHU_DOC_VERSION,
        ...(parsed.title !== undefined ? { title: parsed.title } : {}),
        ...(parsed.frontMatter !== undefined ? { meta: { frontMatter: parsed.frontMatter } } : {}),
        blocks,
      },
      registry,
    );
    assignIds(doc.blocks as unknown as Rec[], previous?.blocks);
    const problems = [...parsed.problems];
    for (const p of checkDocument(doc, registry, { format: 'markdown' })) if (p.severity === 'error') problems.push(p);
    return { doc, problems, source: buildSource(String(text ?? ''), parsed, doc) };
  } catch (err) {
    return {
      doc: normalizeDocument({ version: JINSHU_DOC_VERSION, blocks: [paragraph([], 'p-0')] }, registry),
      problems: [{ code: 'FORMAT', severity: 'error', message: `The Markdown could not be read: ${err instanceof Error ? err.message : String(err)}`, subject: { rule: 'unreadable' } }],
      source: { text: String(text ?? ''), segments: [] },
    };
  }
}

/** Parses pasted Markdown into blocks with fresh random ids. `json` keeps the full Jinshu inline shorthand; `markdown` the plain one. */
export function markdownToBlocks(text: string, options?: { readonly format?: JinshuFormat }): JinshuBlock[] {
  try {
    const parsed = parseMarkdown(text, options?.format ?? 'json', { whole: false });
    if (parsed.blocks.length === 0) return [];
    const doc = normalizeDocument({ version: JINSHU_DOC_VERSION, blocks: parsed.blocks });
    const taken = new Set<string>();
    const renew = (list: Rec[], kind?: string) => {
      for (const node of list) {
        const prefix = ID_PREFIX[kind ?? String(node.type)] ?? 'b';
        let id = newBlockId(prefix);
        while (taken.has(id)) id = newBlockId(prefix);
        taken.add(id);
        node.id = id;
        if (Array.isArray(node.blocks)) renew(node.blocks as Rec[]);
        if (Array.isArray(node.items)) renew(node.items as Rec[], 'item');
        if (Array.isArray(node.rows)) renew(node.rows as Rec[], 'row');
      }
    };
    renew(doc.blocks as unknown as Rec[]);
    return [...doc.blocks];
  } catch {
    return [paragraph(text)];
  }
}

// ============================================================================
// Writing
// ============================================================================

interface WriteState {
  exact: boolean;
  readonly inexact: Set<string>;
}

function inline(runs: readonly Inline[], context: InlineContext, state: WriteState, id: string): string {
  const res = writeInlines(runs, { dialect: 'markdown', context });
  if (!res.exact) state.inexact.add(id);
  return res.text;
}

const indentLines = (lines: readonly string[], pad: string) => lines.map(l => (l === '' ? '' : pad + l));
const quoteLines = (lines: readonly string[]) => lines.map(l => (l === '' ? '>' : `> ${l}`));

function fenceFor(code: string, min = 3): string {
  const longest = Math.max(0, ...(code.match(/`+/g) ?? []).map(r => r.length));
  return '`'.repeat(Math.max(min, longest + 1));
}

type Family = 'bullet' | 'ordered';
const familyOf = (b: JinshuBlock): Family | null => (b.type === 'list' ? (b.style === 'ordered' ? 'ordered' : 'bullet') : null);

/** An empty paragraph says nothing in Markdown, so it is not written. */
const isBlank = (block: JinshuBlock) => block.type === 'paragraph' && writeInlines(block.runs, { dialect: 'markdown', context: 'block' }).text === '';

function writeBlocks(blocks: readonly JinshuBlock[], state: WriteState, top = false): string[] {
  const out: string[] = [];
  let prevFamily: Family | null = null;
  let alternate = false;
  for (const block of blocks) {
    if (isBlank(block)) continue;
    const family = familyOf(block);
    // Two lists in a row would merge; a different marker keeps them apart.
    alternate = family !== null && family === prevFamily ? !alternate : false;
    prevFamily = family;
    // A leading `---` would open front matter.
    const lines = top && out.length === 0 && block.type === 'divider' ? ['***'] : writeBlock(block, state, alternate);
    if (out.length) out.push('');
    out.push(...lines);
  }
  return out;
}

function writeBlock(block: JinshuBlock, state: WriteState, alternate: boolean): string[] {
  switch (block.type) {
    case 'paragraph': {
      const text = inline(block.runs, 'block', state, block.id);
      return text === '' ? [] : text.split('\n');
    }
    case 'heading': {
      const content = inline(block.runs, 'heading', state, block.id);
      return [`${'#'.repeat(block.level)}${content ? ` ${content}` : ''}`];
    }
    case 'quote': {
      const lines = writeBlocks(block.blocks, state);
      return lines.length ? quoteLines(lines) : ['>'];
    }
    case 'callout': {
      const body = block.title?.length ? [{ id: block.id, type: 'paragraph', runs: block.title.map(r => ('text' in r ? { ...r, bold: true } : r)) } as JinshuBlock, ...block.blocks] : block.blocks;
      // A paragraph may follow the alert line directly; anything else needs a blank line to start.
      const lead = body.find(b => !isBlank(b));
      const gap = lead === undefined || lead.type === 'paragraph' ? [] : [''];
      return quoteLines([`[!${CALLOUT_ALERTS[block.tone] ?? 'NOTE'}]`, ...gap, ...writeBlocks(body, state)]);
    }
    case 'list':
      return writeList(block, state, alternate);
    case 'code': {
      const fence = fenceFor(block.code);
      const language = /^[^\s`]+$/.test(block.language) ? block.language : (block.language.match(/[^\s`]+/)?.[0] ?? 'text');
      return [`${fence}${language === 'text' ? '' : language}`, ...(block.code === '' ? [] : block.code.split('\n')), fence];
    }
    case 'math': {
      const lines = block.latex === '' ? [] : block.latex.split('\n');
      if (lines.some(l => MATH_FENCE_RE.test(l.trim()))) {
        const fence = fenceFor(block.latex);
        return [`${fence}math`, ...lines, fence];
      }
      return ['$$', ...lines, '$$'];
    }
    case 'table': {
      const cols = block.rows[0]?.cells.length ?? 0;
      const row = (cells: readonly (readonly Inline[])[]) => `| ${cells.map(c => inline(c, 'cell', state, block.id)).join(' | ')} |`;
      const delim = Array.from({ length: cols }, (_, i) => {
        const a = block.align?.[i];
        return a === 'start' ? ':--' : a === 'center' ? ':-:' : a === 'end' ? '--:' : '---';
      });
      const [head, ...body] = block.rows;
      return head ? [row(head.cells), `| ${delim.join(' | ')} |`, ...body.map(r => row(r.cells))] : [];
    }
    case 'divider':
      return ['---'];
    case 'image': {
      const alt = block.alt ? inline([{ text: block.alt.replace(/[\r\n]+/g, ' ') }], 'label', state, block.id) : '';
      const lines = [`![${alt}](${writeDestination(block.src.replace(/[\r\n]+/g, ''))})`];
      const caption = block.caption?.length ? inline(block.caption, 'block', state, block.id) : '';
      if (caption) lines.push('', ...caption.split('\n'));
      return lines;
    }
    case 'details': {
      const summary: JinshuBlock = { id: block.id, type: 'paragraph', runs: block.summary.map(r => ('text' in r ? { ...r, bold: true } : r)) };
      return writeBlocks([summary, ...block.blocks], state);
    }
    case 'columns':
      return writeBlocks(block.columns.flatMap(c => c.blocks), state);
    case 'video':
    case 'audio':
    case 'file': {
      const label = block.type === 'file' ? block.name : block.type === 'audio' ? (block.title ?? block.src) : block.caption?.length ? undefined : block.src;
      const runs: readonly Inline[] = label !== undefined ? [{ text: label }] : (block as { caption: readonly Inline[] }).caption;
      return writeLink(runs, block.src, state, block.id);
    }
    default: {
      const ext = block as { id: string; runs?: readonly Inline[]; blocks?: readonly JinshuBlock[]; fallback?: string };
      if (ext.blocks?.length) return writeBlocks(ext.blocks, state);
      if (ext.runs?.length) return inline(ext.runs, 'block', state, ext.id).split('\n');
      return inline([{ text: ext.fallback ?? '' }], 'block', state, ext.id).split('\n');
    }
  }
}

function writeLink(runs: readonly Inline[], src: string, state: WriteState, id: string): string[] {
  const text = inline(runs.map(r => ('text' in r ? { ...r, link: undefined } : r)), 'label', state, id);
  return [`[${text || writeDestination(src)}](${writeDestination(src)})`];
}

const BOX_RE = /^\\\[([ xX])\](?=[ \t]|$)/;

function writeList(block: ListBlock, state: WriteState, alternate: boolean): string[] {
  const out: string[] = [];
  const start = block.start ?? 1;
  const texts = block.items.map(item => inline(item.runs, 'block', state, item.id));
  // A leading `[x] ` stays literal, unescaped, unless every bullet item has one and the list would read as tasks.
  const keepEscapes = block.style === 'bullet' && texts.every(t => BOX_RE.test(t));
  block.items.forEach((item, i) => {
    const marker = block.style === 'ordered' ? `${start + i}${alternate ? ')' : '.'}` : alternate ? '*' : '-';
    const pad = ' '.repeat(marker.length + 1);
    let text = block.style === 'task' || keepEscapes ? texts[i] : texts[i].replace(BOX_RE, '[$1]');
    if (block.style === 'task') text = `[${item.checked ? 'x' : ' '}]${text ? ` ${text}` : ''}`;
    const lines = text.split('\n');
    out.push(lines[0] ? `${marker} ${lines[0]}` : marker, ...indentLines(lines.slice(1), pad));
    const children = (item.blocks ?? []).map(child => ({ child, lines: writeBlock(child, state, false) })).filter(c => c.lines.length);
    children.forEach(({ child, lines: childLines }, c) => {
      // An empty item's content starts on the next line; a nested list may follow text when it can interrupt a paragraph.
      const tight =
        c === 0 &&
        (text === '' ||
          (child.type === 'list' && (child.style !== 'ordered' || (child.start ?? 1) === 1) && /^(?:[-*]|\d+[.)]) \S/.test(childLines[0] ?? '')));
      if (!tight) out.push('');
      out.push(...indentLines(childLines, pad));
    });
  });
  return out;
}

type Part = { readonly kind: 'front' | 'title' | 'blocks'; readonly lines: readonly string[] };

const frontMatterOf = (d: JinshuDocument): string | undefined => {
  const fm = (d.meta as { frontMatter?: unknown } | undefined)?.frontMatter;
  return typeof fm === 'string' ? fm : undefined;
};
const frontLines = (fm: string) => (fm === '' ? ['---', '---'] : ['---', ...fm.split('\n'), '---']);
const titleLine = (title: string | undefined, state: WriteState): string[] => {
  if (typeof title !== 'string') return [];
  const text = inline([{ text: title.replace(/[\r\n]+/g, ' ') }], 'heading', state, 'title');
  return text === '' ? [] : [`# ${text}`];
};

function canonicalText(d: JinshuDocument, state: WriteState): string {
  const parts: string[] = [];
  const fm = frontMatterOf(d);
  if (fm !== undefined) parts.push(frontLines(fm).join('\n'));
  const title = titleLine(d.title, state);
  if (title.length) parts.push(title[0]);
  const lines = writeBlocks(d.blocks, state, parts.length === 0);
  if (lines.length) parts.push(lines.join('\n'));
  return parts.length ? `${parts.join('\n\n')}\n` : '';
}

// ============================================================================
// Minimal rewrite: unchanged parts keep their original bytes
// ============================================================================

interface SourceSegment {
  readonly kind: 'front' | 'title' | 'blocks';
  /** Offsets of the part's text in the original file, without its last line's newline. */
  readonly start: number;
  readonly end: number;
  /** Ids of the top-level blocks it made, in order. */
  readonly ids: readonly string[];
  /** Hash of its content when loaded. */
  readonly key: string;
}

/**
 * What `markdownToDocument` read: the original text and where each top-level part came from. Opaque and
 * JSON-safe; keep it with the document and pass it to `documentToMarkdown` so unchanged parts keep their bytes.
 */
export interface MarkdownSource {
  readonly text: string;
  readonly segments: readonly SourceSegment[];
}

const contentKey = (value: unknown) => hashString(JSON.stringify(stripIds(value)));

function buildSource(raw: string, parsed: Parsed, doc: JinshuDocument): MarkdownSource {
  const bom = raw.startsWith('﻿') ? 1 : 0;
  const starts = [0, bom];
  const ends = [0];
  for (const m of raw.slice(bom).matchAll(/\r\n|\r|\n/g)) {
    ends.push(bom + m.index!);
    starts.push(bom + m.index! + m[0].length);
  }
  ends.push(raw.length);
  const blocksMatch = doc.blocks.length === parsed.blocks.length;
  const segments: SourceSegment[] = [];
  let at = 0;
  for (const span of parsed.spans) {
    const ids = blocksMatch ? doc.blocks.slice(at, at + span.count).map(b => b.id) : [];
    const value = span.kind === 'front' ? frontMatterOf(doc) : span.kind === 'title' ? doc.title : doc.blocks.slice(at, at + span.count);
    at += span.count;
    if (span.count > 0 && !blocksMatch) continue;
    segments.push({ kind: span.kind, start: starts[span.first], end: ends[span.last], ids, key: contentKey(value) });
  }
  return { text: raw, segments };
}

type Piece = { readonly seg: number } | { readonly seg?: undefined; readonly lines: readonly string[]; readonly expect: Parsed };

/** The comparable meaning of a document: what a reader of the file gets, ids aside. */
function meaning(title: string | undefined, frontMatter: string | undefined, blocks: readonly JinshuBlock[]): string {
  const doc = normalizeDocument({ version: JINSHU_DOC_VERSION, ...(title !== undefined ? { title } : {}), blocks: [...blocks] });
  return JSON.stringify(stripIds([doc.title ?? null, frontMatter ?? null, doc.blocks.filter(b => !isBlank(b))]));
}

/** Writes `d` keeping the original bytes of every part that did not change; null when that cannot be done safely. */
function writeWithSource(d: JinshuDocument, source: MarkdownSource): string | null {
  const raw = source.text;
  const segs = source.segments;
  const eol = raw.includes('\r\n') ? '\r\n' : '\n';
  const bom = raw.startsWith('﻿') ? '﻿' : '';
  const scratch: WriteState = { exact: true, inexact: new Set() };
  const banned = new Set<number>();
  const fm = frontMatterOf(d);

  for (let attempt = 0; attempt < 4; attempt++) {
    const pieces: Piece[] = [];
    const used = new Set<number>();
    const keep = (kind: SourceSegment['kind'], key: string) => {
      const i = segs.findIndex((sg, n) => sg.kind === kind && !banned.has(n) && !used.has(n) && sg.key === key);
      if (i >= 0) used.add(i);
      return i;
    };
    const canonical = (lines: readonly string[], kind: 'front' | 'title' | 'blocks') => {
      if (lines.length === 0) return;
      const expect = kind === 'blocks' ? parseMarkdown(lines.join('\n'), 'markdown', { whole: false }) : undefined;
      pieces.push({ lines, expect: expect ?? { blocks: [], problems: [], spans: [] } });
    };
    if (fm !== undefined) {
      const i = keep('front', contentKey(fm));
      if (i >= 0) pieces.push({ seg: i });
      else canonical(frontLines(fm), 'front');
    }
    if (d.title !== undefined) {
      const i = keep('title', contentKey(d.title));
      if (i >= 0) pieces.push({ seg: i });
      else canonical(titleLine(d.title, scratch), 'title');
    }
    const byFirstId = new Map<string, number>();
    segs.forEach((sg, n) => {
      if (sg.kind === 'blocks' && sg.ids.length && !banned.has(n)) byFirstId.set(sg.ids[0], n);
    });
    let run: JinshuBlock[] = [];
    const flush = () => {
      if (run.length) canonical(writeBlocks(run, scratch, pieces.length === 0), 'blocks');
      run = [];
    };
    for (let k = 0; k < d.blocks.length; ) {
      const n = byFirstId.get(d.blocks[k].id);
      const sg = n !== undefined ? segs[n] : undefined;
      const slice = sg ? d.blocks.slice(k, k + sg.ids.length) : [];
      if (sg && !used.has(n!) && slice.length === sg.ids.length && slice.every((b, j) => b.id === sg.ids[j]) && contentKey(slice) === sg.key) {
        flush();
        used.add(n!);
        pieces.push({ seg: n! });
        k += slice.length;
      } else {
        run.push(d.blocks[k]);
        k += 1;
      }
    }
    flush();
    // Parts that made no block (link reference definitions) stay after the part they followed.
    segs.forEach((sg, n) => {
      if (sg.kind !== 'blocks' || sg.ids.length || banned.has(n)) return;
      let at = 0;
      pieces.forEach((p, j) => {
        if (p.seg !== undefined && p.seg < n) at = j + 1;
      });
      pieces.splice(at, 0, { seg: n });
    });

    if (pieces.length === 0) return hasContent(raw.replace(/^\ufeff/, '')) ? '' : raw;
    let out = bom;
    pieces.forEach((p, i) => {
      const prev = pieces[i - 1];
      if (i === 0) out += p.seg === 0 ? raw.slice(bom.length, segs[0].start) : '';
      else if (prev.seg !== undefined && p.seg === prev.seg + 1) out += raw.slice(segs[prev.seg].end, segs[p.seg].start);
      else out += eol + eol;
      out += p.seg !== undefined ? raw.slice(segs[p.seg].start, segs[p.seg].end) : p.lines.join(eol);
    });
    const lastPiece = pieces[pieces.length - 1];
    out += lastPiece.seg === segs.length - 1 ? raw.slice(segs[lastPiece.seg].end) : eol;

    // Check the file reads as intended; where a rewritten part disturbs its neighbours, rewrite those too.
    const expected: JinshuBlock[] = [];
    for (const p of pieces) {
      if (p.seg === undefined) expected.push(...p.expect.blocks);
      else if (segs[p.seg].kind === 'blocks') {
        const first = d.blocks.findIndex(b => b.id === segs[p.seg!].ids[0]);
        if (first >= 0) expected.push(...d.blocks.slice(first, first + segs[p.seg].ids.length));
      }
    }
    const titleText = d.title === undefined ? undefined : parseMarkdown(titleLine(d.title, scratch).join('\n'), 'markdown', { whole: true }).title;
    const keptTitle = pieces.some(p => p.seg !== undefined && segs[p.seg].kind === 'title');
    const got = parseMarkdown(out, 'markdown', { whole: true, previous: d });
    if (meaning(got.title, got.frontMatter, got.blocks) === meaning(keptTitle ? d.title : titleText, fm, expected)) return out;
    let more = false;
    pieces.forEach((p, i) => {
      if (p.seg !== undefined) return;
      for (const q of [pieces[i - 1], pieces[i + 1]]) {
        if (q?.seg !== undefined && !banned.has(q.seg)) {
          banned.add(q.seg);
          more = true;
        }
      }
    });
    if (!more) return null;
  }
  return null;
}

/**
 * Writes a document as Markdown. `problems` are `FORMAT` errors, one per thing a Markdown file cannot
 * hold; each is written as the nearest Markdown (a details as its bold summary and body, a video as a
 * link, underline dropped…), so an export can list what was lost. With the `source` the file was read
 * from, every part that did not change keeps its original bytes.
 */
export function documentToMarkdown(doc: JinshuDocument, options?: { readonly source?: MarkdownSource }): { text: string; problems: Problem[] } {
  const problems = checkMarkdownProfile(doc);
  const d = normalizeDocument(doc);
  const state: WriteState = { exact: true, inexact: new Set() };
  const canonical = canonicalText(d, state);
  let text = canonical;
  if (options?.source) {
    try {
      text = writeWithSource(d, options.source) ?? canonical;
    } catch {
      text = canonical;
    }
  }
  for (const id of state.inexact) {
    if (problems.some(p => p.id === id)) continue;
    problems.push({ code: 'FORMAT', severity: 'error', ...(id !== 'title' ? { id } : {}), message: 'Its inline formatting could not be written exactly in Markdown.', subject: { rule: 'inlineInexact' } });
  }
  return { text, problems };
}
