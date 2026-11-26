import {
  BLOCK_FIELDS,
  CHAPTER_FIELDS,
  DOC_FIELDS,
  EXTENSION_BLOCK_FIELDS,
  INLINE_FIELDS,
  LIST_ITEM_FIELDS,
  SUBTITLE_FIELDS,
  TABLE_ROW_FIELDS,
  attrFields,
  isBuiltinBlockType,
  type FieldSpec,
} from './fields';
import { extractOutline, inlineKind, newBlockId, plainText } from './helpers';
import { emptyRegistry } from './registry';
import { isRecord, normalizeInlines, validateLink } from './validate';
import type {
  ExtensionBlock,
  HeadingLevel,
  Inline,
  InlineAtom,
  InlineRun,
  JinshuBlock,
  JinshuDocument,
  JinshuRegistry,
  Problem,
  CheckOptions,
} from './types';

type Rec = Record<string, unknown>;

function escapeText(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function escapeAttr(text: string): string {
  return escapeText(text).replace(/"/g, '&quot;');
}

function unescapeXml(text: string): string {
  return text
    .replace(/&quot;/g, '"')
    .replace(/&gt;/g, '>')
    .replace(/&lt;/g, '<')
    .replace(/&amp;/g, '&');
}

function longestBacktickRun(code: string): number {
  let max = 0;
  let cur = 0;
  for (let i = 0; i < code.length; i++) {
    if (code.charCodeAt(i) === 96) {
      cur += 1;
      if (cur > max) max = cur;
    } else {
      cur = 0;
    }
  }
  return max;
}

// ============================================================================
// Field attributes: same name and unit as the JSON field
// ============================================================================

function formatAttrValue(value: unknown): string {
  if (Array.isArray(value)) return value.map(v => String(v)).join(',');
  return String(value);
}

/** `name="value"` for every attribute-typed field present, in field-table order. */
function fieldAttrs(raw: Rec, fields: readonly FieldSpec[]): string {
  let out = '';
  for (const f of attrFields(fields)) {
    const v = raw[f.name];
    if (v === undefined) continue;
    out += ` ${f.name}="${escapeAttr(formatAttrValue(v))}"`;
  }
  return out;
}

function parseScalar(raw: string, spec: FieldSpec): unknown {
  switch (spec.type) {
    case 'integer':
    case 'number': {
      const n = Number(raw.trim());
      return raw.trim() !== '' && Number.isFinite(n) ? n : raw;
    }
    case 'boolean':
    case 'true':
      return raw === 'true' || raw === '' ? true : raw === 'false' ? false : raw;
    default:
      return raw;
  }
}

/** Reads attribute-typed fields from markup attributes. Bad values pass through for validation to report. */
function readFieldAttrs(attrs: Record<string, string>, fields: readonly FieldSpec[], out: Rec): void {
  for (const f of attrFields(fields)) {
    const raw = attrs[f.name];
    if (raw === undefined) continue;
    if (f.type === 'numberList' || f.type === 'integerList' || f.type === 'enumList') {
      const parts = raw.split(',').map(p => p.trim()).filter(p => p.length > 0);
      out[f.name] = f.type === 'enumList' ? parts : parts.map(p => parseScalar(p, { ...f, type: 'number' }));
    } else {
      out[f.name] = parseScalar(raw, f);
    }
  }
}

function jsonChild(tag: string, value: unknown, indent: string): string {
  return `${indent}<${tag}>${escapeText(JSON.stringify(value))}</${tag}>`;
}

// ============================================================================
// Serializer
// ============================================================================

function atomToMarkup(atom: InlineAtom): string {
  const raw = atom as unknown as Rec;
  const attrs = fieldAttrs({ ...raw, version: raw.version === 1 ? undefined : raw.version }, INLINE_FIELDS.atom);
  const hasAttrs = isRecord(atom.attrs) && Object.keys(atom.attrs).length > 0;
  return hasAttrs
    ? `<${atom.inline}${attrs}><attrs>${escapeText(JSON.stringify(atom.attrs))}</attrs></${atom.inline}>`
    : `<${atom.inline}${attrs}/>`;
}

/** Serializes `Inline[]` to Jinshu Markup in canonical nesting order: a > mark > b > i > u > s > sub/sup > code. */
export function inlinesToMarkup(inlines: readonly Inline[]): string {
  if (!Array.isArray(inlines)) return '';
  let out = '';
  for (const item of inlines) {
    switch (inlineKind(item)) {
      case 'break':
        out += '<br/>';
        break;
      case 'math':
        out += `<math>${escapeText(String((item as { math: unknown }).math))}</math>`;
        break;
      case 'atom':
        out += atomToMarkup(item as InlineAtom);
        break;
      case 'text': {
        const run = item as InlineRun;
        let piece = escapeText(String(run.text));
        if (run.code) piece = `<code>${piece}</code>`;
        if (run.script === 'sub') piece = `<sub>${piece}</sub>`;
        else if (run.script === 'super') piece = `<sup>${piece}</sup>`;
        if (run.strike) piece = `<s>${piece}</s>`;
        if (run.underline) piece = `<u>${piece}</u>`;
        if (run.italic) piece = `<i>${piece}</i>`;
        if (run.bold) piece = `<b>${piece}</b>`;
        if (run.highlight) {
          const tone = run.highlight === 'accent' ? '' : ` tone="${escapeAttr(String(run.highlight))}"`;
          piece = `<mark${tone}>${piece}</mark>`;
        }
        if (run.link) piece = `<a href="${escapeAttr(run.link)}">${piece}</a>`;
        out += piece;
        break;
      }
    }
  }
  return out;
}

function childBlocks(blocks: unknown, registry: JinshuRegistry, indent: string, options?: { includeLineNumbers?: boolean }): string[] {
  return Array.isArray(blocks) ? blocks.map(b => toBlockMarkup(b as JinshuBlock, registry, indent, options)) : [];
}

function wrap(indent: string, open: string, close: string, parts: readonly string[]): string {
  return parts.length === 0 ? `${indent}${open.replace(/>$/, '/>')}` : `${indent}${open}\n${parts.join('\n')}\n${indent}${close}`;
}

function chapterParts(chapters: unknown, indent: string): string[] {
  if (!Array.isArray(chapters)) return [];
  return chapters.map(ch => `${indent}<chapter${fieldAttrs(ch as Rec, CHAPTER_FIELDS)}>${escapeText(String((ch as Rec).title ?? ''))}</chapter>`);
}

/** Serializes one block to Jinshu Markup. Lossless for every field in the field table. */
export function toBlockMarkup(
  block: JinshuBlock,
  registry: JinshuRegistry = emptyRegistry,
  indent = '',
  options?: { includeLineNumbers?: boolean },
): string {
  const raw = block as unknown as Rec;
  const inner = indent + '  ';
  if (!isBuiltinBlockType(block.type)) {
    if (typeof block.type !== 'string' || !block.type.startsWith('x-')) return `${indent}<!-- unsupported block "${String(block.type)}" -->`;
    const ext = block as ExtensionBlock;
    const spec = registry.blocks.get(ext.type);
    const fallback = spec ? safeFallbackText(spec, ext.attrs, ext.fallback) : ext.fallback;
    const attrs = fieldAttrs({ ...raw, version: raw.version === 1 ? undefined : raw.version, fallback }, EXTENSION_BLOCK_FIELDS);
    const hasAttrs = isRecord(ext.attrs) && Object.keys(ext.attrs).length > 0;
    const attrsEl = hasAttrs ? `<attrs>${escapeText(JSON.stringify(ext.attrs))}</attrs>` : '';
    if (Array.isArray(ext.blocks) && ext.blocks.length > 0) {
      const parts = [...(hasAttrs ? [`${inner}${attrsEl}`] : []), ...childBlocks(ext.blocks, registry, inner, options)];
      return `${indent}<${ext.type}${attrs}>\n${parts.join('\n')}\n${indent}</${ext.type}>`;
    }
    const body = attrsEl + (Array.isArray(ext.runs) ? inlinesToMarkup(ext.runs) : '');
    return body ? `${indent}<${ext.type}${attrs}>${body}</${ext.type}>` : `${indent}<${ext.type}${attrs}/>`;
  }

  const attrs = fieldAttrs(raw, BLOCK_FIELDS[block.type]);
  switch (block.type) {
    case 'paragraph':
      return `${indent}<p${attrs}>${inlinesToMarkup(block.runs)}</p>`;
    case 'heading':
      return `${indent}<h${block.level}${attrs}>${inlinesToMarkup(block.runs)}</h${block.level}>`;
    case 'quote':
      return wrap(indent, `<quote${attrs}>`, '</quote>', childBlocks(block.blocks, registry, inner, options));
    case 'callout':
      return wrap(indent, `<callout${attrs}>`, '</callout>', [
        ...(Array.isArray(block.title) && block.title.length > 0 ? [`${inner}<title>${inlinesToMarkup(block.title)}</title>`] : []),
        ...childBlocks(block.blocks, registry, inner, options),
      ]);
    case 'details':
      return wrap(indent, `<details${attrs}>`, '</details>', [
        `${inner}<summary>${inlinesToMarkup(block.summary)}</summary>`,
        ...childBlocks(block.blocks, registry, inner, options),
      ]);
    case 'list': {
      const items = (Array.isArray(block.items) ? block.items : []).map(item => {
        const liAttrs = fieldAttrs(item as unknown as Rec, LIST_ITEM_FIELDS);
        if (!Array.isArray(item.blocks) || item.blocks.length === 0) {
          return `${inner}<li${liAttrs}>${inlinesToMarkup(item.runs)}</li>`;
        }
        const parts: string[] = [];
        if (Array.isArray(item.runs) && item.runs.length > 0) parts.push(`${inner}  <p>${inlinesToMarkup(item.runs)}</p>`);
        parts.push(...childBlocks(item.blocks, registry, inner + '  ', options));
        return `${inner}<li${liAttrs}>\n${parts.join('\n')}\n${inner}</li>`;
      });
      return wrap(indent, `<list${attrs}>`, '</list>', items);
    }
    case 'code': {
      const code = String(block.code ?? '');
      if (code.length === 0) return `${indent}<code${attrs}/>`;
      const fence = '`'.repeat(Math.max(3, longestBacktickRun(code) + 1));
      const body = options?.includeLineNumbers
        ? code.split('\n').map((line, i) => `L${i + 1}: ${line}`).join('\n')
        : code;
      return `${indent}<code${attrs}>\n${fence}\n${body}\n${fence}\n${indent}</code>`;
    }
    case 'math':
      return `${indent}<math${attrs}>${escapeText(String(block.latex ?? ''))}</math>`;
    case 'table': {
      const rows = (Array.isArray(block.rows) ? block.rows : []).map((row, r) => {
        const cellTag = r === 0 && block.headerRow ? 'th' : 'td';
        const cells = (Array.isArray(row.cells) ? row.cells : []).map((c: readonly Inline[]) => `<${cellTag}>${inlinesToMarkup(c)}</${cellTag}>`).join('');
        return `${inner}<tr${fieldAttrs(row as unknown as Rec, TABLE_ROW_FIELDS)}>${cells}</tr>`;
      });
      return wrap(indent, `<table${attrs}>`, '</table>', rows);
    }
    case 'columns': {
      const cols = (Array.isArray(block.columns) ? block.columns : []).map(col =>
        wrap(inner, `<col id="${escapeAttr(String(col.id))}">`, '</col>', childBlocks(col.blocks, registry, inner + '  ', options)),
      );
      return wrap(indent, `<columns${attrs}>`, '</columns>', cols);
    }
    case 'divider':
      return `${indent}<divider${attrs}/>`;
    case 'image':
      return wrap(indent, `<image${attrs}>`, '</image>',
        Array.isArray(block.caption) && block.caption.length > 0 ? [`${inner}<caption>${inlinesToMarkup(block.caption)}</caption>`] : []);
    case 'video':
      return wrap(indent, `<video${attrs}>`, '</video>', [
        ...(Array.isArray(block.caption) && block.caption.length > 0 ? [`${inner}<caption>${inlinesToMarkup(block.caption)}</caption>`] : []),
        ...chapterParts(block.chapters, inner),
        ...(Array.isArray(block.subtitles) ? block.subtitles.map(t => `${inner}<track${fieldAttrs(t as unknown as Rec, SUBTITLE_FIELDS)}/>`) : []),
      ]);
    case 'audio':
      return wrap(indent, `<audio${attrs}>`, '</audio>', [
        ...chapterParts(block.chapters, inner),
        ...childBlocks(block.transcript, registry, inner, options),
      ]);
    case 'file':
      return `${indent}<file${attrs}/>`;
  }
  return '';
}

function safeFallbackText(spec: { fallbackText: (attrs: any) => string }, attrs: unknown, current: string): string {
  try {
    const text = spec.fallbackText(attrs);
    return typeof text === 'string' ? text : current;
  } catch {
    return current;
  }
}

/** Every inline atom in the document by id, so markup that echoes `<x-… id>` keeps the atom's attrs. */
export function collectAtoms(blocks: readonly JinshuBlock[]): Map<string, InlineAtom> {
  const out = new Map<string, InlineAtom>();
  const visitInlines = (list: unknown) => {
    if (!Array.isArray(list)) return;
    for (const item of list) {
      if (inlineKind(item) === 'atom' && typeof (item as InlineAtom).id === 'string') out.set((item as InlineAtom).id, item as InlineAtom);
    }
  };
  const visit = (list: unknown) => {
    if (!Array.isArray(list)) return;
    for (const b of list) {
      if (!isRecord(b)) continue;
      const fields = isBuiltinBlockType(b.type) ? BLOCK_FIELDS[b.type] : EXTENSION_BLOCK_FIELDS;
      for (const f of fields) {
        const v = b[f.name];
        if (f.type === 'inlines') visitInlines(v);
        else if (f.type === 'blocks') visit(v);
        else if (Array.isArray(v) && (f.type === 'items' || f.type === 'columns' || f.type === 'rows')) {
          for (const entry of v) {
            if (!isRecord(entry)) continue;
            visitInlines(entry.runs);
            visit(entry.blocks);
            if (Array.isArray(entry.cells)) entry.cells.forEach(visitInlines);
          }
        }
      }
    }
  };
  visit(blocks);
  return out;
}

export interface AgentViewOptions {
  readonly registry?: JinshuRegistry;
  readonly outline?: boolean;
  readonly range?: { readonly from?: string; readonly to?: string };
  readonly search?: string;
  readonly maxChars?: number;
  readonly includeLineNumbers?: boolean;
}

/** Document-level attributes and `<meta>` for the `<doc>` wrapper, or `null` when there is nothing to carry. */
function docWrapper(doc: JinshuDocument): { open: string; meta?: string } | null {
  const raw = doc as unknown as Rec;
  const attrs = fieldAttrs({ ...raw, version: raw.version === 1 ? undefined : raw.version, title: raw.title === '' ? undefined : raw.title }, DOC_FIELDS);
  const hasMeta = isRecord(raw.meta) && Object.keys(raw.meta).length > 0;
  if (!attrs && !hasMeta) return null;
  return { open: `<doc${attrs}>`, ...(hasMeta ? { meta: jsonChild('meta', raw.meta, '') } : {}) };
}

/** Renders a `JinshuDocument` into Jinshu Markup for AI agents; lossless for the whole document. */
export function toAgentView(doc: JinshuDocument, options?: AgentViewOptions): string {
  const registry = options?.registry ?? emptyRegistry;

  if (options?.outline) {
    const outline = extractOutline(doc);
    const header = doc.title ? `# ${doc.title}\n` : '';
    if (outline.length === 0) return `${header}(No headings; ${doc.blocks.length} top-level blocks)`;
    return header + outline.map(item => `${'  '.repeat(item.level - 1)}- [h${item.level} id="${item.id}"] ${item.text}`).join('\n');
  }

  let selectedBlocks = [...doc.blocks];

  if (options?.range) {
    const { from, to } = options.range;
    const startIdx = from ? selectedBlocks.findIndex(b => b.id === from) : 0;
    const endIdx = to ? selectedBlocks.findIndex(b => b.id === to) : selectedBlocks.length - 1;
    const s = startIdx >= 0 ? startIdx : 0;
    const e = endIdx >= 0 ? endIdx : selectedBlocks.length - 1;
    selectedBlocks = selectedBlocks.slice(s, Math.max(s, e + 1));
  }

  if (options?.search) {
    const needle = options.search.toLowerCase();
    const matchedIndices = new Set<number>();
    selectedBlocks.forEach((block, idx) => {
      const m = toBlockMarkup(block, registry);
      if (m.toLowerCase().includes(needle)) {
        if (idx > 0) matchedIndices.add(idx - 1);
        matchedIndices.add(idx);
        if (idx + 1 < selectedBlocks.length) matchedIndices.add(idx + 1);
      }
    });
    selectedBlocks = selectedBlocks.filter((_, idx) => matchedIndices.has(idx));
  }

  const wrapper = docWrapper(doc);
  const lines: string[] = [];
  if (wrapper) {
    lines.push(wrapper.open);
    if (wrapper.meta) lines.push(wrapper.meta);
  }
  for (const b of selectedBlocks) {
    lines.push(toBlockMarkup(b, registry, '', { includeLineNumbers: options?.includeLineNumbers }));
  }
  if (wrapper) lines.push('</doc>');

  const full = lines.join('\n');
  const maxChars = options?.maxChars ?? 32_000;
  if (full.length > maxChars && !options?.range && !options?.search) {
    const outline = toAgentView(doc, { ...options, outline: true });
    return `<!-- Document truncated (${full.length} chars > ${maxChars}). Use range { from, to } or search to inspect sections. -->\n${outline}`;
  }
  return full;
}

// ============================================================================
// Zero-dependency Recursive-Descent Jinshu Markup Parser
// ============================================================================

interface XmlNode {
  readonly kind: 'element' | 'text';
  readonly tag?: string;
  readonly attrs?: Record<string, string>;
  readonly children?: XmlNode[];
  readonly text?: string;
  readonly rawBody?: string;
}

class XmlParser {
  private pos = 0;
  private readonly src: string;
  readonly problems: Problem[] = [];

  constructor(src: string) {
    this.src = src;
  }

  private eof(): boolean {
    return this.pos >= this.src.length;
  }

  private peek(): string {
    return this.src[this.pos] ?? '';
  }

  private skipWs(): void {
    while (!this.eof() && /\s/.test(this.src[this.pos])) this.pos += 1;
  }

  parseNodes(stopTag?: string): XmlNode[] {
    const nodes: XmlNode[] = [];
    while (!this.eof()) {
      if (this.src.startsWith('<!--', this.pos)) {
        const end = this.src.indexOf('-->', this.pos + 4);
        if (end < 0) {
          this.problems.push({ code: 'PARSE', severity: 'error', message: 'Unclosed XML comment.' });
          this.pos = this.src.length;
          break;
        }
        this.pos = end + 3;
        continue;
      }
      if (this.src.startsWith('</', this.pos)) {
        if (!stopTag) {
          const closeEnd = this.src.indexOf('>', this.pos);
          const found = closeEnd >= 0 ? this.src.slice(this.pos + 2, closeEnd).trim() : '';
          this.problems.push({ code: 'PARSE', severity: 'error', message: `Unexpected closing tag </${found}>.` });
          this.pos = closeEnd >= 0 ? closeEnd + 1 : this.src.length;
          continue;
        }
        const closeStart = this.pos;
        this.pos += 2;
        const name = this.readTagName();
        this.skipWs();
        if (this.peek() === '>') this.pos += 1;
        if (name !== stopTag) {
          this.problems.push({
            code: 'PARSE',
            severity: 'error',
            message: `Mismatched closing tag </${name}>; expected </${stopTag}>.`,
          });
          this.pos = closeStart;
        }
        return nodes;
      }
      if (this.peek() === '<') {
        const el = this.parseElement();
        if (el) nodes.push(el);
      } else {
        const nextLt = this.src.indexOf('<', this.pos);
        const end = nextLt < 0 ? this.src.length : nextLt;
        const raw = this.src.slice(this.pos, end);
        this.pos = end;
        nodes.push({ kind: 'text', text: unescapeXml(raw) });
      }
    }
    if (stopTag) {
      this.problems.push({ code: 'PARSE', severity: 'error', message: `Unclosed tag <${stopTag}>.` });
    }
    return nodes;
  }

  private readTagName(): string {
    const start = this.pos;
    while (!this.eof() && /[a-zA-Z0-9_-]/.test(this.src[this.pos])) this.pos += 1;
    return this.src.slice(start, this.pos);
  }

  private parseElement(): XmlNode | null {
    this.pos += 1; // consume '<'
    const rawTag = this.readTagName();
    if (!rawTag) {
      this.problems.push({ code: 'PARSE', severity: 'error', message: `Expected tag name at offset ${this.pos}.` });
      return null;
    }
    const tag = rawTag.toLowerCase();
    const attrs: Record<string, string> = {};

    while (!this.eof()) {
      this.skipWs();
      if (this.src.startsWith('/>', this.pos)) {
        this.pos += 2;
        return { kind: 'element', tag, attrs, children: [] };
      }
      if (this.peek() === '>') {
        this.pos += 1;
        break;
      }
      const attrName = this.readTagName();
      if (!attrName) {
        this.problems.push({ code: 'PARSE', severity: 'error', message: `Malformed attribute in <${tag}>.` });
        const gt = this.src.indexOf('>', this.pos);
        this.pos = gt >= 0 ? gt + 1 : this.src.length;
        return null;
      }
      this.skipWs();
      if (this.peek() !== '=') {
        this.problems.push({ code: 'PARSE', severity: 'error', message: `Attribute "${attrName}" in <${tag}> must have a double-quoted value.` });
        continue;
      }
      this.pos += 1;
      this.skipWs();
      const quote = this.peek();
      if (quote !== '"') {
        this.problems.push({ code: 'PARSE', severity: 'error', message: `Attribute "${attrName}" in <${tag}> must be double-quoted.` });
        const gt = this.src.indexOf('>', this.pos);
        this.pos = gt >= 0 ? gt + 1 : this.src.length;
        return null;
      }
      this.pos += 1;
      const valStart = this.pos;
      const valEnd = this.src.indexOf('"', valStart);
      if (valEnd < 0) {
        this.problems.push({ code: 'PARSE', severity: 'error', message: `Unclosed attribute quote for "${attrName}" in <${tag}>.` });
        this.pos = this.src.length;
        return null;
      }
      attrs[attrName] = unescapeXml(this.src.slice(valStart, valEnd));
      this.pos = valEnd + 1;
    }

    // Special raw-body parsing for fenced block `<code>` (distinguished by `lang` or `id` attr or fenced body)
    if (tag === 'code' && (['id', 'language', 'filename', 'highlightLines'].some(k => k in attrs) || this.looksLikeFencedCode())) {
      const rawBody = this.readCodeBlockBody();
      return { kind: 'element', tag, attrs, rawBody };
    }

    const children = this.parseNodes(tag);
    return { kind: 'element', tag, attrs, children };
  }

  private looksLikeFencedCode(): boolean {
    const slice = this.src.slice(this.pos, this.pos + 16);
    return /^\s*```/.test(slice);
  }

  private readCodeBlockBody(): string {
    // A backtick fence: optional newline, the fence (>= 3 backticks, possibly indented), body, the same
    // fence, `</code>`. An indented fence's indent is taken off each body line, as Markdown does.
    const save = this.pos;
    const open = /^\n?([ \t]*)(`{3,})\n/.exec(this.src.slice(this.pos));
    if (open) {
      const [opening, indent, fence] = open;
      const bodyStart = this.pos + opening.length;
      const close = new RegExp(`(^|\\n)[ \\t]*${fence}[ \\t]*\\n?[ \\t]*</code>`);
      const rest = this.src.slice(bodyStart);
      const found = close.exec(rest);
      if (found) {
        this.pos = bodyStart + found.index + found[0].length;
        const body = rest.slice(0, found.index);
        if (!indent) return body;
        return body
          .split('\n')
          .map(line => {
            let cut = 0;
            while (cut < indent.length && line[cut] === indent[cut]) cut += 1;
            return line.slice(cut);
          })
          .join('\n');
      }
    }

    // Fallback: raw text until </code>
    this.pos = save;
    const closeIdx = this.src.indexOf('</code>', this.pos);
    if (closeIdx < 0) {
      this.problems.push({ code: 'PARSE', severity: 'error', message: 'Unclosed <code> block.' });
      const body = this.src.slice(this.pos);
      this.pos = this.src.length;
      return unescapeXml(body);
    }
    const raw = this.src.slice(this.pos, closeIdx);
    this.pos = closeIdx + '</code>'.length;
    return unescapeXml(raw);
  }
}


const BLOCK_TAGS = new Set([
  'p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'quote', 'callout', 'details',
  'list', 'li', 'table', 'columns', 'col', 'divider', 'image', 'video', 'audio', 'file',
]);

/** A block-level element: a block tag, or `<code>`/`<math>` carrying block attributes. */
function isBlockNode(node: XmlNode): boolean {
  if (node.kind !== 'element' || !node.tag) return false;
  if (BLOCK_TAGS.has(node.tag)) return true;
  return (node.tag === 'code' && node.rawBody !== undefined) || (node.tag === 'math' && node.attrs?.id !== undefined);
}

interface ParseContext {
  readonly problems: Problem[];
  readonly registry: JinshuRegistry;
  readonly existingAtoms?: ReadonlyMap<string, InlineAtom>;
  /** The document's link and src rules, so an allowed `lx:` link is kept. */
  readonly check?: CheckOptions;
}

function textOf(nodes: readonly XmlNode[]): string {
  return nodes.map(c => (c.kind === 'text' ? c.text ?? '' : textOf(c.children ?? []))).join('');
}

function parseJsonChild(node: XmlNode, ctx: ParseContext, ownerId?: string): Rec | undefined {
  const text = textOf(node.children ?? []).trim();
  if (text === '') return {};
  try {
    const value = JSON.parse(text) as unknown;
    if (isRecord(value)) return value;
  } catch {
    // reported below
  }
  ctx.problems.push({ code: 'PARSE', severity: 'error', ...(ownerId ? { id: ownerId } : {}), message: `<${node.tag}> must hold a JSON object.` });
  return undefined;
}

function nodeToAtom(node: XmlNode, ctx: ParseContext): Inline {
  const attrs = node.attrs ?? {};
  const lookupId = attrs.id;
  const existing = lookupId !== undefined ? ctx.existingAtoms?.get(lookupId) : undefined;
  const fields: Rec = {};
  readFieldAttrs(attrs, INLINE_FIELDS.atom, fields);
  const attrsNode = (node.children ?? []).find(c => c.kind === 'element' && c.tag === 'attrs');
  const parsedAttrs = attrsNode ? parseJsonChild(attrsNode, ctx) : undefined;
  return {
    ...(lookupId !== undefined ? { id: lookupId } : {}),
    inline: node.tag as `x-${string}-${string}`,
    version: (fields.version as number | undefined) ?? existing?.version ?? 1,
    attrs: parsedAttrs ?? existing?.attrs ?? {},
    fallback: (fields.fallback as string | undefined) ?? existing?.fallback ?? `[${node.tag}]`,
  } as InlineAtom;
}

function nodesToInlines(
  nodes: readonly XmlNode[],
  marks: Partial<Omit<InlineRun, 'text'>>,
  ctx: ParseContext,
  inHeading = false,
): Inline[] {
  const out: Inline[] = [];
  const nested = (children: readonly XmlNode[], next: Partial<Omit<InlineRun, 'text'>>) =>
    out.push(...nodesToInlines(children, next, ctx, inHeading));
  for (const node of nodes) {
    if (node.kind === 'text') {
      const text = (node.text ?? '').replace(/\n/g, ' ');
      if (text.length > 0) out.push({ text, ...marks });
      continue;
    }
    const tag = node.tag!;
    const attrs = node.attrs ?? {};
    const children = node.children ?? [];

    if (tag === 'br') {
      out.push({ break: true });
    } else if (tag === 'math') {
      out.push({ math: textOf(children) });
    } else if (tag === 'b' || tag === 'strong') {
      nested(children, { ...marks, bold: true });
    } else if (tag === 'i' || tag === 'em') {
      nested(children, { ...marks, italic: true });
    } else if (tag === 'u') {
      nested(children, { ...marks, underline: true });
    } else if (tag === 's' || tag === 'del') {
      nested(children, { ...marks, strike: true });
    } else if (tag === 'code' && node.rawBody === undefined) {
      const raw = textOf(children);
      if (raw.length > 0) out.push({ text: raw, ...marks, code: true });
    } else if (tag === 'sub') {
      nested(children, { ...marks, script: 'sub' });
    } else if (tag === 'sup') {
      nested(children, { ...marks, script: 'super' });
    } else if (tag === 'mark') {
      nested(children, { ...marks, highlight: (attrs.tone || 'accent') as InlineRun['highlight'] });
    } else if (tag === 'a') {
      const href = attrs.href;
      const linkError = href ? validateLink(href, ctx.check) : null;
      if (!href) {
        ctx.problems.push({ code: 'PARSE', severity: 'error', message: '<a> tag requires an href attribute.' });
      } else if (linkError) {
        // Keep the words, drop the unsafe target: the text is still the author's.
        ctx.problems.push({ code: 'INVALID', severity: 'warning', message: `${linkError}; link removed, text kept.` });
        nested(children, marks);
      } else {
        nested(children, { ...marks, link: href });
      }
    } else if (tag.startsWith('x-')) {
      out.push(nodeToAtom(node, ctx));
    } else if (isBlockNode(node)) {
      ctx.problems.push({ code: 'PARSE', severity: 'error', message: `Block tag <${tag}> is not allowed inside inline content.` });
    } else {
      ctx.problems.push({ code: 'PARSE', severity: 'warning', message: `Unknown inline tag <${tag}> unwrapped.` });
      nested(children, marks);
    }
  }
  return normalizeInlines(out, { inHeading });
}

function elementNodes(nodes: readonly XmlNode[]): XmlNode[] {
  return nodes.filter(n => n.kind === 'element');
}

function childBlocksOf(nodes: readonly XmlNode[], ctx: ParseContext): JinshuBlock[] {
  return elementNodes(nodes)
    .map(c => nodeToBlock(c, ctx))
    .filter((b): b is JinshuBlock => b !== null);
}

function parseChapter(node: XmlNode): Rec {
  const out: Rec = {};
  readFieldAttrs(node.attrs ?? {}, CHAPTER_FIELDS, out);
  out.title = textOf(node.children ?? []);
  return out;
}

function parseTrack(node: XmlNode): Rec {
  const out: Rec = {};
  readFieldAttrs(node.attrs ?? {}, SUBTITLE_FIELDS, out);
  return out;
}

const TAG_TYPES: Readonly<Record<string, string>> = {
  p: 'paragraph', quote: 'quote', callout: 'callout', details: 'details', list: 'list', code: 'code',
  math: 'math', table: 'table', columns: 'columns', divider: 'divider', image: 'image', video: 'video',
  audio: 'audio', file: 'file',
};

function nodeToBlock(node: XmlNode, ctx: ParseContext): JinshuBlock | null {
  if (node.kind !== 'element' || !node.tag) return null;
  const tag = node.tag;
  const attrs = node.attrs ?? {};
  const children = node.children ?? [];
  const { problems } = ctx;
  const id = attrs.id || newBlockId(tag.slice(0, 4));
  const inlines = (nodes: readonly XmlNode[], inHeading = false) => nodesToInlines(nodes, {}, ctx, inHeading);

  const headingMatch = /^h([1-6])$/.exec(tag);
  const blockType = headingMatch ? 'heading' : TAG_TYPES[tag] ?? (tag.startsWith('x-') ? tag : undefined);
  if (!blockType) {
    problems.push({ code: 'PARSE', severity: 'error', message: `Unsupported block tag <${tag}>.` });
    return null;
  }
  const out: Rec = { id, type: blockType };
  const fields = isBuiltinBlockType(blockType) ? BLOCK_FIELDS[blockType] : EXTENSION_BLOCK_FIELDS;
  readFieldAttrs(attrs, fields, out);
  out.id = id;

  switch (blockType) {
    case 'paragraph':
      out.runs = inlines(children);
      break;
    case 'heading': {
      let level = Number(headingMatch![1]);
      if (level > 4) {
        problems.push({ code: 'PARSE', severity: 'warning', id, message: `<${tag}> downgraded to <h4>.` });
        level = 4;
      }
      out.level = level as HeadingLevel;
      out.runs = inlines(children, true);
      break;
    }
    case 'quote':
      out.blocks = childBlocksOf(children, ctx);
      break;
    case 'callout':
    case 'details': {
      if (blockType === 'callout' && out.tone === undefined) out.tone = 'info';
      const blocks: JinshuBlock[] = [];
      for (const child of elementNodes(children)) {
        if (blockType === 'callout' && child.tag === 'title') out.title = inlines(child.children ?? []);
        else if (blockType === 'details' && child.tag === 'summary') out.summary = inlines(child.children ?? []);
        else {
          const b = nodeToBlock(child, ctx);
          if (b) blocks.push(b);
        }
      }
      if (blockType === 'details' && out.summary === undefined) out.summary = [];
      out.blocks = blocks;
      break;
    }
    case 'list': {
      if (out.style === undefined) out.style = 'bullet';
      const items: Rec[] = [];
      for (const child of elementNodes(children)) {
        if (child.tag !== 'li') {
          problems.push({ code: 'PARSE', severity: 'error', id, message: `Expected <li> inside <list>, got <${child.tag}>.` });
          continue;
        }
        const item: Rec = {};
        readFieldAttrs(child.attrs ?? {}, LIST_ITEM_FIELDS, item);
        item.id = child.attrs?.id || newBlockId('li');
        const liChildren = child.children ?? [];
        if (!liChildren.some(isBlockNode)) {
          item.runs = inlines(liChildren);
        } else {
          // a leading id-less <p> is the item's own text; the rest are child blocks
          const elems = elementNodes(liChildren);
          let startIdx = 0;
          item.runs = [];
          if (elems.length > 0 && elems[0].tag === 'p' && !elems[0].attrs?.id) {
            item.runs = inlines(elems[0].children ?? []);
            startIdx = 1;
          }
          const nestedBlocks = elems.slice(startIdx).map(e => nodeToBlock(e, ctx)).filter((b): b is JinshuBlock => b !== null);
          if (nestedBlocks.length > 0) item.blocks = nestedBlocks;
        }
        items.push(item);
      }
      out.items = items;
      break;
    }
    case 'code':
      if (out.language === undefined) out.language = 'text';
      out.code = (node.rawBody ?? textOf(children)).replace(/\r\n?/g, '\n');
      break;
    case 'math':
      out.latex = textOf(children).replace(/\r\n?/g, '\n');
      break;
    case 'table': {
      const rows: Rec[] = [];
      let headerFromMarkup = false;
      const readRow = (tr: XmlNode) => {
        const cellNodes = elementNodes(tr.children ?? []).filter(c => c.tag === 'td' || c.tag === 'th');
        if (rows.length === 0 && cellNodes.length > 0 && cellNodes.every(c => c.tag === 'th')) headerFromMarkup = true;
        rows.push({ id: tr.attrs?.id || newBlockId('r'), cells: cellNodes.map(c => inlines(c.children ?? [])) });
      };
      for (const section of elementNodes(children)) {
        if (section.tag === 'tr') readRow(section);
        else if (section.tag === 'thead' || section.tag === 'tbody') {
          if (section.tag === 'thead' && rows.length === 0) headerFromMarkup = true;
          for (const tr of elementNodes(section.children ?? [])) if (tr.tag === 'tr') readRow(tr);
        }
      }
      if (out.headerRow === undefined && headerFromMarkup) out.headerRow = true;
      out.rows = rows;
      break;
    }
    case 'columns':
      out.columns = elementNodes(children)
        .filter(c => c.tag === 'col')
        .map(c => ({ id: c.attrs?.id || newBlockId('col'), blocks: childBlocksOf(c.children ?? [], ctx) }));
      break;
    case 'divider':
    case 'file':
      break;
    case 'image':
    case 'video':
    case 'audio': {
      const chapters: Rec[] = [];
      const subtitles: Rec[] = [];
      const transcript: JinshuBlock[] = [];
      for (const child of elementNodes(children)) {
        if (child.tag === 'caption' && blockType !== 'audio') out.caption = inlines(child.children ?? []);
        else if (child.tag === 'chapter' && blockType !== 'image') chapters.push(parseChapter(child));
        else if (child.tag === 'track' && blockType === 'video') subtitles.push(parseTrack(child));
        else if (blockType === 'audio') {
          const b = nodeToBlock(child, ctx);
          if (b) transcript.push(b);
        } else {
          problems.push({ code: 'PARSE', severity: 'warning', id, message: `<${child.tag}> is not a child of <${tag}>; ignored.` });
        }
      }
      if (chapters.length > 0) out.chapters = chapters;
      if (subtitles.length > 0) out.subtitles = subtitles;
      if (transcript.length > 0) out.transcript = transcript;
      break;
    }
    default: {
      // Extension block: `<attrs>` JSON child, then inline or block content.
      const spec = ctx.registry.blocks.get(blockType);
      const attrsNode = elementNodes(children).find(c => c.tag === 'attrs');
      out.attrs = (attrsNode ? parseJsonChild(attrsNode, ctx, id) : undefined) ?? {};
      if (out.version === undefined) out.version = spec?.version ?? 1;
      if (out.fallback === undefined) out.fallback = `[${blockType}]`;
      const rest = children.filter(c => c !== attrsNode);
      const hasText = rest.some(c => c.kind === 'text' && (c.text ?? '').trim().length > 0);
      const elems = elementNodes(rest);
      const asBlocks = spec
        ? typeof spec.content === 'object'
        : !hasText && elems.length > 0 && elems.every(e => isBlockNode(e) || (e.tag?.startsWith('x-') && !ctx.registry.inlines.has(e.tag)));
      if (asBlocks) {
        const blocks = childBlocksOf(rest, ctx);
        if (blocks.length > 0) out.blocks = blocks;
      } else if (spec?.content !== 'none' && (hasText || elems.length > 0)) {
        out.runs = inlines(rest);
      }
      break;
    }
  }
  return out as unknown as JinshuBlock;
}

export interface ParseMarkupResult {
  readonly blocks: readonly JinshuBlock[];
  readonly problems: readonly Problem[];
  readonly title?: string;
  readonly lang?: string;
  readonly id?: string;
  readonly version?: number;
  readonly meta?: Readonly<Record<string, unknown>>;
}

/** Parses a Jinshu Markup fragment (or `<doc>...</doc>`) into `JinshuBlock[]`. */
export function parseAgentMarkup(
  markup: string,
  options?: { readonly registry?: JinshuRegistry; readonly existingAtoms?: ReadonlyMap<string, InlineAtom>; readonly check?: CheckOptions },
): ParseMarkupResult {
  const normalized = String(markup ?? '').replace(/\r\n?/g, '\n');
  const parser = new XmlParser(normalized);
  const nodes = parser.parseNodes();
  const problems: Problem[] = [...parser.problems];
  const ctx: ParseContext = { problems, registry: options?.registry ?? emptyRegistry, existingAtoms: options?.existingAtoms, check: options?.check };

  let rootElements = elementNodes(nodes);
  const docFields: Rec = {};

  if (rootElements.length === 1 && rootElements[0].tag === 'doc') {
    readFieldAttrs(rootElements[0].attrs ?? {}, DOC_FIELDS, docFields);
    rootElements = elementNodes(rootElements[0].children ?? []);
    const metaNode = rootElements.find(n => n.tag === 'meta');
    if (metaNode) {
      const meta = parseJsonChild(metaNode, ctx);
      if (meta) docFields.meta = meta;
      rootElements = rootElements.filter(n => n !== metaNode);
    }
  }

  const blocks: JinshuBlock[] = [];
  for (const el of rootElements) {
    const b = nodeToBlock(el, ctx);
    if (b) blocks.push(b);
  }

  return {
    blocks,
    problems,
    ...(typeof docFields.title === 'string' ? { title: docFields.title } : {}),
    ...(typeof docFields.lang === 'string' ? { lang: docFields.lang } : {}),
    ...(typeof docFields.id === 'string' ? { id: docFields.id } : {}),
    ...(typeof docFields.version === 'number' ? { version: docFields.version } : {}),
    ...(docFields.meta ? { meta: docFields.meta as Rec } : {}),
  };
}

/** Parses an inline Jinshu Markup fragment (used by `edit` op). Rejects block tags. */
export function parseInlineMarkup(
  markup: string,
  options?: { readonly existingAtoms?: ReadonlyMap<string, InlineAtom>; readonly inHeading?: boolean; readonly registry?: JinshuRegistry; readonly check?: CheckOptions },
): { readonly runs: readonly Inline[]; readonly problems: readonly Problem[] } {
  const normalized = String(markup ?? '').replace(/\r\n?/g, '\n');
  const parser = new XmlParser(normalized);
  const nodes = parser.parseNodes();
  const problems: Problem[] = [...parser.problems];
  const ctx: ParseContext = { problems, registry: options?.registry ?? emptyRegistry, existingAtoms: options?.existingAtoms, check: options?.check };
  const runs = nodesToInlines(nodes, {}, ctx, options?.inHeading);
  return { runs, problems };
}

/** Helper to fingerprint a block's textual content for `rewrite` ID alignment. */
export function blockFingerprint(block: JinshuBlock): string {
  switch (block.type) {
    case 'paragraph':
    case 'heading':
      return `${block.type}:${plainText(block.runs).trim().slice(0, 80)}`;
    case 'code':
      return `code:${block.language}:${String(block.code).trim().slice(0, 80)}`;
    case 'math':
      return `math:${String(block.latex).trim().slice(0, 80)}`;
    case 'image':
    case 'video':
    case 'audio':
    case 'file':
      return `${block.type}:${block.src}`;
    default:
      return `${block.type}:${JSON.stringify(block).slice(0, 80)}`;
  }
}
