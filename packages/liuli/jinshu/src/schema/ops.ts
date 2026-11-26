import {
  LIST_ITEM_FIELDS,
  RUN_MARK_FIELDS,
  blockFields,
  updatableFields,
  type FieldSpec,
} from './fields';
import {
  inlineKind,
  isInlineRun,
  inlineLength,
  newBlockId,
  paragraph,
  plainText,
  tableColumnCount,
  walkBlocks,
} from './helpers';
import {
  blockFingerprint,
  collectAtoms,
  inlinesToMarkup,
  parseAgentMarkup,
  parseInlineMarkup,
  toBlockMarkup,
} from './markup';
import { emptyRegistry, validateJsonSchema } from './registry';
import { checkDocument, checkFieldValue, isRecord, normalizeDocument, normalizeInlines, readOnlyReasons } from './validate';
import type {
  AnyJinshuOp,
  ApplyOpsOptions,
  ApplyOpsResult,
  BlockInput,
  CalloutBlock,
  CodeBlock,
  DetailsBlock,
  EditField,
  ExtensionBlock,
  HeadingBlock,
  ImageBlock,
  Inline,
  InlineAtom,
  InlineMarkKey,
  InlineRun,
  JinshuBlock,
  JinshuDocument,
  JinshuRegistry,
  JinshuTextOp,
  ListBlock,
  ListItem,
  ParagraphBlock,
  Position,
  Problem,
  TableAlign,
  TableBlock,
  TableRow,
  VideoBlock,
  CheckOptions,
} from './types';

function hydrateBlockInput(input: BlockInput, registry: JinshuRegistry): JinshuBlock {
  if (!isRecord(input) || typeof input.type !== 'string') return input as unknown as JinshuBlock;
  const id = input.id || newBlockId(input.type.slice(0, 4));
  const raw = { ...input, id } as Record<string, unknown>;
  if (Array.isArray(input.rows)) {
    raw.rows = input.rows.map(row => (isRecord(row) && !row.id ? { ...row, id: newBlockId('r') } : row));
  }
  if (Array.isArray(input.blocks)) {
    raw.blocks = input.blocks.map(b => hydrateBlockInput(b, registry));
  }
  if (Array.isArray(input.items)) {
    raw.items = input.items.map(item => (!isRecord(item) ? item : {
      ...item,
      id: item.id || newBlockId('li'),
      ...(Array.isArray(item.blocks) ? { blocks: item.blocks.map((b: BlockInput) => hydrateBlockInput(b, registry)) } : {}),
    }));
  }
  if (Array.isArray(input.columns)) {
    raw.columns = input.columns.map(col => (!isRecord(col) ? col : {
      ...col,
      id: col.id || newBlockId('col'),
      blocks: Array.isArray(col.blocks) ? col.blocks.map((b: BlockInput) => hydrateBlockInput(b, registry)) : col.blocks,
    }));
  }
  if (input.type.startsWith('x-')) {
    const spec = registry.blocks.get(input.type);
    const attrs = (input as { attrs?: Record<string, unknown> }).attrs ?? {};
    raw.version = input.version ?? spec?.version ?? 1;
    raw.attrs = attrs;
    raw.fallback = input.fallback ?? `[${input.type}]`;
  }
  return raw as unknown as JinshuBlock;
}

function resolveNewBlocks(
  op: { readonly markup?: string; readonly blocks?: readonly BlockInput[] },
  registry: JinshuRegistry,
  opIndex: number,
  existingAtoms: ReadonlyMap<string, InlineAtom>,
  check: CheckOptions | undefined,
): { blocks: JinshuBlock[]; problems: Problem[] } {
  if (op.markup !== undefined) {
    if (typeof op.markup !== 'string') {
      return { blocks: [], problems: [{ code: 'INVALID', severity: 'error', opIndex, message: 'markup must be a string.' }] };
    }
    const parsed = parseAgentMarkup(op.markup, { registry, existingAtoms, check });
    const problems = parsed.problems.map(p => ({ ...p, opIndex }));
    const hasError = problems.some(p => p.severity === 'error');
    if (hasError || parsed.blocks.length === 0) {
      if (!hasError && parsed.blocks.length === 0) {
        problems.push({ code: 'PARSE', severity: 'error', opIndex, message: 'Markup produced no blocks.' });
      }
      return { blocks: [], problems };
    }
    return { blocks: [...parsed.blocks], problems };
  }
  if (op.blocks !== undefined) {
    if (!Array.isArray(op.blocks) || op.blocks.length === 0) {
      return { blocks: [], problems: [{ code: 'INVALID', severity: 'error', opIndex, message: 'blocks must be a non-empty array.' }] };
    }
    return { blocks: op.blocks.map(b => hydrateBlockInput(b, registry)), problems: [] };
  }
  return { blocks: [], problems: [{ code: 'INVALID', severity: 'error', opIndex, message: 'Provide either markup or blocks.' }] };
}

interface BlockListRef {
  readonly list: JinshuBlock[];
  readonly index: number;
}

interface ListItemRef {
  readonly listBlock: ListBlock;
  readonly items: ListItem[];
  readonly index: number;
}

function cloneDoc(doc: JinshuDocument): JinshuDocument {
  return JSON.parse(JSON.stringify(doc)) as JinshuDocument;
}

function findBlockLocation(blocks: JinshuBlock[], id: string): BlockListRef | null {
  for (let i = 0; i < blocks.length; i++) {
    const b = blocks[i];
    if (b.id === id) return { list: blocks, index: i };
    if (b.type === 'quote' || b.type === 'callout' || b.type === 'details') {
      const found = findBlockLocation(b.blocks as JinshuBlock[], id);
      if (found) return found;
    } else if (b.type === 'list') {
      for (const item of b.items) {
        if (item.blocks) {
          const found = findBlockLocation(item.blocks as JinshuBlock[], id);
          if (found) return found;
        }
      }
    } else if (b.type === 'columns') {
      for (const col of b.columns) {
        const found = findBlockLocation(col.blocks as JinshuBlock[], id);
        if (found) return found;
      }
    } else if (b.type === 'audio' && b.transcript) {
      const found = findBlockLocation(b.transcript as JinshuBlock[], id);
      if (found) return found;
    } else if (b.type.startsWith('x-') && (b as ExtensionBlock).blocks) {
      const found = findBlockLocation((b as ExtensionBlock).blocks as JinshuBlock[], id);
      if (found) return found;
    }
  }
  return null;
}

function findListItemLocation(blocks: JinshuBlock[], itemId: string): ListItemRef | null {
  for (const b of blocks) {
    if (b.type === 'list') {
      const items = b.items as ListItem[];
      for (let i = 0; i < items.length; i++) {
        if (items[i].id === itemId) return { listBlock: b, items, index: i };
        if (items[i].blocks) {
          const found = findListItemLocation(items[i].blocks as JinshuBlock[], itemId);
          if (found) return found;
        }
      }
    } else if (b.type === 'quote' || b.type === 'callout' || b.type === 'details') {
      const found = findListItemLocation(b.blocks as JinshuBlock[], itemId);
      if (found) return found;
    } else if (b.type === 'columns') {
      for (const col of b.columns) {
        const found = findListItemLocation(col.blocks as JinshuBlock[], itemId);
        if (found) return found;
      }
    } else if (b.type.startsWith('x-') && (b as ExtensionBlock).blocks) {
      const found = findListItemLocation((b as ExtensionBlock).blocks as JinshuBlock[], itemId);
      if (found) return found;
    }
  }
  return null;
}

function findContainerChildren(doc: JinshuDocument, containerId: string | 'doc'): JinshuBlock[] | null {
  if (containerId === 'doc') return doc.blocks as JinshuBlock[];
  const loc = findBlockLocation(doc.blocks as JinshuBlock[], containerId);
  if (loc) {
    const b = loc.list[loc.index];
    if (b.type === 'quote' || b.type === 'callout' || b.type === 'details') return b.blocks as JinshuBlock[];
    if (b.type === 'audio') {
      const a = b as { transcript?: JinshuBlock[] };
      if (!a.transcript) a.transcript = [];
      return a.transcript;
    }
    if (b.type.startsWith('x-')) {
      const ext = b as { blocks?: JinshuBlock[] };
      if (!ext.blocks) ext.blocks = [];
      return ext.blocks;
    }
    return null;
  }
  // Check column id or list item id
  let colTarget: JinshuBlock[] | null = null;
  function scan(blocks: JinshuBlock[]): void {
    for (const b of blocks) {
      if (colTarget) return;
      if (b.type === 'columns') {
        for (const col of b.columns) {
          if (col.id === containerId) {
            colTarget = col.blocks as JinshuBlock[];
            return;
          }
          scan(col.blocks as JinshuBlock[]);
        }
      } else if (b.type === 'list') {
        for (const item of b.items) {
          if (item.id === containerId) {
            const mutable = item as { blocks?: JinshuBlock[] };
            if (!mutable.blocks) mutable.blocks = [];
            colTarget = mutable.blocks;
            return;
          }
          if (item.blocks) scan(item.blocks as JinshuBlock[]);
        }
      } else if (b.type === 'quote' || b.type === 'callout' || b.type === 'details') {
        scan(b.blocks as JinshuBlock[]);
      } else if (b.type.startsWith('x-') && (b as ExtensionBlock).blocks) {
        scan((b as ExtensionBlock).blocks as JinshuBlock[]);
      }
    }
  }
  scan(doc.blocks as JinshuBlock[]);
  return colTarget;
}

function insertAtPosition(
  doc: JinshuDocument,
  at: Position,
  newBlocks: readonly JinshuBlock[],
  opIndex: number,
): Problem | null {
  if ('before' in at || 'after' in at) {
    const targetId = 'before' in at ? at.before : at.after;
    const isBefore = 'before' in at;
    const loc = findBlockLocation(doc.blocks as JinshuBlock[], targetId);
    if (loc) {
      loc.list.splice(isBefore ? loc.index : loc.index + 1, 0, ...newBlocks);
      return null;
    }
    // Check if targeting a list item `<li id="...">`
    const liLoc = findListItemLocation(doc.blocks as JinshuBlock[], targetId);
    if (liLoc) {
      // If inserting a list block, splice its items; otherwise convert paragraphs to list items
      const toInsert: ListItem[] = [];
      for (const b of newBlocks) {
        if (b.type === 'list') toInsert.push(...b.items);
        else if (b.type === 'paragraph') toInsert.push({ id: b.id, runs: b.runs });
        else {
          return {
            code: 'INVALID',
            severity: 'error',
            opIndex,
            id: targetId,
            message: `Cannot insert block of type "${b.type}" directly as a sibling of list item "${targetId}".`,
          };
        }
      }
      liLoc.items.splice(isBefore ? liLoc.index : liLoc.index + 1, 0, ...toInsert);
      return null;
    }
    return { code: 'NOT_FOUND', severity: 'error', opIndex, id: targetId, message: `Target id "${targetId}" not found.` };
  }

  const containerId = 'start' in at ? at.start : at.end;
  const container = findContainerChildren(doc, containerId);
  if (!container) {
    return { code: 'NOT_FOUND', severity: 'error', opIndex, id: containerId, message: `Container "${containerId}" not found or cannot hold child blocks.` };
  }
  if ('start' in at) container.unshift(...newBlocks);
  else container.push(...newBlocks);
  return null;
}

function getInlineField(
  doc: JinshuDocument,
  id: string,
  field: EditField | undefined,
): { inlines: readonly Inline[]; inHeading: boolean; set: (next: readonly Inline[]) => void } | null {
  const loc = findBlockLocation(doc.blocks as JinshuBlock[], id);
  if (!loc) {
    // Check list item
    const liLoc = findListItemLocation(doc.blocks as JinshuBlock[], id);
    if (liLoc && (!field || field === 'runs')) {
      return {
        inlines: liLoc.items[liLoc.index].runs,
        inHeading: false,
        set: next => {
          liLoc.items[liLoc.index] = { ...liLoc.items[liLoc.index], runs: next };
        },
      };
    }
    return null;
  }

  const blk = loc.list[loc.index];
  const f = field ?? 'runs';
  if (f === 'runs' && blk.type.startsWith('x-') && (blk as ExtensionBlock).blocks === undefined) {
    const ext = blk as ExtensionBlock;
    return {
      inlines: ext.runs ?? [],
      inHeading: false,
      set: next => {
        loc.list[loc.index] = { ...ext, runs: next } as ExtensionBlock;
      },
    };
  }
  if (f === 'runs' && (blk.type === 'paragraph' || blk.type === 'heading')) {
    return {
      inlines: blk.runs,
      inHeading: blk.type === 'heading',
      set: next => {
        loc.list[loc.index] = { ...blk, runs: next } as ParagraphBlock | HeadingBlock;
      },
    };
  }
  if (f === 'title' && blk.type === 'callout') {
    return {
      inlines: blk.title ?? [],
      inHeading: false,
      set: next => {
        loc.list[loc.index] = { ...blk, title: next } as CalloutBlock;
      },
    };
  }
  if (f === 'summary' && blk.type === 'details') {
    return {
      inlines: blk.summary,
      inHeading: false,
      set: next => {
        loc.list[loc.index] = { ...blk, summary: next } as DetailsBlock;
      },
    };
  }
  if (f === 'caption' && (blk.type === 'image' || blk.type === 'video')) {
    return {
      inlines: blk.caption ?? [],
      inHeading: false,
      set: next => {
        loc.list[loc.index] = { ...blk, caption: next } as ImageBlock | VideoBlock;
      },
    };
  }
  if (typeof f === 'object' && blk.type === 'table') {
    const cell = (f as { cell?: readonly [unknown, unknown] }).cell;
    if (!Array.isArray(cell)) return null;
    const [rowId, c] = cell;
    const r = blk.rows.findIndex(row => row.id === rowId);
    const row = blk.rows[r];
    if (!row || typeof c !== 'number' || c < 0 || c >= row.cells.length) return null;
    return {
      inlines: row.cells[c],
      inHeading: false,
      set: next => {
        const rows = blk.rows.map((rr, i) => (i === r ? { ...rr, cells: rr.cells.map((cell, j) => (j === c ? next : cell)) } : rr));
        loc.list[loc.index] = { ...blk, rows } as TableBlock;
      },
    };
  }
  return null;
}

/** Checks whether UTF-16 offset `at` splits a surrogate pair in `str`. */
export function isSurrogateSplit(str: string, at: number): boolean {
  if (at <= 0 || at >= str.length) return false;
  const prev = str.charCodeAt(at - 1);
  const next = str.charCodeAt(at);
  return prev >= 0xd800 && prev <= 0xdbff && next >= 0xdc00 && next <= 0xdfff;
}

/** Checks whether UTF-16 inline offset `at` splits a surrogate pair inside `inlines`. */
export function isInlineSurrogateSplit(inlines: readonly Inline[], at: number): boolean {
  if (at <= 0) return false;
  let cursor = 0;
  for (const item of inlines) {
    if (isInlineRun(item)) {
      const end = cursor + item.text.length;
      if (at > cursor && at < end) {
        return isSurrogateSplit(item.text, at - cursor);
      }
      cursor = end;
    } else {
      cursor += 1;
    }
  }
  return false;
}

/** Slices `inlines` at UTF-16 code unit offset `at` into `[before, after]`. */
export function splitInlinesAt(inlines: readonly Inline[], at: number): [Inline[], Inline[]] {
  const before: Inline[] = [];
  const after: Inline[] = [];
  let cursor = 0;

  for (const item of inlines) {
    if (isInlineRun(item)) {
      const end = cursor + item.text.length;
      if (end <= at) {
        before.push(item);
      } else if (cursor >= at) {
        after.push(item);
      } else {
        const cut = at - cursor;
        if (cut > 0) before.push({ ...item, text: item.text.slice(0, cut) });
        if (cut < item.text.length) after.push({ ...item, text: item.text.slice(cut) });
      }
      cursor = end;
    } else {
      if (cursor < at) before.push(item);
      else after.push(item);
      cursor += 1;
    }
  }
  return [before, after];
}

function spliceInlines(
  inlines: readonly Inline[],
  from: number,
  to: number,
  insert: readonly Inline[],
  inHeading: boolean,
): Inline[] {
  const [prefix] = splitInlinesAt(inlines, Math.max(0, from));
  const [, suffix] = splitInlinesAt(inlines, Math.max(from, to));
  return normalizeInlines([...prefix, ...insert, ...suffix], { inHeading });
}

function applyMarksToInlines(
  inlines: readonly Inline[],
  from: number,
  to: number,
  add: Partial<Omit<InlineRun, 'text'>> | undefined,
  remove: readonly InlineMarkKey[] | undefined,
  inHeading: boolean,
): Inline[] {
  const [prefix, rest] = splitInlinesAt(inlines, Math.max(0, from));
  const [middle, suffix] = splitInlinesAt(rest, Math.max(0, to - from));
  const updatedMiddle = middle.map(item => {
    if (!isInlineRun(item)) return item;
    const next: Record<string, unknown> = { ...item };
    if (remove) {
      for (const k of remove) delete next[k];
    }
    if (add) {
      for (const [k, v] of Object.entries(add)) {
        if (v !== undefined) next[k] = v;
      }
    }
    return next as unknown as InlineRun;
  });
  return normalizeInlines([...prefix, ...updatedMiddle, ...suffix], { inHeading });
}

/** Plain-text projection of inlines; offsets equal inline offsets (a break is `\n`, math and atoms one U+FFFC). */
export function inlinePlainProjection(inlines: readonly Inline[]): string {
  let out = '';
  for (const item of inlines) {
    const kind = inlineKind(item);
    if (kind === 'text') out += (item as InlineRun).text;
    else if (kind === 'break') out += '\n';
    else out += '￼';
  }
  return out;
}

function findAll(haystack: string, needle: string): number[] {
  const out: number[] = [];
  if (!needle) return out;
  let pos = 0;
  while ((pos = haystack.indexOf(needle, pos)) >= 0) {
    out.push(pos);
    pos += needle.length;
  }
  return out;
}

/** Marks of the text run holding the character at `offset` (none when it is not inside a run). */
function marksAt(inlines: readonly Inline[], offset: number): Record<string, unknown> {
  let cursor = 0;
  for (const item of inlines) {
    if (inlineKind(item) === 'text') {
      const run = item as InlineRun;
      const end = cursor + run.text.length;
      if (offset >= cursor && offset < end) {
        const { text: _text, ...marks } = run as unknown as Record<string, unknown>;
        return marks;
      }
      cursor = end;
    } else {
      if (offset === cursor) return {};
      cursor += 1;
    }
  }
  return {};
}

function isPlainRuns(runs: readonly Inline[]): boolean {
  return runs.every(r => inlineKind(r) === 'text' && Object.keys(r).every(k => k === 'text'));
}

type ConvertResult = { readonly block: JinshuBlock } | { readonly lost: string };

/** The block's single inline content, or what converting it to text would lose. */
function singleInlineContent(blk: JinshuBlock): { runs: readonly Inline[] } | { lost: string } {
  switch (blk.type) {
    case 'paragraph':
    case 'heading':
      return { runs: blk.runs };
    case 'code':
      if (blk.code.includes('\n')) return { lost: 'the code\'s line breaks' };
      return { runs: blk.code ? [{ text: blk.code }] : [] };
    case 'list': {
      if (blk.items.length !== 1) return { lost: `${blk.items.length - 1} other list item(s)` };
      const item = blk.items[0];
      if (item.blocks && item.blocks.length > 0) return { lost: 'the list item\'s nested blocks' };
      if (item.checked) return { lost: 'the task\'s checked state' };
      return { runs: item.runs };
    }
    case 'quote':
    case 'callout': {
      if (blk.type === 'callout' && blk.title && blk.title.length > 0) return { lost: 'the callout title' };
      if (blk.blocks.length !== 1 || blk.blocks[0].type !== 'paragraph') return { lost: `the ${blk.type}'s child blocks other than one paragraph` };
      return { runs: blk.blocks[0].runs };
    }
    default:
      return { lost: `the ${blk.type} block's content` };
  }
}

/** Type conversion, allowed only when nothing would be lost. Keeps the block id. */
function convertBlock(blk: JinshuBlock, op: Extract<JinshuTextOp, { op: 'setType' }>): ConvertResult {
  const id = blk.id;
  // Same-family conversions keep the structure.
  if (op.type === 'list' && blk.type === 'list') {
    const style = op.style ?? blk.style;
    if (style !== 'task' && blk.items.some(i => i.checked)) return { lost: 'the tasks\' checked state' };
    return { block: { ...blk, style } };
  }
  if ((op.type === 'quote' || op.type === 'callout') && (blk.type === 'quote' || blk.type === 'callout')) {
    if (op.type === 'quote' && blk.type === 'callout' && blk.title && blk.title.length > 0) return { lost: 'the callout title' };
    return {
      block: op.type === 'quote'
        ? { id, type: 'quote', blocks: blk.blocks }
        : { ...(blk.type === 'callout' ? blk : {}), id, type: 'callout', tone: op.tone ?? (blk.type === 'callout' ? blk.tone : 'info'), blocks: blk.blocks },
    };
  }
  if (op.type === 'code' && blk.type === 'code') return { block: { ...blk, language: op.language ?? blk.language } };

  const content = singleInlineContent(blk);
  if ('lost' in content) return content;
  const runs = content.runs;
  const childId = op.childId || newBlockId('p');
  switch (op.type) {
    case 'paragraph':
      return { block: { id, type: 'paragraph', runs } };
    case 'heading': {
      if (JSON.stringify(normalizeInlines(runs, { inHeading: true })) !== JSON.stringify(normalizeInlines(runs))) {
        return { lost: 'line breaks, bold or sub/superscript marks (headings drop them)' };
      }
      return { block: { id, type: 'heading', level: op.level ?? (blk.type === 'heading' ? blk.level : 2), runs } };
    }
    case 'quote':
      return { block: { id, type: 'quote', blocks: [{ id: childId, type: 'paragraph', runs }] } };
    case 'callout':
      return { block: { id, type: 'callout', tone: op.tone ?? 'info', blocks: [{ id: childId, type: 'paragraph', runs }] } };
    case 'list':
      return { block: { id, type: 'list', style: op.style ?? 'bullet', items: [{ id: childId, runs }] } };
    case 'code': {
      if (!runs.every(r => inlineKind(r) === 'break' || isPlainRuns([r]))) {
        return { lost: 'inline formatting, links, math or atoms (code is plain text)' };
      }
      return { block: { id, type: 'code', language: op.language ?? 'text', code: inlinePlainProjection(runs) } };
    }
    default:
      return { lost: `conversion to "${String(op.type)}" is not supported` };
  }
}

function rejected(doc: JinshuDocument, problems: Problem[]): ApplyOpsResult {
  return { doc, problems };
}

/**
 * Atomically applies a batch of `JinshuOp | JinshuTextOp` to a document.
 * If any operation fails or post-validation finds errors, returns the original `doc` unchanged with `problems`.
 * Never throws; a document newer than this code (problem `VERSION`) is read-only.
 */
export function applyOps(
  doc: JinshuDocument,
  ops: readonly AnyJinshuOp[],
  registry: JinshuRegistry = emptyRegistry,
  options?: ApplyOpsOptions,
): ApplyOpsResult {
  const problems: Problem[] = [];
  let opIndex = -1;
  try {
    const readOnly = readOnlyReasons(doc, registry);
    if (readOnly.length > 0) return rejected(doc, readOnly);
    if (!Array.isArray(ops)) {
      return rejected(doc, [{ code: 'INVALID', severity: 'error', message: 'ops must be an array.' }]);
    }
    const working = cloneDoc(doc);
    for (opIndex = 0; opIndex < ops.length; opIndex++) {
      const op = ops[opIndex];
      if (!isRecord(op) || typeof op.op !== 'string') {
        problems.push({ code: 'INVALID', severity: 'error', opIndex, message: 'Each op must be an object with an "op" name.' });
        return rejected(doc, problems);
      }
      const failure = applyOne(working, op as unknown as AnyJinshuOp, opIndex, registry, options, problems);
      if (failure) return rejected(doc, problems);
    }

    const normalized = normalizeDocument(working, registry);
    problems.push(...checkDocument(normalized, registry, options));
    if (problems.some(p => p.severity === 'error') || options?.dryRun) return rejected(doc, problems);
    return { doc: normalized, problems };
  } catch (err) {
    problems.push({
      code: 'INVALID',
      severity: 'error',
      ...(opIndex >= 0 ? { opIndex } : {}),
      message: `Malformed op: ${err instanceof Error ? err.message : String(err)}`,
    });
    return rejected(doc, problems);
  }
}

const STRUCTURAL_HINT = 'use replace (or text ops) to change content and structure';

/** Checks an `update` op's attrs against the target's field table before anything is applied. */
function checkUpdateAttrs(
  attrs: Readonly<Record<string, unknown>>,
  fields: readonly FieldSpec[],
  label: string,
  opIndex: number,
  id: string,
  options: ApplyOpsOptions | undefined,
): Problem[] {
  const out: Problem[] = [];
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined) continue;
    const spec = fields.find(f => f.name === key);
    if (!spec) {
      const allowed = updatableFields(fields).map(f => f.name);
      out.push({
        code: 'UNKNOWN_FIELD',
        severity: 'error',
        opIndex,
        id,
        message: `${label} has no field "${key}"${allowed.length > 0 ? `; updatable fields: ${allowed.join(', ')}` : ''}.`,
      });
      continue;
    }
    if (spec.structural) {
      out.push({ code: 'UNKNOWN_FIELD', severity: 'error', opIndex, id, message: `"${key}" cannot be set through update; ${STRUCTURAL_HINT}.` });
      continue;
    }
    if (value === null) {
      if (spec.required) out.push({ code: 'INVALID', severity: 'error', opIndex, id, message: `Required field "${key}" of ${label} cannot be removed.` });
      continue;
    }
    const err = checkFieldValue(value, spec, options);
    if (err) out.push({ code: 'INVALID', severity: 'error', opIndex, id, message: `${label}: ${err}.` });
  }
  return out;
}

function mergeAttrs(base: Record<string, unknown>, attrs: Readonly<Record<string, unknown>>): Record<string, unknown> {
  const next = { ...base };
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined) continue;
    if (v === null) delete next[k];
    else next[k] = v;
  }
  return next;
}

function resolveRowIndex(rows: readonly TableRow[], rowId: unknown): number {
  return typeof rowId === 'string' ? rows.findIndex(r => r.id === rowId) : -1;
}

/** Applies one op to `working` in place. Returns true when it failed (problems already pushed). */
function applyOne(
  working: JinshuDocument,
  op: AnyJinshuOp,
  opIndex: number,
  registry: JinshuRegistry,
  options: ApplyOpsOptions | undefined,
  problems: Problem[],
): boolean {
  const fail = (p: Omit<Problem, 'opIndex' | 'severity'> & { severity?: Problem['severity'] }) => {
    problems.push({ severity: 'error', opIndex, ...p });
    return true;
  };
  const blocks = working.blocks as JinshuBlock[];

  switch (op.op) {
    case 'insert': {
      const res = resolveNewBlocks(op, registry, opIndex, collectAtoms(blocks), options);
      problems.push(...res.problems);
      if (res.problems.some(p => p.severity === 'error')) return true;
      if (!isRecord(op.at)) return fail({ code: 'INVALID', message: 'insert needs at: { before } | { after } | { start } | { end }.' });
      const err = insertAtPosition(working, op.at, res.blocks, opIndex);
      if (err) {
        problems.push(err);
        return true;
      }
      return false;
    }

    case 'replace': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc) return fail({ code: 'NOT_FOUND', id: op.id, message: `Block "${op.id}" not found.` });
      const res = resolveNewBlocks(op, registry, opIndex, collectAtoms(blocks), options);
      problems.push(...res.problems);
      if (res.problems.some(p => p.severity === 'error')) return true;
      // First replacement block inherits the original `id`; remaining blocks keep new ids.
      const replaced = res.blocks.map((b, idx) => (idx === 0 ? { ...b, id: op.id } : b));
      loc.list.splice(loc.index, 1, ...replaced);
      return false;
    }

    case 'edit': {
      if (typeof op.find !== 'string' || op.find.length === 0) {
        return fail({ code: 'INVALID', id: op.id, message: 'edit needs a non-empty "find" string.' });
      }
      if (typeof op.replace !== 'string') return fail({ code: 'INVALID', id: op.id, message: 'edit needs a "replace" string.' });
      const loc = findBlockLocation(blocks, op.id);
      const block = loc ? loc.list[loc.index] : undefined;

      // Code block plain-text edit (supports optional 1-based inclusive lineRange)
      const isCodeEdit = block?.type === 'code' && (op.field === undefined || op.field === 'code');
      if (op.lineRange && !isCodeEdit) {
        return fail({ code: 'INVALID', id: op.id, message: 'lineRange is only valid for field: "code" on a code block.' });
      }

      if (isCodeEdit && block && loc) {
        const codeBlk = block as CodeBlock;
        const current = toBlockMarkup(codeBlk, registry, '', { includeLineNumbers: true });
        const lines = codeBlk.code.split('\n');
        let targetText = codeBlk.code;
        let startLine = 1;
        let endLine = lines.length;
        if (op.lineRange) {
          [startLine, endLine] = op.lineRange;
          if (!Number.isInteger(startLine) || !Number.isInteger(endLine) || startLine < 1 || endLine < startLine || endLine > lines.length) {
            return fail({ code: 'INVALID', id: op.id, message: `lineRange [${startLine}, ${endLine}] out of bounds (code has ${lines.length} lines).`, current });
          }
          targetText = lines.slice(startLine - 1, endLine).join('\n');
        }
        const hits = findAll(targetText, op.find).length;
        if (hits === 0) return fail({ code: 'NO_MATCH', id: op.id, message: `Quote not found in code block "${op.id}".`, current });
        if (hits > 1 && !op.all) {
          return fail({ code: 'AMBIGUOUS_MATCH', id: op.id, message: `Quote matched ${hits} times in code block "${op.id}". Pass a longer quote, lineRange, or all: true.`, current });
        }
        const updatedSegment = op.all ? targetText.split(op.find).join(op.replace) : targetText.replace(op.find, () => op.replace);
        const nextCode = op.lineRange
          ? [...lines.slice(0, startLine - 1), updatedSegment, ...lines.slice(endLine)].join('\n')
          : updatedSegment;
        loc.list[loc.index] = { ...codeBlk, code: nextCode };
        return false;
      }

      // Inline field: match plain text, splice parsed inline markup back at run offsets.
      const fieldRef = getInlineField(working, op.id, op.field);
      if (!fieldRef) return fail({ code: 'NOT_FOUND', id: op.id, message: `Editable field not found on "${op.id}".` });
      const current = block ? toBlockMarkup(block, registry) : inlinesToMarkup(fieldRef.inlines);
      const projection = inlinePlainProjection(fieldRef.inlines);
      const matches = findAll(projection, op.find);
      if (matches.length === 0) {
        return fail({ code: 'NO_MATCH', id: op.id, message: `Text "${op.find.slice(0, 40)}" not found in "${op.id}" (find matches the plain text, not markup).`, current });
      }
      if (matches.length > 1 && !op.all) {
        return fail({ code: 'AMBIGUOUS_MATCH', id: op.id, message: `Text matched ${matches.length} times in "${op.id}". Provide more context or set all: true.`, current });
      }
      const parsed = parseInlineMarkup(op.replace, { existingAtoms: collectAtoms(blocks), inHeading: fieldRef.inHeading, registry, check: options });
      const parseErrors = parsed.problems.filter(p => p.severity === 'error');
      if (parseErrors.length > 0) {
        problems.push(...parseErrors.map(p => ({ ...p, opIndex, id: op.id, current })));
        return true;
      }
      const plain = isPlainRuns(parsed.runs);
      let next: readonly Inline[] = fieldRef.inlines;
      for (let m = matches.length - 1; m >= 0; m--) {
        const start = matches[m];
        const marks = plain ? marksAt(next, start) : {};
        const insert = plain ? parsed.runs.map(r => ({ ...marks, text: (r as InlineRun).text }) as InlineRun) : parsed.runs;
        next = spliceInlines(next, start, start + op.find.length, insert, fieldRef.inHeading);
      }
      fieldRef.set(next);
      return false;
    }

    case 'update': {
      if (!isRecord(op.attrs)) return fail({ code: 'INVALID', id: op.id, message: 'update needs an "attrs" object.' });
      const loc = findBlockLocation(blocks, op.id);
      if (!loc) {
        const liLoc = findListItemLocation(blocks, op.id);
        if (!liLoc) return fail({ code: 'NOT_FOUND', id: op.id, message: `Block or list item "${op.id}" not found.` });
        const errs = checkUpdateAttrs(op.attrs, LIST_ITEM_FIELDS, 'list item', opIndex, op.id, options);
        if (errs.length > 0) {
          problems.push(...errs);
          return true;
        }
        liLoc.items[liLoc.index] = mergeAttrs(liLoc.items[liLoc.index] as unknown as Record<string, unknown>, op.attrs) as unknown as ListItem;
        return false;
      }
      const blk = loc.list[loc.index];
      if (typeof blk.type === 'string' && blk.type.startsWith('x-')) {
        const ext = blk as ExtensionBlock;
        const spec = registry.blocks.get(ext.type);
        const known = spec?.attrsSchema.properties;
        if (known && spec?.attrsSchema.additionalProperties === false) {
          for (const key of Object.keys(op.attrs)) {
            if (!(key in known)) {
              return fail({ code: 'UNKNOWN_FIELD', id: op.id, message: `${ext.type} has no attribute "${key}"; attributes: ${Object.keys(known).join(', ')}.` });
            }
          }
        }
        const nextAttrs = mergeAttrs(isRecord(ext.attrs) ? ext.attrs : {}, op.attrs);
        if (spec) {
          const errs = validateJsonSchema(nextAttrs, spec.attrsSchema);
          if (errs.length > 0) return fail({ code: 'INVALID', id: op.id, message: `${ext.type}: ${errs.join('; ')}` });
        }
        loc.list[loc.index] = { ...ext, attrs: nextAttrs };
        return false;
      }
      const fields = blockFields(blk.type);
      if (!fields) return fail({ code: 'INVALID', id: op.id, message: `Block "${op.id}" has unknown type "${String(blk.type)}".` });
      const errs = checkUpdateAttrs(op.attrs, fields, `"${blk.type}" block`, opIndex, op.id, options);
      if (errs.length > 0) {
        problems.push(...errs);
        return true;
      }
      loc.list[loc.index] = mergeAttrs(blk as unknown as Record<string, unknown>, op.attrs) as unknown as JinshuBlock;
      return false;
    }

    case 'tableRow': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc || loc.list[loc.index].type !== 'table') return fail({ code: 'NOT_FOUND', id: op.id, message: `Table block "${op.id}" not found.` });
      const tbl = loc.list[loc.index] as TableBlock;
      const cols = tableColumnCount(tbl);
      const rows = [...tbl.rows];
      const anchorId = op.before ?? op.after;
      const anchorIndex = () => {
        const i = resolveRowIndex(rows, anchorId);
        return i < 0 ? -1 : op.before !== undefined ? i : i + 1;
      };
      if (op.action === 'insert') {
        if (anchorId !== undefined && resolveRowIndex(rows, anchorId) < 0) {
          return fail({ code: 'NOT_FOUND', id: op.id, message: `Row "${String(anchorId)}" not found in table "${op.id}".` });
        }
        if (op.cells !== undefined && (!Array.isArray(op.cells) || op.cells.some(c => !Array.isArray(c)))) {
          return fail({ code: 'INVALID', id: op.id, message: 'tableRow cells must be an array of Inline[] cells.' });
        }
        const newRow: TableRow = {
          id: op.rowId ?? newBlockId('r'),
          cells: Array.from({ length: cols }, (_, c) => op.cells?.[c] ?? []),
        };
        rows.splice(anchorId === undefined ? rows.length : anchorIndex(), 0, newRow);
      } else if (op.action === 'delete' || op.action === 'move') {
        const from = resolveRowIndex(rows, op.rowId);
        if (from < 0) return fail({ code: 'NOT_FOUND', id: op.id, message: `Row "${String(op.rowId)}" not found in table "${op.id}".` });
        if (op.action === 'delete') {
          if (rows.length <= 1) return fail({ code: 'INVALID', id: op.id, message: 'Cannot delete the last row of a table; delete the table instead.' });
          rows.splice(from, 1);
        } else {
          if (anchorId === undefined || anchorId === op.rowId) {
            return fail({ code: 'INVALID', id: op.id, message: 'tableRow move needs before or after naming another row.' });
          }
          const [moved] = rows.splice(from, 1);
          const to = anchorIndex();
          if (to < 0) return fail({ code: 'NOT_FOUND', id: op.id, message: `Row "${String(anchorId)}" not found in table "${op.id}".` });
          rows.splice(to, 0, moved);
        }
      } else {
        return fail({ code: 'INVALID', id: op.id, message: `Unknown tableRow action "${String(op.action)}".` });
      }
      loc.list[loc.index] = { ...tbl, rows };
      return false;
    }

    case 'tableCol': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc || loc.list[loc.index].type !== 'table') return fail({ code: 'NOT_FOUND', id: op.id, message: `Table block "${op.id}" not found.` });
      const tbl = loc.list[loc.index] as TableBlock;
      const cols = tableColumnCount(tbl);
      const rows = tbl.rows.map(r => ({ ...r, cells: [...r.cells] }));
      let align: TableAlign[] | undefined = tbl.align ? [...tbl.align] : undefined;
      let widths: number[] | undefined = tbl.widths ? [...tbl.widths] : undefined;
      const index = op.index;
      if (!Number.isInteger(index)) return fail({ code: 'INVALID', id: op.id, message: 'tableCol needs an integer index.' });
      const move = <T>(arr: T[], from: number, to: number) => {
        const [x] = arr.splice(from, 1);
        arr.splice(to, 0, x);
      };

      if (op.action === 'insert') {
        if (index < 0 || index > cols) return fail({ code: 'INVALID', id: op.id, message: `Column index ${index} out of bounds.` });
        rows.forEach((row, r) => row.cells.splice(index, 0, r === 0 && tbl.headerRow ? op.header ?? [] : []));
        // A table without alignments or widths gets them when the new column names one; the rest keep the defaults.
        if (align || op.align !== undefined) {
          const next = align ?? Array.from({ length: cols }, () => 'start' as const);
          next.splice(index, 0, op.align ?? 'start');
          align = next;
        }
        if (widths || op.width !== undefined) {
          const next = widths ?? Array.from({ length: cols }, () => 1);
          next.splice(index, 0, op.width ?? 1);
          widths = next;
        }
      } else if (op.action === 'delete') {
        if (cols <= 1) return fail({ code: 'INVALID', id: op.id, message: 'Cannot delete the last column of a table.' });
        if (index < 0 || index >= cols) return fail({ code: 'INVALID', id: op.id, message: `Column index ${index} out of bounds.` });
        for (const row of rows) row.cells.splice(index, 1);
        if (align) align.splice(index, 1);
        if (widths) widths.splice(index, 1);
      } else if (op.action === 'move') {
        const to = op.toIndex;
        if (to === undefined || !Number.isInteger(to) || index < 0 || index >= cols || to < 0 || to >= cols) {
          return fail({ code: 'INVALID', id: op.id, message: `Invalid tableCol move indices (${index} -> ${String(to)}).` });
        }
        for (const row of rows) move(row.cells, index, to);
        if (align) move(align, index, to);
        if (widths) move(widths, index, to);
      } else {
        return fail({ code: 'INVALID', id: op.id, message: `Unknown tableCol action "${String(op.action)}".` });
      }
      loc.list[loc.index] = { ...tbl, rows, ...(align ? { align } : {}), ...(widths ? { widths } : {}) };
      return false;
    }

    case 'delete': {
      const loc = findBlockLocation(blocks, op.id);
      if (loc) {
        loc.list.splice(loc.index, 1);
        if (working.blocks.length === 0) blocks.push(paragraph());
        return false;
      }
      const liLoc = findListItemLocation(blocks, op.id);
      if (liLoc) {
        if (liLoc.items.length <= 1) {
          return fail({ code: 'INVALID', id: op.id, message: 'Cannot delete the last item of a list; delete the list block itself instead.' });
        }
        liLoc.items.splice(liLoc.index, 1);
        return false;
      }
      return fail({ code: 'NOT_FOUND', id: op.id, message: `Id "${op.id}" not found.` });
    }

    case 'move': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc) return fail({ code: 'NOT_FOUND', id: op.id, message: `Block "${op.id}" not found.` });
      if (!isRecord(op.to)) return fail({ code: 'INVALID', id: op.id, message: 'move needs to: { before } | { after } | { start } | { end }.' });
      const [moved] = loc.list.splice(loc.index, 1);
      const err = insertAtPosition(working, op.to, [moved], opIndex);
      if (err) {
        problems.push(err);
        return true;
      }
      return false;
    }

    case 'setMeta': {
      const mut = working as { title?: unknown; lang?: unknown; meta?: Record<string, unknown> };
      if (op.title !== undefined && typeof op.title !== 'string') return fail({ code: 'INVALID', message: 'setMeta title must be a string.' });
      if (op.lang !== undefined && typeof op.lang !== 'string') return fail({ code: 'INVALID', message: 'setMeta lang must be a string.' });
      if (op.meta !== undefined && !isRecord(op.meta)) return fail({ code: 'INVALID', message: 'setMeta meta must be an object.' });
      if (op.title !== undefined) mut.title = op.title;
      if (op.lang !== undefined) mut.lang = op.lang;
      if (op.meta !== undefined) {
        // Merged key by key; a `null` value removes the key.
        const meta: Record<string, unknown> = { ...(mut.meta ?? {}) };
        for (const [key, value] of Object.entries(op.meta)) {
          if (value === null) delete meta[key];
          else meta[key] = value;
        }
        mut.meta = meta;
      }
      return false;
    }

    case 'rewrite': {
      if (typeof op.markup !== 'string') return fail({ code: 'INVALID', message: 'rewrite needs a markup string.' });
      const parsed = parseAgentMarkup(op.markup, { registry, existingAtoms: collectAtoms(blocks), check: options });
      problems.push(...parsed.problems.map(p => ({ ...p, opIndex })));
      if (parsed.problems.some(p => p.severity === 'error')) return true;
      // Align blocks without explicit IDs by content fingerprint
      const oldById = new Map<string, JinshuBlock>();
      walkBlocks(working.blocks, b => oldById.set(b.id, b));
      const oldByFingerprint = new Map<string, string[]>();
      for (const oldBlk of working.blocks) {
        const fp = blockFingerprint(oldBlk);
        const list = oldByFingerprint.get(fp) ?? [];
        list.push(oldBlk.id);
        oldByFingerprint.set(fp, list);
      }
      const usedIds = new Set<string>();
      const aligned = parsed.blocks.map(b => {
        if (oldById.has(b.id) && !usedIds.has(b.id)) {
          usedIds.add(b.id);
          return b;
        }
        const candidates = oldByFingerprint.get(blockFingerprint(b));
        if (candidates && candidates.length > 0) {
          const reuse = candidates.shift()!;
          if (!usedIds.has(reuse)) {
            usedIds.add(reuse);
            return { ...b, id: reuse };
          }
        }
        usedIds.add(b.id);
        return b;
      }).map(b => {
        // Markup has no spelling for unknown (newer) fields; keep those of the block it rewrites.
        const old = oldById.get(b.id);
        if (!old || old.type !== b.type) return b;
        const fields = blockFields(old.type);
        if (!fields) return b;
        const known = new Set(fields.map(f => f.name));
        const extra: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(old)) if (!known.has(k)) extra[k] = v;
        return { ...extra, ...b } as JinshuBlock;
      });
      const mut = working as unknown as { title?: string; lang?: string; id?: string; meta?: Readonly<Record<string, unknown>>; blocks: JinshuBlock[] };
      if (parsed.title !== undefined) mut.title = parsed.title;
      if (parsed.lang !== undefined) mut.lang = parsed.lang;
      if (parsed.id !== undefined) mut.id = parsed.id;
      if (parsed.meta !== undefined) mut.meta = parsed.meta;
      mut.blocks = aligned.length > 0 ? aligned : [paragraph()];
      return false;
    }

    // ====================================================================
    // Editor fine-grained JinshuTextOp
    // ====================================================================
    case 'text': {
      if (!isRecord(op.target)) return fail({ code: 'INVALID', message: 'text op needs a target.' });
      if (!Number.isInteger(op.from) || !Number.isInteger(op.to) || op.from < 0 || op.to < op.from) {
        return fail({ code: 'INVALID', id: op.target.blockId, message: `Invalid text range [${op.from}, ${op.to}].` });
      }
      if (!Array.isArray(op.insert)) return fail({ code: 'INVALID', id: op.target.blockId, message: 'text op insert must be an Inline[] array.' });
      const loc = findBlockLocation(blocks, op.target.blockId);
      const blk = loc ? loc.list[loc.index] : undefined;
      if (blk?.type === 'code' && (op.target.field === undefined || op.target.field === 'code') && loc) {
        if (isSurrogateSplit(blk.code, op.from) || isSurrogateSplit(blk.code, op.to)) {
          return fail({ code: 'INVALID', id: op.target.blockId, message: 'Text offset splits a UTF-16 surrogate pair.' });
        }
        loc.list[loc.index] = { ...blk, code: blk.code.slice(0, op.from) + plainText(op.insert) + blk.code.slice(op.to) };
        return false;
      }
      const fieldRef = getInlineField(working, op.target.blockId, op.target.field);
      if (!fieldRef) return fail({ code: 'NOT_FOUND', id: op.target.blockId, message: `Text target "${op.target.blockId}" not found.` });
      if (isInlineSurrogateSplit(fieldRef.inlines, op.from) || isInlineSurrogateSplit(fieldRef.inlines, op.to)) {
        return fail({ code: 'INVALID', id: op.target.blockId, message: 'Text offset splits a UTF-16 surrogate pair.' });
      }
      fieldRef.set(spliceInlines(fieldRef.inlines, op.from, op.to, op.insert, fieldRef.inHeading));
      return false;
    }

    case 'mark': {
      if (!isRecord(op.target)) return fail({ code: 'INVALID', message: 'mark op needs a target.' });
      if (!Number.isInteger(op.from) || !Number.isInteger(op.to) || op.from < 0 || op.to < op.from) {
        return fail({ code: 'INVALID', id: op.target.blockId, message: `Invalid mark range [${op.from}, ${op.to}].` });
      }
      if (op.add !== undefined) {
        if (!isRecord(op.add)) return fail({ code: 'INVALID', id: op.target.blockId, message: 'mark add must be an object.' });
        const errs = checkUpdateAttrs(op.add, RUN_MARK_FIELDS, 'inline run', opIndex, op.target.blockId, options);
        if (errs.length > 0) {
          problems.push(...errs);
          return true;
        }
      }
      const fieldRef = getInlineField(working, op.target.blockId, op.target.field);
      if (!fieldRef) return fail({ code: 'NOT_FOUND', id: op.target.blockId, message: `Mark target "${op.target.blockId}" not found.` });
      if (isInlineSurrogateSplit(fieldRef.inlines, op.from) || isInlineSurrogateSplit(fieldRef.inlines, op.to)) {
        return fail({ code: 'INVALID', id: op.target.blockId, message: 'Mark offset splits a UTF-16 surrogate pair.' });
      }
      fieldRef.set(applyMarksToInlines(fieldRef.inlines, op.from, op.to, op.add, op.remove, fieldRef.inHeading));
      return false;
    }

    case 'split': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc) return fail({ code: 'NOT_FOUND', id: op.id, message: `Block "${op.id}" not found.` });
      const blk = loc.list[loc.index];
      if (blk.type !== 'paragraph' && blk.type !== 'heading') {
        return fail({ code: 'INVALID', id: op.id, message: `Cannot split block of type "${blk.type}".` });
      }
      if (!Number.isInteger(op.at) || op.at < 0 || isInlineSurrogateSplit(blk.runs, op.at)) {
        return fail({ code: 'INVALID', id: op.id, message: 'Split offset is negative or splits a UTF-16 surrogate pair.' });
      }
      const [before, after] = splitInlinesAt(blk.runs, op.at);
      const atEnd = op.at >= inlineLength(blk.runs);
      loc.list[loc.index] = { ...blk, runs: before };
      // Enter at end of heading creates a paragraph
      const second: JinshuBlock = blk.type === 'heading' && atEnd
        ? { id: op.newId, type: 'paragraph', runs: after }
        : { ...blk, id: op.newId, runs: after };
      loc.list.splice(loc.index + 1, 0, second);
      return false;
    }

    case 'merge': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc || loc.index === 0) return fail({ code: 'INVALID', id: op.id, message: `Block "${op.id}" has no previous sibling to merge with.` });
      const prev = loc.list[loc.index - 1];
      const curr = loc.list[loc.index];
      if ((prev.type !== 'paragraph' && prev.type !== 'heading') || (curr.type !== 'paragraph' && curr.type !== 'heading')) {
        return fail({ code: 'INVALID', id: op.id, message: 'Only paragraph/heading blocks can be merged.' });
      }
      // previous block keeps id; current block id disappears
      loc.list[loc.index - 1] = {
        ...prev,
        runs: normalizeInlines([...prev.runs, ...curr.runs], { inHeading: prev.type === 'heading' }),
      };
      loc.list.splice(loc.index, 1);
      return false;
    }

    case 'setType': {
      const loc = findBlockLocation(blocks, op.id);
      if (!loc) return fail({ code: 'NOT_FOUND', id: op.id, message: `Block "${op.id}" not found.` });
      const blk = loc.list[loc.index];
      const res = convertBlock(blk, op);
      if ('lost' in res) {
        return fail({ code: 'INVALID', id: op.id, message: `Converting "${blk.type}" to "${String(op.type)}" would lose ${res.lost}.` });
      }
      loc.list[loc.index] = res.block;
      return false;
    }

    default:
      return fail({ code: 'INVALID', message: `Unknown op "${String((op as { op: unknown }).op)}".` });
  }
}
