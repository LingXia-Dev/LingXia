/**
 * What each file format can hold. JSON holds everything; Markdown holds a strict subset, and this table
 * is the one statement of it: `formatAllows`, `checkDocument(…, { format: 'markdown' })`, the Markdown
 * writer and the generated format reference all read it.
 */
import { inlineKind } from './helpers';
import type { CalloutTone, FormatSubject, Inline, InlineMarkKey, JinshuDocument, JinshuFormat, Problem } from './types';

export interface MarkdownBlockProfile {
  /** Fields beyond `id` and `type` that Markdown can say. */
  readonly fields: readonly string[];
  /** How the block is written. */
  readonly syntax: string;
}

export const MARKDOWN_BLOCKS: Readonly<Record<string, MarkdownBlockProfile>> = {
  paragraph: { fields: ['runs'], syntax: 'Text; an empty paragraph is not written, nor spaces at the edges of a line' },
  heading: { fields: ['level', 'runs'], syntax: '`#` to `####` (setext read; `#####` and `######` read as `####`)' },
  quote: { fields: ['blocks'], syntax: '`> `' },
  callout: { fields: ['tone', 'blocks'], syntax: 'Alert syntax: `> [!NOTE]` info, `> [!TIP]` tip, `> [!WARNING]` warning, `> [!CAUTION]` danger; no title' },
  list: { fields: ['style', 'start', 'items'], syntax: '`- ` bullet, `1. ` ordered from `start`, `- [ ] ` / `- [x] ` task when every item has a box; children indented' },
  code: { fields: ['language', 'code'], syntax: 'Fenced with backticks and the language; `text` is a bare fence; indented code reads as `text`' },
  math: { fields: ['latex'], syntax: '`$$` on its own lines; a `math` fence reads as math too' },
  table: { fields: ['align', 'headerRow', 'rows'], syntax: 'GFM table with its header row (required); `:--` `:-:` `--:` align; no line breaks in cells' },
  divider: { fields: [], syntax: '`---`' },
  image: { fields: ['src', 'alt'], syntax: '`![alt](src)` alone in a paragraph; an empty `alt` is the same as none' },
};

export const MARKDOWN_ITEM_FIELDS: readonly string[] = ['id', 'checked', 'runs', 'blocks'];
export const MARKDOWN_ROW_FIELDS: readonly string[] = ['id', 'cells'];

export type FormatMark = InlineMarkKey | 'math' | 'atom';

export const MARKDOWN_MARKS: Readonly<Partial<Record<FormatMark, string>>> = {
  bold: '`**bold**`',
  italic: '`*italic*`',
  strike: '`~~strike~~`',
  code: '`` `code` ``',
  link: '`[text](url)`, `<url>` and bare URLs',
  math: '`$latex$`',
};

/** Document fields Markdown can say: the title as a leading `# Title`, `meta.frontMatter` as front matter. */
export const MARKDOWN_DOC_FIELDS: Readonly<Record<string, string>> = {
  title: 'A leading `# Title` line, when it is plain text',
  meta: 'Only `meta.frontMatter`: the file\'s leading `---` block, kept verbatim',
};

export const CALLOUT_ALERTS: Readonly<Record<CalloutTone, string>> = { info: 'NOTE', tip: 'TIP', warning: 'WARNING', danger: 'CAUTION' };

/** Whether a format can hold this block type, mark or block field. */
export function formatAllows(
  format: JinshuFormat,
  what: { readonly block: string } | { readonly mark: FormatMark } | { readonly block: string; readonly field: string },
): boolean {
  if (format !== 'markdown') return true;
  if ('mark' in what) return MARKDOWN_MARKS[what.mark] !== undefined;
  const spec = MARKDOWN_BLOCKS[what.block];
  if (!spec) return false;
  if ('field' in what) return what.field === 'id' || what.field === 'type' || spec.fields.includes(what.field);
  return true;
}

/** Whether `$latex$` reads back as this latex: not empty, one line, no `$`, and in Markdown no edge spaces. */
export function mathWritable(latex: string, dialect: 'jinshu' | 'markdown'): boolean {
  if (latex === '' || latex.includes('\n') || latex.includes('$')) return false;
  if (dialect === 'markdown' && (/^\s|\s$/.test(latex) || latex.endsWith('\\'))) return false;
  return true;
}

type Rec = Record<string, unknown>;
const isRec = (v: unknown): v is Rec => v !== null && typeof v === 'object' && !Array.isArray(v);

/** Everything in `doc` a Markdown file cannot hold, as `FORMAT` errors. */
export function checkMarkdownProfile(doc: JinshuDocument): Problem[] {
  const problems: Problem[] = [];
  const seen = new Set<string>();
  const error = (message: string, subject: FormatSubject, id?: string) => {
    const key = `${id ?? ''}\u0000${message}`;
    if (seen.has(key)) return;
    seen.add(key);
    problems.push({ code: 'FORMAT', severity: 'error', ...(id !== undefined ? { id } : {}), message, subject });
  };
  if (!isRec(doc)) return problems;

  for (const key of Object.keys(doc)) {
    if (key === 'version' || key === 'blocks' || doc[key as keyof JinshuDocument] === undefined) continue;
    if (key === 'title') {
      if (typeof doc.title === 'string' && doc.title.includes('\n')) error('A Markdown title is one line.', { rule: 'titleLine' });
    } else if (key === 'meta') {
      const meta = doc.meta;
      const ok = isRec(meta) && Object.keys(meta).every(k => k === 'frontMatter') && (meta.frontMatter === undefined || typeof meta.frontMatter === 'string');
      if (!ok) error('Markdown keeps only meta.frontMatter (the front matter block); other meta is lost.', { field: 'meta' });
      else if (typeof meta.frontMatter === 'string' && /(^|\n)(?:---|\.\.\.)[ \t]*(\n|$)/.test(meta.frontMatter)) error('Front matter cannot contain a "---" or "..." line.', { rule: 'frontMatterLine' });
    } else error(`Markdown cannot hold the document's "${key}".`, { field: key });
  }

  const inlines = (list: unknown, id: string | undefined, where: 'text' | 'cell') => {
    if (!Array.isArray(list)) return;
    for (const item of list as Inline[]) {
      const kind = inlineKind(item);
      if (kind === 'atom') error('Markdown cannot hold inline atoms; they are written as their fallback text.', { mark: 'atom' }, id);
      else if (kind === 'break' && where === 'cell') error('A Markdown table cell cannot hold a line break.', { rule: 'cellBreak' }, id);
      else if (kind === 'math' && !mathWritable((item as { math: string }).math, 'markdown')) {
        error('Inline math in Markdown must be one line with no "$" and no space at either end.', { rule: 'inlineMath' }, id);
      } else if (kind === 'text') {
        const run = item as unknown as Rec;
        for (const mark of ['underline', 'highlight', 'script'] as const) if (run[mark] !== undefined) error(`Markdown cannot hold ${mark}.`, { mark }, id);
        if (where === 'cell' && run.code === true && typeof run.text === 'string' && run.text.includes('\\|')) {
          error('Code in a Markdown table cell cannot hold a backslash before a pipe.', { rule: 'cellCodePipe' }, id);
        }
      }
    }
  };

  const fields = (raw: Rec, allowed: readonly string[], label: string, block: string, id: string | undefined) => {
    for (const key of Object.keys(raw)) {
      if (key === 'id' || key === 'type' || raw[key] === undefined || allowed.includes(key)) continue;
      error(`Markdown cannot hold "${key}" on ${label}.`, { field: key, block }, id);
    }
  };

  const blocks = (list: unknown) => {
    if (!Array.isArray(list)) return;
    for (const b of list) {
      if (!isRec(b)) continue;
      const type = String(b.type);
      const id = typeof b.id === 'string' ? b.id : undefined;
      const spec = MARKDOWN_BLOCKS[type];
      if (!spec) {
        error(`Markdown cannot hold "${type}" blocks.`, { block: type }, id);
        if (Array.isArray(b.blocks)) blocks(b.blocks);
        if (Array.isArray(b.columns)) for (const col of b.columns) if (isRec(col)) blocks(col.blocks);
        continue;
      }
      fields(b, spec.fields, `a ${type} block`, type, id);
      switch (type) {
        case 'paragraph':
        case 'heading':
          inlines(b.runs, id, 'text');
          break;
        case 'quote':
        case 'callout':
          blocks(b.blocks);
          break;
        case 'code':
          if (typeof b.language !== 'string' || !/^[^\s`]+$/.test(b.language)) error('A Markdown code language is one word without backticks.', { rule: 'codeLanguage' }, id);
          break;
        case 'image':
          if (typeof b.alt === 'string' && /[\r\n]/.test(b.alt)) error('A Markdown image alt is one line.', { rule: 'imageLine' }, id);
          if (typeof b.src === 'string' && /[\r\n]/.test(b.src)) error('A Markdown image source is one line.', { rule: 'imageLine' }, id);
          break;
        case 'list':
          if (typeof b.start === 'number' && b.start > 999_999_999) error('A Markdown list starts at 999999999 or less.', { rule: 'listStart' }, id);
          if (Array.isArray(b.items)) {
            for (const item of b.items) {
              if (!isRec(item)) continue;
              const itemId = typeof item.id === 'string' ? item.id : id;
              fields(item, MARKDOWN_ITEM_FIELDS, 'a list item', 'list', itemId);
              if (item.checked === true && b.style !== 'task') error('Only a task list item can be checked in Markdown.', { rule: 'checkedItem' }, itemId);
              inlines(item.runs, itemId, 'text');
              blocks(item.blocks);
            }
          }
          break;
        case 'table':
          if (b.headerRow !== true) error('A Markdown table needs a header row; its first row becomes one.', { rule: 'tableHeader' }, id);
          if (Array.isArray(b.rows)) {
            for (const row of b.rows) {
              if (!isRec(row)) continue;
              fields(row, MARKDOWN_ROW_FIELDS, 'a table row', 'table', id);
              if (Array.isArray(row.cells)) for (const cell of row.cells) inlines(cell, id, 'cell');
            }
          }
          break;
      }
    }
  };
  blocks(doc.blocks);
  return problems;
}
