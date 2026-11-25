import type {
  AudioBlock,
  CalloutBlock,
  CalloutTone,
  CodeBlock,
  ColumnItem,
  ColumnsBlock,
  DetailsBlock,
  DividerBlock,
  ExtensionBlock,
  FileBlock,
  FileDisplay,
  HardBreak,
  HeadingBlock,
  HeadingLevel,
  ImageBlock,
  ImageSize,
  Inline,
  InlineAtom,
  InlineKind,
  InlineMath,
  InlineRun,
  JinshuBlock,
  JinshuDocument,
  ListBlock,
  ListItem,
  ListStyle,
  MathBlock,
  MediaChapter,
  OutlineItem,
  ParagraphBlock,
  QuoteBlock,
  TableAlign,
  TableBlock,
  TableRow,
  VideoBlock,
  VideoSubtitle,
} from './types';

const ID_ALPHABET = '0123456789abcdefghijklmnopqrstuvwxyz';

/** Generates a stable short random block id matching `[a-z0-9_-]{1,32}`. */
export function newBlockId(prefix = 'b'): string {
  let rand = '';
  for (let i = 0; i < 6; i++) {
    rand += ID_ALPHABET[Math.floor(Math.random() * ID_ALPHABET.length)];
  }
  const cleanPrefix = prefix.toLowerCase().replace(/[^a-z0-9_-]/g, '').slice(0, 12) || 'b';
  return `${cleanPrefix}_${rand}`;
}

const INLINE_KIND_KEYS: readonly (readonly [string, InlineKind])[] = [
  ['text', 'text'],
  ['break', 'break'],
  ['math', 'math'],
  ['inline', 'atom'],
];

/**
 * The one inline discriminator: which of `text` / `break` / `math` / `inline` is present.
 * `undefined` when none is, or when more than one is (a validation error).
 */
export function inlineKind(item: unknown): InlineKind | undefined {
  if (item === null || typeof item !== 'object' || Array.isArray(item)) return undefined;
  let kind: InlineKind | undefined;
  for (const [key, k] of INLINE_KIND_KEYS) {
    if (Object.prototype.hasOwnProperty.call(item, key) && (item as Record<string, unknown>)[key] !== undefined) {
      if (kind) return undefined;
      kind = k;
    }
  }
  return kind;
}

export function isInlineRun(item: Inline): item is InlineRun {
  return inlineKind(item) === 'text';
}

export function isHardBreak(item: Inline): item is HardBreak {
  return inlineKind(item) === 'break';
}

export function isInlineAtom(item: Inline): item is InlineAtom {
  return inlineKind(item) === 'atom';
}

export function isInlineMath(item: Inline): item is InlineMath {
  return inlineKind(item) === 'math';
}

/** Converts `string | Inline[]` into `Inline[]`. */
export function toInlines(input: string | readonly Inline[]): readonly Inline[] {
  if (typeof input === 'string') {
    return input.length === 0 ? [] : [{ text: input }];
  }
  return input;
}

/** Extracts plain text from an `Inline[]` sequence. */
export function plainText(inlines: readonly Inline[] | undefined): string {
  if (!Array.isArray(inlines) || inlines.length === 0) return '';
  let out = '';
  for (const item of inlines) {
    switch (inlineKind(item)) {
      case 'text': out += (item as InlineRun).text; break;
      case 'break': out += '\n'; break;
      case 'math': out += (item as InlineMath).math; break;
      case 'atom': out += (item as InlineAtom).fallback; break;
    }
  }
  return out;
}

/** Computes UTF-16 code unit length of an `Inline[]` sequence (break/atom/math each count as 1). */
export function inlineLength(inlines: readonly Inline[]): number {
  let len = 0;
  for (const item of inlines) {
    if (isInlineRun(item)) len += item.text.length;
    else len += 1;
  }
  return len;
}

export function run(text: string, marks?: Partial<Omit<InlineRun, 'text'>>): InlineRun {
  return marks ? { text, ...marks } : { text };
}

export function hardBreak(): HardBreak {
  return { break: true };
}

export function inlineMath(latex: string): InlineMath {
  return { math: latex };
}

export function inlineAtom(
  inline: `x-${string}-${string}`,
  attrs: Readonly<Record<string, unknown>>,
  fallback: string,
  options?: { id?: string; version?: number },
): InlineAtom {
  return { id: options?.id ?? newBlockId('at'), inline, version: options?.version ?? 1, attrs, fallback };
}

export function paragraph(content: string | readonly Inline[] = [], id = newBlockId('p')): ParagraphBlock {
  return { id, type: 'paragraph', runs: toInlines(content) };
}

export function heading(level: HeadingLevel, content: string | readonly Inline[], id = newBlockId('h')): HeadingBlock {
  return { id, type: 'heading', level, runs: toInlines(content) };
}

export function quote(blocks: readonly JinshuBlock[], id = newBlockId('q')): QuoteBlock {
  return { id, type: 'quote', blocks };
}

export function callout(
  tone: CalloutTone,
  blocks: readonly JinshuBlock[],
  options?: { title?: string | readonly Inline[]; id?: string },
): CalloutBlock {
  const block: CalloutBlock = {
    id: options?.id ?? newBlockId('c'),
    type: 'callout',
    tone,
    ...(options?.title !== undefined ? { title: toInlines(options.title) } : {}),
    blocks,
  };
  return block;
}

export function details(
  summary: string | readonly Inline[],
  blocks: readonly JinshuBlock[],
  options?: { defaultOpen?: boolean; id?: string },
): DetailsBlock {
  return {
    id: options?.id ?? newBlockId('d'),
    type: 'details',
    summary: toInlines(summary),
    ...(options?.defaultOpen ? { defaultOpen: true } : {}),
    blocks,
  };
}

export function listItem(
  content: string | readonly Inline[],
  options?: { id?: string; checked?: boolean; blocks?: readonly JinshuBlock[] },
): ListItem {
  return {
    id: options?.id ?? newBlockId('li'),
    ...(options?.checked !== undefined ? { checked: options.checked } : {}),
    runs: toInlines(content),
    ...(options?.blocks && options.blocks.length > 0 ? { blocks: options.blocks } : {}),
  };
}

export function list(
  style: ListStyle,
  items: readonly ListItem[],
  options?: { start?: number; id?: string },
): ListBlock {
  return {
    id: options?.id ?? newBlockId('l'),
    type: 'list',
    style,
    ...(options?.start !== undefined && options.start !== 1 ? { start: options.start } : {}),
    items,
  };
}

export function codeBlock(
  language: string,
  code: string,
  options?: { filename?: string; highlightLines?: readonly number[]; id?: string },
): CodeBlock {
  return {
    id: options?.id ?? newBlockId('code'),
    type: 'code',
    language,
    code,
    ...(options?.filename ? { filename: options.filename } : {}),
    ...(options?.highlightLines && options.highlightLines.length > 0 ? { highlightLines: options.highlightLines } : {}),
  };
}

export function mathBlock(latex: string, id = newBlockId('m')): MathBlock {
  return { id, type: 'math', latex };
}

export function tableRow(cells: readonly (string | readonly Inline[])[], id = newBlockId('r')): TableRow {
  return { id, cells: cells.map(toInlines) };
}

/**
 * Builds a table. `header` becomes `rows[0]` with `headerRow: true`; pass `null` for a table without one.
 * A row given as a `TableRow` keeps its id; a plain cell array gets a fresh one.
 */
export function table(
  header: readonly (string | readonly Inline[])[] | null,
  rows: readonly (readonly (string | readonly Inline[])[] | TableRow)[],
  options?: { align?: readonly TableAlign[]; widths?: readonly number[]; id?: string; headerRowId?: string },
): TableBlock {
  const body = rows.map(r => (Array.isArray(r) ? tableRow(r as readonly (string | readonly Inline[])[]) : (r as TableRow)));
  return {
    id: options?.id ?? newBlockId('t'),
    type: 'table',
    ...(options?.align ? { align: options.align } : {}),
    ...(options?.widths ? { widths: options.widths } : {}),
    ...(header ? { headerRow: true as const } : {}),
    rows: header ? [tableRow(header, options?.headerRowId), ...body] : body,
  };
}

/** Number of columns (cells in the first row). */
export function tableColumnCount(block: TableBlock): number {
  return block.rows[0]?.cells.length ?? 0;
}

export function columns(
  cols: readonly (readonly JinshuBlock[] | ColumnItem)[],
  options?: { widths?: readonly number[]; id?: string },
): ColumnsBlock {
  return {
    id: options?.id ?? newBlockId('cols'),
    type: 'columns',
    ...(options?.widths ? { widths: options.widths } : {}),
    columns: cols.map(c => (Array.isArray(c) ? { id: newBlockId('col'), blocks: c } : (c as ColumnItem))),
  };
}

export function divider(id = newBlockId('hr')): DividerBlock {
  return { id, type: 'divider' };
}

export function image(
  src: string,
  options?: { alt?: string; width?: number; height?: number; caption?: string | readonly Inline[]; size?: ImageSize; id?: string },
): ImageBlock {
  return {
    id: options?.id ?? newBlockId('img'),
    type: 'image',
    src,
    ...(options?.alt !== undefined ? { alt: options.alt } : {}),
    ...(options?.width !== undefined ? { width: options.width } : {}),
    ...(options?.height !== undefined ? { height: options.height } : {}),
    ...(options?.caption !== undefined ? { caption: toInlines(options.caption) } : {}),
    ...(options?.size && options.size !== 'full' ? { size: options.size } : {}),
  };
}

export function video(
  src: string,
  options?: {
    poster?: string;
    durationMs?: number;
    caption?: string | readonly Inline[];
    chapters?: readonly MediaChapter[];
    subtitles?: readonly VideoSubtitle[];
    id?: string;
  },
): VideoBlock {
  return {
    id: options?.id ?? newBlockId('v'),
    type: 'video',
    src,
    ...(options?.poster ? { poster: options.poster } : {}),
    ...(options?.durationMs !== undefined ? { durationMs: options.durationMs } : {}),
    ...(options?.caption !== undefined ? { caption: toInlines(options.caption) } : {}),
    ...(options?.chapters && options.chapters.length > 0 ? { chapters: options.chapters } : {}),
    ...(options?.subtitles && options.subtitles.length > 0 ? { subtitles: options.subtitles } : {}),
  };
}

export function audio(
  src: string,
  options?: {
    title?: string;
    cover?: string;
    durationMs?: number;
    chapters?: readonly MediaChapter[];
    transcript?: readonly JinshuBlock[];
    id?: string;
  },
): AudioBlock {
  return {
    id: options?.id ?? newBlockId('a'),
    type: 'audio',
    src,
    ...(options?.title ? { title: options.title } : {}),
    ...(options?.cover ? { cover: options.cover } : {}),
    ...(options?.durationMs !== undefined ? { durationMs: options.durationMs } : {}),
    ...(options?.chapters && options.chapters.length > 0 ? { chapters: options.chapters } : {}),
    ...(options?.transcript && options.transcript.length > 0 ? { transcript: options.transcript } : {}),
  };
}

export function fileBlock(
  src: string,
  name: string,
  options?: {
    display?: FileDisplay;
    mimeType?: string;
    sizeBytes?: number;
    pageCount?: number;
    description?: string;
    id?: string;
  },
): FileBlock {
  return {
    id: options?.id ?? newBlockId('f'),
    type: 'file',
    src,
    name,
    ...(options?.display ? { display: options.display } : {}),
    ...(options?.mimeType ? { mimeType: options.mimeType } : {}),
    ...(options?.sizeBytes !== undefined ? { sizeBytes: options.sizeBytes } : {}),
    ...(options?.pageCount !== undefined ? { pageCount: options.pageCount } : {}),
    ...(options?.description ? { description: options.description } : {}),
  };
}

export function customBlock(
  type: `x-${string}-${string}`,
  attrs: Readonly<Record<string, unknown>>,
  fallback: string,
  options?: { version?: number; runs?: readonly Inline[]; blocks?: readonly JinshuBlock[]; id?: string },
): ExtensionBlock {
  return {
    id: options?.id ?? newBlockId('x'),
    type,
    version: options?.version ?? 1,
    attrs,
    fallback,
    ...(options?.runs ? { runs: options.runs } : {}),
    ...(options?.blocks ? { blocks: options.blocks } : {}),
  };
}

/** Creates a valid v1 JinshuDocument (defaults to a single empty paragraph if `blocks` is empty). */
export function createDocument(options?: {
  id?: string;
  title?: string;
  lang?: string;
  meta?: Readonly<Record<string, unknown>>;
  blocks?: readonly JinshuBlock[];
}): JinshuDocument {
  const blocks = options?.blocks && options.blocks.length > 0 ? options.blocks : [paragraph()];
  return {
    version: 1,
    ...(options?.id ? { id: options.id } : {}),
    ...(options?.title !== undefined ? { title: options.title } : {}),
    ...(options?.lang ? { lang: options.lang } : {}),
    ...(options?.meta ? { meta: options.meta } : {}),
    blocks,
  };
}

/** Recursively visits every JinshuBlock in document order. */
export function walkBlocks(blocks: readonly JinshuBlock[], visit: (block: JinshuBlock, parentId: string | 'doc') => void, parentId: string | 'doc' = 'doc'): void {
  for (const block of blocks) {
    visit(block, parentId);
    if (block.type === 'quote' || block.type === 'callout' || block.type === 'details') {
      walkBlocks(block.blocks, visit, block.id);
    } else if (block.type === 'list') {
      for (const item of block.items) {
        if (item.blocks) walkBlocks(item.blocks, visit, item.id);
      }
    } else if (block.type === 'columns') {
      for (const col of block.columns) {
        walkBlocks(col.blocks, visit, col.id);
      }
    } else if (block.type === 'audio' && block.transcript) {
      walkBlocks(block.transcript, visit, block.id);
    } else if (block.type.startsWith('x-') && (block as ExtensionBlock).blocks) {
      walkBlocks((block as ExtensionBlock).blocks!, visit, block.id);
    }
  }
}

/** Recursively collects blocks, optionally filtered by `type`. */
export function listBlocks<T extends JinshuBlock['type']>(
  doc: JinshuDocument,
  type: T,
): Extract<JinshuBlock, { readonly type: T }>[];
export function listBlocks(doc: JinshuDocument): JinshuBlock[];
export function listBlocks(doc: JinshuDocument, type?: string): JinshuBlock[] {
  const out: JinshuBlock[] = [];
  walkBlocks(doc.blocks, block => {
    if (!type || block.type === type) out.push(block);
  });
  return out;
}

/** Convenience wrapper returning all `file` blocks in the document (including nested). */
export function listFiles(doc: JinshuDocument): FileBlock[] {
  return listBlocks(doc, 'file');
}

/** Finds a block anywhere in the document by `id`. */
export function findBlockById(doc: JinshuDocument, id: string): JinshuBlock | undefined {
  let found: JinshuBlock | undefined;
  walkBlocks(doc.blocks, block => {
    if (!found && block.id === id) found = block;
  });
  return found;
}

/** Extracts heading outline items in document order. */
export function extractOutline(doc: JinshuDocument): OutlineItem[] {
  return listBlocks(doc, 'heading').map(h => ({
    id: h.id,
    level: h.level,
    text: plainText(h.runs),
  }));
}

/** Simple deterministic FNV-1a content hash for block change detection / observation contentHash. */
export function hashString(input: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(36);
}

export function blockContentHash(block: JinshuBlock): string {
  return hashString(JSON.stringify(block));
}
