import {
  BLOCK_FIELDS,
  CHAPTER_FIELDS,
  COLUMN_FIELDS,
  DOC_FIELDS,
  EXTENSION_BLOCK_FIELDS,
  INLINE_FIELDS,
  LIST_ITEM_FIELDS,
  SUBTITLE_FIELDS,
  TABLE_ROW_FIELDS,
  blockFields,
  fieldNames,
  isBuiltinBlockType,
  type FieldSpec,
} from './fields';
import { inlineKind, newBlockId, paragraph } from './helpers';
import { checkMarkdownProfile } from './profile';
import { emptyRegistry, validateJsonSchema } from './registry';
import type {
  BlockSpec,
  CheckOptions,
  ExtensionBlock,
  Inline,
  InlineSpec,
  JinshuBlock,
  JinshuDocument,
  JinshuRegistry,
  Problem,
} from './types';
import { JINSHU_DOC_VERSION } from './types';

const ID_RE = /^[a-z0-9_-]{1,32}$/;
const EXT_TYPE_RE = /^x-[a-z0-9]+-[a-z0-9-]+$/;
const FORBIDDEN_KEYS = new Set(['__proto__', 'constructor', 'prototype']);
const MAX_BLOCKS = 5000;
const MAX_DOC_BYTES = 2 * 1024 * 1024;
const MAX_BLOCK_CHARS = 50_000;
const MAX_LIST_DEPTH = 6;

type Rec = Record<string, unknown>;

export function isRecord(value: unknown): value is Rec {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function deepEqual(a: unknown, b: unknown): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}

function own(obj: Rec, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

// ============================================================================
// Field values
// ============================================================================

/**
 * Checks one field value against its spec (type, enum, bounds, URL). Returns an error message or `null`.
 * Structured fields (inlines, blocks, rows, …) only get their array shape checked here.
 */
export function checkFieldValue(value: unknown, spec: FieldSpec, options?: CheckOptions): string | null {
  const name = spec.name;
  const inEnum = (v: unknown) => !spec.enum || spec.enum.includes(v as string | number);
  const enumText = () => (spec.enum ?? []).map(v => JSON.stringify(v)).join(' | ');
  switch (spec.type) {
    case 'id':
      return typeof value === 'string' && ID_RE.test(value) ? null : `${name} "${String(value)}" must match [a-z0-9_-]{1,32}`;
    case 'string': {
      if (typeof value !== 'string') return `${name} must be a string`;
      if (spec.nonEmpty && value.trim().length === 0) return `${name} must be a non-empty string`;
      if (!inEnum(value)) return `${name} "${value}" must be one of ${enumText()}`;
      if (spec.url === 'src') {
        const err = validateSrc(value, options);
        return err ? `${name}: ${err}` : null;
      }
      if (spec.url === 'link') return validateLink(value, options);
      return null;
    }
    case 'integer':
    case 'number': {
      if (typeof value !== 'number' || !Number.isFinite(value)) return `${name} must be a ${spec.type}`;
      if (spec.type === 'integer' && !Number.isInteger(value)) return `${name} must be an integer`;
      if (!inEnum(value)) return `${name} ${value} must be one of ${enumText()}`;
      if (spec.min !== undefined && value < spec.min) return `${name} must be >= ${spec.min}`;
      if (spec.positive && value <= 0) return `${name} must be > 0`;
      return null;
    }
    case 'boolean':
      return typeof value === 'boolean' ? null : `${name} must be a boolean`;
    case 'true':
      return value === true ? null : `${name} must be true or omitted`;
    case 'object':
      return isRecord(value) ? null : `${name} must be an object`;
    case 'numberList':
    case 'integerList':
    case 'enumList': {
      if (!Array.isArray(value)) return `${name} must be an array`;
      for (const v of value) {
        if (spec.type === 'enumList') {
          if (!inEnum(v)) return `${name} entries must be one of ${enumText()}`;
          continue;
        }
        if (typeof v !== 'number' || !Number.isFinite(v)) return `${name} entries must be numbers`;
        if (spec.type === 'integerList' && !Number.isInteger(v)) return `${name} entries must be integers`;
        if (spec.min !== undefined && v < spec.min) return `${name} entries must be >= ${spec.min}`;
        if (spec.positive && v <= 0) return `${name} entries must be > 0`;
      }
      return null;
    }
    case 'inlines':
      return Array.isArray(value) ? null : `${name} must be an Inline[] array`;
    default:
      return Array.isArray(value) ? null : `${name} must be an array`;
  }
}

// ============================================================================
// Canonical form
// ============================================================================

type Recurse = (spec: FieldSpec, value: unknown) => unknown;

/** Copies known fields in table order (canonicalised, defaults omitted), then any unknown keys untouched. */
function normalizeRecord(raw: Rec, fields: readonly FieldSpec[], recurse: Recurse): Rec {
  const out: Rec = {};
  for (const f of fields) {
    if (!own(raw, f.name)) continue;
    let v = raw[f.name];
    if (v === undefined) continue;
    v = recurse(f, v);
    if (!f.required) {
      if (f.default !== undefined && deepEqual(v, f.default)) continue;
      if (f.type === 'true' && v === false) continue;
      if (Array.isArray(v) && v.length === 0) continue;
      if (f.type === 'string' && v === '' && !f.keepEmpty) continue;
      if (f.type === 'object' && isRecord(v) && Object.keys(v).length === 0) continue;
    }
    out[f.name] = v;
  }
  const known = fieldNames(fields);
  for (const key of Object.keys(raw)) {
    if (!known.has(key) && !FORBIDDEN_KEYS.has(key) && raw[key] !== undefined) out[key] = raw[key];
  }
  return out;
}

const copyArray: Recurse = (_spec, v) => (Array.isArray(v) ? [...v] : v);

function normalizeText(v: unknown): unknown {
  return typeof v === 'string' ? v.replace(/\r\n?/g, '\n') : v;
}

function runMarksKey(run: Rec): string {
  const rest: Rec = {};
  for (const key of Object.keys(run).sort()) if (key !== 'text') rest[key] = run[key];
  return JSON.stringify(rest);
}

/** Canonicalizes an `Inline[]` list. Unknown shapes are kept for validation to report. */
export function normalizeInlines(inlines: readonly Inline[], options?: { inHeading?: boolean }): Inline[] {
  if (!Array.isArray(inlines)) return inlines as Inline[];
  const inHeading = options?.inHeading === true;
  const out: Inline[] = [];

  for (const item of inlines) {
    const kind = inlineKind(item);
    if (!kind) {
      out.push(item);
      continue;
    }
    const raw = item as unknown as Rec;
    if (kind === 'break') {
      if (!inHeading) out.push(normalizeRecord(raw, INLINE_FIELDS.break, copyArray) as unknown as Inline);
      continue;
    }
    if (kind === 'math') {
      out.push(normalizeRecord({ ...raw, math: normalizeText(raw.math) }, INLINE_FIELDS.math, copyArray) as unknown as Inline);
      continue;
    }
    if (kind === 'atom') {
      const src: Rec = { ...raw };
      if (src.version === undefined) src.version = 1;
      out.push(normalizeRecord(src, INLINE_FIELDS.atom, copyArray) as unknown as Inline);
      continue;
    }
    // text run
    const text = normalizeText(raw.text);
    if (text === '') continue;
    const src: Rec = { ...raw, text };
    // Mark rules
    if (inHeading) {
      delete src.bold;
      delete src.script;
    }
    if (src.code === true) {
      delete src.highlight;
      delete src.script;
    }
    const run = normalizeRecord(src, INLINE_FIELDS.text, copyArray);
    const prev = out[out.length - 1] as unknown as Rec | undefined;
    if (
      prev &&
      inlineKind(prev) === 'text' &&
      typeof prev.text === 'string' &&
      typeof run.text === 'string' &&
      runMarksKey(prev) === runMarksKey(run)
    ) {
      out[out.length - 1] = { ...prev, text: prev.text + run.text } as unknown as Inline;
    } else {
      out.push(run as unknown as Inline);
    }
  }
  return out;
}

function safeFallback(spec: BlockSpec<any> | InlineSpec<any>, attrs: unknown, current: unknown): string | undefined {
  try {
    const text = spec.fallbackText(attrs as Rec);
    if (typeof text === 'string') return text;
  } catch {
    // An app's fallbackText throwing on malformed attrs must not break normalize; validation reports the attrs.
  }
  return typeof current === 'string' ? current : undefined;
}

function normalizeBlock(block: JinshuBlock, registry: JinshuRegistry): JinshuBlock {
  if (!isRecord(block)) return block;
  let raw = block as unknown as Rec;
  const blockType = raw.type;
  let fields: readonly FieldSpec[];
  if (isBuiltinBlockType(blockType)) {
    fields = BLOCK_FIELDS[blockType];
    if (blockType === 'code') raw = { ...raw, code: normalizeText(raw.code) };
    if (blockType === 'math') raw = { ...raw, latex: normalizeText(raw.latex) };
  } else if (typeof blockType === 'string' && blockType.startsWith('x-')) {
    fields = EXTENSION_BLOCK_FIELDS;
    raw = { ...raw };
    if (raw.version === undefined) raw.version = 1;
    const spec = registry.blocks.get(blockType);
    const fallback = spec ? safeFallback(spec, raw.attrs, raw.fallback) : raw.fallback;
    raw.fallback = typeof fallback === 'string' ? fallback : `[${blockType}]`;
  } else {
    return { ...raw } as unknown as JinshuBlock;
  }
  const inHeading = blockType === 'heading';
  return normalizeRecord(raw, fields, (spec, v) => normalizeField(spec, v, registry, inHeading)) as unknown as JinshuBlock;
}

function normalizeField(spec: FieldSpec, v: unknown, registry: JinshuRegistry, inHeading = false): unknown {
  if (!Array.isArray(v)) return v;
  const sub = (fields: readonly FieldSpec[]) => (item: unknown) =>
    isRecord(item) ? normalizeRecord(item, fields, (s, x) => normalizeField(s, x, registry)) : item;
  switch (spec.type) {
    case 'inlines':
      return normalizeInlines(v as Inline[], { inHeading: inHeading && spec.name === 'runs' });
    case 'blocks':
      return v.map(b => normalizeBlock(b as JinshuBlock, registry));
    case 'items':
      return v.map(sub(LIST_ITEM_FIELDS));
    case 'columns':
      return v.map(sub(COLUMN_FIELDS));
    case 'rows':
      return v.map(sub(TABLE_ROW_FIELDS));
    case 'cells':
      return v.map(cell => normalizeInlines(cell as Inline[]));
    case 'chapters':
      return v.map(sub(CHAPTER_FIELDS));
    case 'subtitles':
      return v.map(sub(SUBTITLE_FIELDS));
    default:
      return [...v];
  }
}

// ============================================================================
// Generic traversal (driven by the field table)
// ============================================================================

interface Visitor {
  block?(block: Rec): void;
  /** Ids of list items, columns and table rows. */
  nestedId?(id: unknown): void;
  inlines?(list: unknown[]): void;
}

function nestedFieldsFor(spec: FieldSpec): readonly FieldSpec[] | undefined {
  switch (spec.type) {
    case 'items': return LIST_ITEM_FIELDS;
    case 'columns': return COLUMN_FIELDS;
    case 'rows': return TABLE_ROW_FIELDS;
    default: return undefined;
  }
}

function visitRecord(raw: Rec, fields: readonly FieldSpec[], visitor: Visitor): void {
  for (const f of fields) {
    const v = raw[f.name];
    if (!Array.isArray(v)) continue;
    if (f.type === 'inlines') visitor.inlines?.(v);
    else if (f.type === 'cells') for (const cell of v) { if (Array.isArray(cell)) visitor.inlines?.(cell); }
    else if (f.type === 'blocks') visitBlocks(v, visitor);
    else {
      const nested = nestedFieldsFor(f);
      if (!nested) continue;
      for (const item of v) {
        if (!isRecord(item)) continue;
        visitor.nestedId?.(item.id);
        visitRecord(item, nested, visitor);
      }
    }
  }
}

function visitBlocks(blocks: readonly unknown[], visitor: Visitor): void {
  for (const b of blocks) {
    if (!isRecord(b)) continue;
    visitor.block?.(b);
    const fields = blockFields(b.type);
    if (fields) visitRecord(b, fields, visitor);
  }
}

/** Gives every inline atom an id unique across the document (missing, malformed and repeated ids are replaced). */
function assignAtomIds(blocks: readonly JinshuBlock[]): void {
  const taken = new Set<string>();
  visitBlocks(blocks, {
    block: b => { if (typeof b.id === 'string') taken.add(b.id); },
    nestedId: id => { if (typeof id === 'string') taken.add(id); },
  });
  visitBlocks(blocks, {
    inlines: list => {
      for (const item of list) {
        if (inlineKind(item) !== 'atom') continue;
        const atom = item as Rec;
        if (typeof atom.id !== 'string' || !ID_RE.test(atom.id) || taken.has(atom.id)) {
          let next = newBlockId('at');
          while (taken.has(next)) next = newBlockId('at');
          atom.id = next;
        }
        taken.add(atom.id as string);
      }
    },
  });
}

/**
 * Produces the canonical form of a `JinshuDocument`. Never throws; keeps unknown keys at every
 * level and never rewrites `version` (a missing one becomes the current version).
 */
export function normalizeDocument(doc: JinshuDocument, registry: JinshuRegistry = emptyRegistry): JinshuDocument {
  if (!isRecord(doc)) return doc;
  const raw = doc as unknown as Rec;
  const blocks = Array.isArray(raw.blocks) && raw.blocks.length > 0
    ? (raw.blocks as JinshuBlock[]).map(b => normalizeBlock(b, registry))
    : [paragraph()];
  assignAtomIds(blocks);
  const out = normalizeRecord({ ...raw, version: raw.version ?? JINSHU_DOC_VERSION, blocks }, DOC_FIELDS, (_s, v) => v);
  if (!own(out, 'version')) out.version = JINSHU_DOC_VERSION;
  return out as unknown as JinshuDocument;
}

function migrateBlock(block: JinshuBlock, registry: JinshuRegistry): JinshuBlock {
  if (!isRecord(block)) return block;
  const raw = block as unknown as Rec;
  const fields = blockFields(raw.type);
  if (!fields) return block;
  const next: Rec = { ...raw };
  for (const f of fields) {
    const v = raw[f.name];
    if (!Array.isArray(v)) continue;
    if (f.type === 'blocks') next[f.name] = v.map(b => migrateBlock(b as JinshuBlock, registry));
    else if (f.type === 'items' || f.type === 'columns') {
      next[f.name] = v.map(item => (isRecord(item) && Array.isArray(item.blocks)
        ? { ...item, blocks: (item.blocks as JinshuBlock[]).map(b => migrateBlock(b, registry)) }
        : item));
    }
  }
  if (typeof raw.type === 'string' && raw.type.startsWith('x-')) {
    const spec = registry.blocks.get(raw.type);
    const from = typeof raw.version === 'number' ? raw.version : 1;
    if (spec && from < spec.version && spec.migrate) {
      try {
        next.attrs = spec.migrate(raw.attrs, from);
        next.version = spec.version;
      } catch {
        // Leave the block at its old version; validation reports the attrs.
      }
    }
  }
  return next as unknown as JinshuBlock;
}

/** Runs registered extension `migrate` functions up to their spec versions, then normalizes. Never throws. */
export function migrateDocument(doc: JinshuDocument, registry: JinshuRegistry = emptyRegistry): JinshuDocument {
  if (!isRecord(doc) || !Array.isArray(doc.blocks)) return normalizeDocument(doc, registry);
  return normalizeDocument({ ...doc, blocks: doc.blocks.map(b => migrateBlock(b, registry)) }, registry);
}

// ============================================================================
// URLs
// ============================================================================

/** Validates a URL scheme for `src`. */
export function validateSrc(src: string, options?: CheckOptions): string | null {
  if (typeof src !== 'string' || src.trim().length === 0) {
    return 'src must be a non-empty string';
  }
  const trimmed = src.trim();
  const schemeMatch = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(trimmed);
  if (schemeMatch) {
    const scheme = schemeMatch[1].toLowerCase();
    if (scheme === 'javascript' || scheme === 'data' || scheme === 'file' || scheme === 'vbscript') {
      return `forbidden src scheme "${scheme}:"`;
    }
    if (options?.allowedSrcSchemes && !options.allowedSrcSchemes.includes(scheme)) {
      return `src scheme "${scheme}:" is not in allowedSrcSchemes`;
    }
    return null;
  }
  // Relative path traversal check: normalize segments and reject escaping root
  const segments = trimmed.split('/');
  let depth = 0;
  for (const seg of segments) {
    if (seg === '' || seg === '.') continue;
    if (seg === '..') {
      depth -= 1;
      if (depth < 0) return `relative src "${src}" escapes root directory via ".."`;
    } else {
      depth += 1;
    }
  }
  return null;
}

/** Validates a URL scheme for inline `link`. */
export function validateLink(href: string, options?: CheckOptions): string | null {
  if (typeof href !== 'string' || href.trim().length === 0) {
    return 'link must be a non-empty string';
  }
  const trimmed = href.trim();
  if (trimmed.startsWith('#')) {
    const targetId = trimmed.slice(1);
    if (!ID_RE.test(targetId)) return `in-page link "${href}" has invalid target id`;
    return null;
  }
  const schemeMatch = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(trimmed);
  if (!schemeMatch) {
    return `link "${href}" must use https:, http:, mailto:, tel:, or #<id>`;
  }
  const scheme = schemeMatch[1].toLowerCase();
  if (scheme === 'https' || scheme === 'http' || scheme === 'mailto' || scheme === 'tel') {
    return null;
  }
  if (scheme === 'lx') {
    if (!options?.allowedLxApps || options.allowedLxApps.length === 0) {
      return 'lx: cross-app link requires explicit allowedLxApps whitelist';
    }
    const appMatch = /^lx:\/\/([^/?#]+)/i.exec(trimmed);
    if (!appMatch || !options.allowedLxApps.includes(appMatch[1])) {
      return `lx: target "${appMatch?.[1] ?? trimmed}" is not in allowedLxApps`;
    }
    return null;
  }
  return `forbidden link scheme "${scheme}:"`;
}

// ============================================================================
// Versions
// ============================================================================

/**
 * Reasons this document cannot be edited by this code: a newer document `version`, or an extension block
 * or inline atom newer than its registered spec. Rendering is unaffected.
 */
export function readOnlyReasons(doc: JinshuDocument, registry: JinshuRegistry = emptyRegistry): Problem[] {
  const out: Problem[] = [];
  if (!isRecord(doc)) return out;
  if (typeof doc.version === 'number' && doc.version > JINSHU_DOC_VERSION) {
    out.push({
      code: 'VERSION',
      severity: 'error',
      message: `Document version ${doc.version} is newer than supported ${JINSHU_DOC_VERSION}; it is read-only here.`,
    });
  }
  const versionOf = (r: Rec) => (typeof r.version === 'number' ? r.version : 1);
  if (Array.isArray(doc.blocks)) {
    visitBlocks(doc.blocks, {
      block: b => {
        const spec = typeof b.type === 'string' ? registry.blocks.get(b.type) : undefined;
        if (spec && versionOf(b) > spec.version) {
          out.push({
            code: 'VERSION',
            severity: 'error',
            id: typeof b.id === 'string' ? b.id : undefined,
            message: `${String(b.type)} version ${versionOf(b)} is newer than the registered ${spec.version}; the document is read-only here.`,
          });
        }
      },
      inlines: list => {
        for (const item of list) {
          if (inlineKind(item) !== 'atom') continue;
          const atom = item as Rec;
          const spec = typeof atom.inline === 'string' ? registry.inlines.get(atom.inline) : undefined;
          if (spec && versionOf(atom) > spec.version) {
            out.push({
              code: 'VERSION',
              severity: 'error',
              id: typeof atom.id === 'string' ? atom.id : undefined,
              message: `${String(atom.inline)} version ${versionOf(atom)} is newer than the registered ${spec.version}; the document is read-only here.`,
            });
          }
        }
      },
    });
  }
  return out;
}

// ============================================================================
// Validation
// ============================================================================

interface BlockContext {
  readonly inQuote?: boolean;
  readonly inCallout?: boolean;
  readonly inDetails?: boolean;
  readonly inColumns?: boolean;
  readonly inListItem?: boolean;
  readonly inTranscript?: boolean;
  readonly listDepth: number;
}

/** Validates a `JinshuDocument` without throwing, returning errors & warnings. */
export function checkDocument(
  doc: JinshuDocument,
  registry: JinshuRegistry = emptyRegistry,
  options?: CheckOptions,
): Problem[] {
  try {
    return checkDocumentUnsafe(doc, registry, options);
  } catch (err) {
    return [{ code: 'INVALID', severity: 'error', message: `Document could not be checked: ${String(err)}` }];
  }
}

function checkDocumentUnsafe(doc: JinshuDocument, registry: JinshuRegistry, options?: CheckOptions): Problem[] {
  const problems: Problem[] = [];
  const seenIds = new Set<string>();
  const inPageLinks: { target: string; owner?: string }[] = [];
  let totalBlocks = 0;
  let lastHeadingLevel = 0;

  const error = (message: string, id?: string, code: Problem['code'] = 'INVALID') =>
    problems.push({ code, severity: 'error', ...(id !== undefined ? { id } : {}), message });
  const warn = (message: string, id?: string, code: Problem['code'] = 'INVALID') =>
    problems.push({ code, severity: 'warning', ...(id !== undefined ? { id } : {}), message });

  if (!isRecord(doc)) {
    return [{ code: 'INVALID', severity: 'error', message: 'Document must be an object.' }];
  }
  if (typeof doc.version === 'number' && doc.version > JINSHU_DOC_VERSION) {
    warn(`Document version ${doc.version} is newer than supported ${JINSHU_DOC_VERSION}; it renders read-only and unknown fields are preserved.`, undefined, 'VERSION');
  }
  if (!Array.isArray(doc.blocks) || doc.blocks.length === 0) {
    error('Document must contain at least 1 block (empty document = one empty paragraph).');
    return problems;
  }
  checkFields(doc as unknown as Rec, DOC_FIELDS, 'document', undefined);

  function registerId(id: unknown, label: string): void {
    if (typeof id !== 'string' || !ID_RE.test(id)) {
      error(`${label} id "${String(id)}" must match [a-z0-9_-]{1,32}.`, typeof id === 'string' ? id : undefined);
      return;
    }
    if (seenIds.has(id)) {
      error(`Duplicate id "${id}" found in document; every block, list item, column, table row and inline atom id must be unique.`, id);
    } else {
      seenIds.add(id);
    }
  }

  /** Type/enum/URL/required checks from the field table, plus warnings for unknown keys. */
  function checkFields(raw: Rec, fields: readonly FieldSpec[], label: string, ownerId: string | undefined): void {
    const known = fieldNames(fields);
    for (const f of fields) {
      if (f.name === 'id' || f.name === 'type') continue;
      const v = raw[f.name];
      if (v === undefined) {
        if (f.required) error(`${label}: required field "${f.name}" is missing.`, ownerId);
        continue;
      }
      const err = checkFieldValue(v, f, options);
      if (err) error(`${label}: ${err}.`, ownerId);
      else if (f.url === 'link' && typeof v === 'string' && v.trim().startsWith('#')) {
        inPageLinks.push({ target: v.trim().slice(1), owner: ownerId });
      }
    }
    for (const key of Object.keys(raw)) {
      if (!known.has(key) && !FORBIDDEN_KEYS.has(key)) {
        warn(`Unknown optional field "${key}" on ${label} (preserved for forward compatibility).`, ownerId);
      }
    }
  }

  function checkInlines(inlines: unknown, blockId: string | undefined, inHeading = false): void {
    if (!Array.isArray(inlines)) {
      error('Inline list must be an array.', blockId);
      return;
    }
    let charCount = 0;
    for (const item of inlines) {
      const kind = inlineKind(item);
      if (!kind) {
        const keys = isRecord(item) ? ['text', 'break', 'math', 'inline'].filter(k => item[k] !== undefined) : [];
        error(
          keys.length > 1
            ? `Inline item has conflicting keys ${keys.map(k => `"${k}"`).join(' and ')}; exactly one of text / break / math / inline is allowed.`
            : 'Unknown inline item shape; expected one of text / break / math / inline.',
          blockId,
        );
        continue;
      }
      const raw = item as Rec;
      checkFields(raw, INLINE_FIELDS[kind], `inline ${kind}`, blockId);
      if (kind === 'break') {
        if (inHeading) error('HardBreak is not allowed inside heading.', blockId);
        charCount += 1;
      } else if (kind === 'math') {
        charCount += typeof raw.math === 'string' ? raw.math.length : 0;
      } else if (kind === 'atom') {
        if (typeof raw.inline === 'string' && !EXT_TYPE_RE.test(raw.inline)) {
          error(`Inline atom type "${raw.inline}" must match x-<scope>-<name>.`, blockId);
        }
        registerId(raw.id, `Inline atom (${String(raw.inline)})`);
        const spec = typeof raw.inline === 'string' ? registry.inlines.get(raw.inline) : undefined;
        if (spec && isRecord(raw.attrs)) {
          for (const err of validateJsonSchema(raw.attrs, spec.attrsSchema)) error(`Inline ${raw.inline}: ${err}`, blockId);
        }
        if (spec && typeof raw.version === 'number' && raw.version > spec.version) {
          warn(`Inline ${raw.inline} version ${raw.version} is newer than the registered ${spec.version}; read-only here.`, blockId, 'VERSION');
        }
        charCount += 1;
      } else {
        if (typeof raw.text === 'string') {
          if (raw.text.includes('\n')) error('InlineRun.text must not contain "\\n"; use HardBreak ({ break: true }) instead.', blockId);
          charCount += raw.text.length;
        }
      }
    }
    if (charCount > MAX_BLOCK_CHARS) {
      error(`Block text exceeds ${MAX_BLOCK_CHARS} UTF-16 code units.`, blockId, 'LIMIT');
    }
  }

  function checkChildBlocks(blocks: unknown, context: BlockContext, ownerId: string, label: string, minOne: boolean): void {
    if (!Array.isArray(blocks)) return; // shape error already reported by checkFields
    if (minOne && blocks.length === 0) {
      error(`${label} must contain at least 1 child block.`, ownerId);
      return;
    }
    for (const child of blocks) checkBlock(child, context);
  }

  function checkBlock(blockValue: unknown, context: BlockContext): void {
    totalBlocks += 1;
    if (totalBlocks > MAX_BLOCKS) {
      if (totalBlocks === MAX_BLOCKS + 1) {
        error(`Document exceeds maximum limit of ${MAX_BLOCKS} blocks.`, undefined, 'LIMIT');
      }
      return;
    }
    if (!isRecord(blockValue)) {
      error('Block must be an object.');
      return;
    }
    const block = blockValue;
    const type = block.type;
    const id = typeof block.id === 'string' ? block.id : undefined;
    registerId(block.id, `Block (${String(type)})`);

    const fields = blockFields(type);
    if (!fields || (typeof type === 'string' && type.startsWith('x-') && !EXT_TYPE_RE.test(type))) {
      error(`Unknown block type "${String(type)}". Custom blocks must use x-<scope>-<name>.`, id);
      return;
    }
    const label = `"${String(type)}" block`;
    checkFields(block, fields, label, id);

    // Context restrictions
    if (context.inTranscript && type !== 'paragraph') {
      error(`Audio transcript only allows paragraph blocks, got "${String(type)}".`, id);
    }
    if (context.inQuote && (type === 'heading' || type === 'columns')) {
      error(`"${type}" is not allowed inside quote.`, id);
    }
    if ((context.inCallout || context.inDetails) && (type === 'columns' || type === 'details')) {
      error(`"${type}" is not allowed inside callout/details.`, id);
    }
    if (context.inColumns && type === 'columns') {
      error('Nested columns are not allowed.', id);
    }
    if (context.inListItem && !['paragraph', 'list', 'code', 'image'].includes(type as string)) {
      error(`List item child blocks only allow paragraph, list, code, or image; got "${String(type)}".`, id);
    }

    switch (type) {
      case 'paragraph':
        checkInlines(block.runs, id);
        break;
      case 'heading': {
        const level = block.level as number;
        if (typeof level === 'number' && [1, 2, 3, 4].includes(level)) {
          if (lastHeadingLevel > 0 && level > lastHeadingLevel + 1) {
            warn(`Heading skips level from h${lastHeadingLevel} to h${level}.`, id);
          }
          lastHeadingLevel = level;
        }
        checkInlines(block.runs, id, true);
        break;
      }
      case 'quote':
        checkChildBlocks(block.blocks, { ...context, inQuote: true }, id ?? '', 'Quote', true);
        break;
      case 'callout':
        if (Array.isArray(block.title)) checkInlines(block.title, id);
        checkChildBlocks(block.blocks, { ...context, inCallout: true }, id ?? '', 'Callout', true);
        break;
      case 'details':
        if (Array.isArray(block.summary)) checkInlines(block.summary, id);
        checkChildBlocks(block.blocks, { ...context, inDetails: true }, id ?? '', 'Details', true);
        break;
      case 'list': {
        const nextDepth = context.listDepth + 1;
        if (nextDepth > MAX_LIST_DEPTH) {
          error(`List nesting depth exceeds maximum of ${MAX_LIST_DEPTH}.`, id, 'LIMIT');
        }
        if (!Array.isArray(block.items)) break;
        if (block.items.length === 0) {
          error('List must contain at least 1 item.', id);
          break;
        }
        for (const item of block.items) {
          if (!isRecord(item)) {
            error('List item must be an object.', id);
            continue;
          }
          registerId(item.id, 'List item');
          const itemId = typeof item.id === 'string' ? item.id : id;
          checkFields(item, LIST_ITEM_FIELDS, 'list item', itemId);
          if (Array.isArray(item.runs)) checkInlines(item.runs, itemId);
          if (Array.isArray(item.blocks)) {
            for (const child of item.blocks) checkBlock(child, { ...context, inListItem: true, listDepth: nextDepth });
          }
        }
        break;
      }
      case 'code':
        if (typeof block.code === 'string' && block.code.length > MAX_BLOCK_CHARS) {
          error(`Code block exceeds ${MAX_BLOCK_CHARS} characters.`, id, 'LIMIT');
        }
        if (typeof block.code === 'string' && Array.isArray(block.highlightLines)) {
          const lines = block.code.split('\n').length;
          if (block.highlightLines.some(n => typeof n === 'number' && n > lines)) {
            warn(`highlightLines points past the last line (${lines}).`, id);
          }
        }
        break;
      case 'math':
      case 'divider':
        break;
      case 'table': {
        if (!Array.isArray(block.rows)) break;
        if (block.rows.length === 0) {
          error('Table must have at least 1 row.', id);
          break;
        }
        let cols = -1;
        block.rows.forEach((row, r) => {
          if (!isRecord(row)) {
            error(`Table row ${r} must be an object { id, cells }.`, id);
            return;
          }
          registerId(row.id, 'Table row');
          checkFields(row, TABLE_ROW_FIELDS, 'table row', id);
          if (!Array.isArray(row.cells)) return;
          if (cols < 0) {
            cols = row.cells.length;
            if (cols === 0) error('Table must have at least 1 column.', id);
          } else if (row.cells.length !== cols) {
            error(`Table row "${String(row.id)}" has ${row.cells.length} cells, expected ${cols}.`, id);
          }
          for (const cell of row.cells) checkInlines(cell, id);
        });
        if (cols > 0) {
          if (Array.isArray(block.widths) && block.widths.length !== cols) error('Table widths length must match column count.', id);
          if (Array.isArray(block.align) && block.align.length !== cols) error('Table align length must match column count.', id);
        }
        break;
      }
      case 'columns': {
        if (!Array.isArray(block.columns)) break;
        if (block.columns.length < 2 || block.columns.length > 3) {
          error('Columns block must have 2 or 3 columns.', id);
          break;
        }
        if (Array.isArray(block.widths) && block.widths.length !== block.columns.length) {
          error('Columns widths length must match column count.', id);
        }
        for (const col of block.columns) {
          if (!isRecord(col)) {
            error('Column must be an object.', id);
            continue;
          }
          registerId(col.id, 'Column');
          const colId = typeof col.id === 'string' ? col.id : id;
          checkFields(col, COLUMN_FIELDS, 'column', colId);
          checkChildBlocks(col.blocks, { ...context, inColumns: true }, colId ?? '', 'Each column', true);
        }
        break;
      }
      case 'image':
        if (block.alt === undefined) {
          warn('Image is missing alt text for accessibility (use alt="" for decorative images).', id);
        }
        if (Array.isArray(block.caption)) checkInlines(block.caption, id);
        break;
      case 'video':
        if (Array.isArray(block.caption)) checkInlines(block.caption, id);
        checkNested(block.chapters, CHAPTER_FIELDS, 'video chapter', id);
        checkNested(block.subtitles, SUBTITLE_FIELDS, 'video subtitle', id);
        break;
      case 'audio':
        checkNested(block.chapters, CHAPTER_FIELDS, 'audio chapter', id);
        if (Array.isArray(block.transcript)) {
          for (const child of block.transcript) checkBlock(child, { ...context, inTranscript: true });
        }
        break;
      case 'file':
        break;
      default:
        checkExtension(block as unknown as ExtensionBlock, id, context);
        break;
    }
  }

  function checkNested(list: unknown, fields: readonly FieldSpec[], label: string, ownerId: string | undefined): void {
    if (!Array.isArray(list)) return;
    for (const entry of list) {
      if (!isRecord(entry)) error(`${label} must be an object.`, ownerId);
      else checkFields(entry, fields, label, ownerId);
    }
  }

  function checkExtension(ext: ExtensionBlock, id: string | undefined, context: BlockContext): void {
    const raw = ext as unknown as Rec;
    const spec = registry.blocks.get(ext.type);
    const hasRuns = Array.isArray(raw.runs) && raw.runs.length > 0;
    const hasBlocks = Array.isArray(raw.blocks) && raw.blocks.length > 0;
    if (spec) {
      if (isRecord(raw.attrs)) {
        for (const err of validateJsonSchema(raw.attrs, spec.attrsSchema)) error(`${ext.type}: ${err}`, id);
      }
      const version = typeof raw.version === 'number' ? raw.version : 1;
      if (version > spec.version) {
        warn(`${ext.type} version ${version} is newer than the registered ${spec.version}; read-only here.`, id, 'VERSION');
      }
      if (spec.content === 'none' && (hasRuns || hasBlocks)) {
        error(`${ext.type} has content "none" and must not carry runs or blocks.`, id);
      } else if (spec.content === 'inline' && hasBlocks) {
        error(`${ext.type} has inline content and must not carry child blocks.`, id);
      } else if (typeof spec.content === 'object' && hasRuns) {
        error(`${ext.type} holds child blocks and must not carry runs.`, id);
      }
    }
    if (hasRuns && hasBlocks) error(`${ext.type} must not carry both runs and blocks.`, id);
    if (Array.isArray(raw.runs)) checkInlines(raw.runs, id);
    if (Array.isArray(raw.blocks)) for (const child of raw.blocks) checkBlock(child, context);
  }

  for (const block of doc.blocks) {
    checkBlock(block, { listDepth: 0 });
  }

  for (const link of inPageLinks) {
    if (!seenIds.has(link.target)) {
      warn(`In-page link "#${link.target}" points to no block in this document.`, link.owner, 'NOT_FOUND');
    }
  }

  if (options?.format === 'markdown') problems.push(...checkMarkdownProfile(doc));

  // Approximate canonical JSON byte size limit
  const serializedLen = JSON.stringify(doc).length;
  if (serializedLen > MAX_DOC_BYTES) {
    error(`Document size (${serializedLen} chars) exceeds 2 MB limit.`, undefined, 'LIMIT');
  }

  return problems;
}
