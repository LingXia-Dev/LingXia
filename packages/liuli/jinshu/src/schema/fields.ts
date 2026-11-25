/**
 * Field specs: the single table of every field of every block, inline and nested record.
 * Validation, canonical form, `update` op checks, markup attributes and the AI schema
 * export all read it; nothing else keeps its own list of keys.
 */
import type { BuiltinBlockType, InlineKind } from './types';

export type FieldType =
  | 'id'
  | 'string'
  | 'integer'
  | 'number'
  | 'boolean'
  /** A flag that is either `true` or absent (inline marks, `headerRow`). */
  | 'true'
  | 'object'
  | 'numberList'
  | 'integerList'
  | 'enumList'
  | 'inlines'
  | 'blocks'
  | 'items'
  | 'columns'
  | 'rows'
  | 'cells'
  | 'chapters'
  | 'subtitles';

export interface FieldSpec {
  readonly name: string;
  readonly type: FieldType;
  /** Allowed values (for `enumList`: allowed element values). */
  readonly enum?: readonly (string | number)[];
  readonly required?: boolean;
  /** Canonical form omits an optional field equal to its default. */
  readonly default?: unknown;
  /** Lower bound for numbers and list elements (inclusive). */
  readonly min?: number;
  /** Numbers (and list elements) must be > 0. */
  readonly positive?: boolean;
  readonly nonEmpty?: boolean;
  /** Optional strings normally drop `''`; this keeps it (alt="" marks a decorative image). */
  readonly keepEmpty?: boolean;
  /** URL-like string checked by `validateSrc` or `validateLink`. */
  readonly url?: 'src' | 'link';
  /** Content or identity: cannot be set through `update` (use replace / text ops). */
  readonly structural?: boolean;
  /** Where markup carries it when not an attribute: in the tag name, or as the element's text content. */
  readonly markup?: 'tag' | 'content';
  readonly description?: string;
}

const id: FieldSpec = { name: 'id', type: 'id', required: true, structural: true };
const type: FieldSpec = { name: 'type', type: 'string', required: true, structural: true, markup: 'tag' };
const runs: FieldSpec = { name: 'runs', type: 'inlines', required: true, structural: true };
const blocks: FieldSpec = { name: 'blocks', type: 'blocks', required: true, structural: true };
const src: FieldSpec = { name: 'src', type: 'string', required: true, nonEmpty: true, url: 'src' };
const caption: FieldSpec = { name: 'caption', type: 'inlines' };
const durationMs: FieldSpec = { name: 'durationMs', type: 'integer', min: 0 };
const chapters: FieldSpec = { name: 'chapters', type: 'chapters' };

export const BLOCK_FIELDS: Readonly<Record<BuiltinBlockType, readonly FieldSpec[]>> = {
  paragraph: [id, type, runs],
  heading: [id, type, { name: 'level', type: 'integer', enum: [1, 2, 3, 4], required: true, markup: 'tag' }, runs],
  quote: [id, type, blocks],
  callout: [
    id,
    type,
    { name: 'tone', type: 'string', enum: ['info', 'tip', 'warning', 'danger'], required: true, default: 'info' },
    { name: 'title', type: 'inlines' },
    blocks,
  ],
  details: [
    id,
    type,
    { name: 'summary', type: 'inlines', required: true },
    { name: 'defaultOpen', type: 'boolean', default: false },
    blocks,
  ],
  list: [
    id,
    type,
    { name: 'style', type: 'string', enum: ['bullet', 'ordered', 'task'], required: true, default: 'bullet' },
    { name: 'start', type: 'integer', min: 0, default: 1 },
    { name: 'items', type: 'items', required: true, structural: true },
  ],
  code: [
    id,
    type,
    { name: 'language', type: 'string', required: true, default: 'text' },
    { name: 'code', type: 'string', required: true, markup: 'content' },
    { name: 'filename', type: 'string' },
    { name: 'highlightLines', type: 'integerList', min: 1 },
  ],
  math: [id, type, { name: 'latex', type: 'string', required: true, markup: 'content' }],
  table: [
    id,
    type,
    { name: 'align', type: 'enumList', enum: ['start', 'center', 'end'] },
    { name: 'widths', type: 'numberList', positive: true },
    { name: 'headerRow', type: 'true' },
    { name: 'rows', type: 'rows', required: true, structural: true },
  ],
  columns: [
    id,
    type,
    { name: 'widths', type: 'numberList', positive: true },
    { name: 'columns', type: 'columns', required: true, structural: true },
  ],
  divider: [id, type],
  image: [
    id,
    type,
    src,
    { name: 'alt', type: 'string', keepEmpty: true },
    { name: 'width', type: 'integer', min: 1 },
    { name: 'height', type: 'integer', min: 1 },
    caption,
    { name: 'size', type: 'string', enum: ['small', 'medium', 'full'], default: 'full' },
  ],
  video: [
    id,
    type,
    src,
    { name: 'poster', type: 'string', url: 'src' },
    durationMs,
    caption,
    chapters,
    { name: 'subtitles', type: 'subtitles' },
  ],
  audio: [
    id,
    type,
    src,
    { name: 'title', type: 'string' },
    { name: 'cover', type: 'string', url: 'src' },
    durationMs,
    chapters,
    { name: 'transcript', type: 'blocks', structural: true },
  ],
  file: [
    id,
    type,
    { name: 'display', type: 'string', enum: ['attachment', 'inline'] },
    src,
    { name: 'name', type: 'string', required: true, nonEmpty: true },
    { name: 'mimeType', type: 'string' },
    { name: 'sizeBytes', type: 'integer', min: 0 },
    { name: 'pageCount', type: 'integer', min: 0 },
    { name: 'description', type: 'string' },
  ],
};

export const EXTENSION_BLOCK_FIELDS: readonly FieldSpec[] = [
  id,
  type,
  { name: 'version', type: 'integer', min: 1, required: true, default: 1 },
  { name: 'attrs', type: 'object', required: true },
  { name: 'fallback', type: 'string' },
  { name: 'runs', type: 'inlines', structural: true },
  { name: 'blocks', type: 'blocks', structural: true },
];

export const LIST_ITEM_FIELDS: readonly FieldSpec[] = [
  id,
  { name: 'checked', type: 'boolean', default: false },
  runs,
  { name: 'blocks', type: 'blocks', structural: true },
];

export const COLUMN_FIELDS: readonly FieldSpec[] = [id, blocks];

export const TABLE_ROW_FIELDS: readonly FieldSpec[] = [id, { name: 'cells', type: 'cells', required: true, structural: true }];

export const CHAPTER_FIELDS: readonly FieldSpec[] = [
  { name: 'atMs', type: 'integer', min: 0, required: true },
  { name: 'title', type: 'string', required: true, markup: 'content' },
];

export const SUBTITLE_FIELDS: readonly FieldSpec[] = [
  { name: 'src', type: 'string', required: true, nonEmpty: true, url: 'src' },
  { name: 'lang', type: 'string', required: true, nonEmpty: true },
  { name: 'label', type: 'string', required: true },
];

export const DOC_FIELDS: readonly FieldSpec[] = [
  { name: 'version', type: 'integer', min: 1, required: true, default: 1 },
  { name: 'id', type: 'string' },
  { name: 'title', type: 'string' },
  { name: 'lang', type: 'string' },
  { name: 'meta', type: 'object' },
  { name: 'blocks', type: 'blocks', required: true, structural: true },
];

/** Inline run marks in canonical order; every mark is optional and omitted when off. */
export const RUN_MARK_FIELDS: readonly FieldSpec[] = [
  { name: 'bold', type: 'true' },
  { name: 'italic', type: 'true' },
  { name: 'underline', type: 'true' },
  { name: 'strike', type: 'true' },
  { name: 'code', type: 'true' },
  { name: 'script', type: 'string', enum: ['sub', 'super'] },
  { name: 'highlight', type: 'string', enum: ['accent', 'warning', 'success', 'danger'] },
  { name: 'link', type: 'string', url: 'link' },
];

export const INLINE_FIELDS: Readonly<Record<InlineKind, readonly FieldSpec[]>> = {
  text: [{ name: 'text', type: 'string', required: true }, ...RUN_MARK_FIELDS],
  break: [{ name: 'break', type: 'true', required: true }],
  math: [{ name: 'math', type: 'string', required: true }],
  atom: [
    { name: 'id', type: 'id', required: true },
    { name: 'inline', type: 'string', required: true, markup: 'tag' },
    { name: 'version', type: 'integer', min: 1, required: true, default: 1 },
    { name: 'attrs', type: 'object', required: true },
    { name: 'fallback', type: 'string', required: true },
  ],
};

/** Field types written as one markup attribute (`name="value"`) unless `markup` says otherwise. */
export const ATTR_FIELD_TYPES: ReadonlySet<FieldType> = new Set<FieldType>([
  'id',
  'string',
  'integer',
  'number',
  'boolean',
  'true',
  'numberList',
  'integerList',
  'enumList',
]);

/** Fields written as markup attributes, in table order. */
export function attrFields(fields: readonly FieldSpec[]): FieldSpec[] {
  return fields.filter(f => ATTR_FIELD_TYPES.has(f.type) && f.markup === undefined);
}

export function isBuiltinBlockType(value: unknown): value is BuiltinBlockType {
  return typeof value === 'string' && Object.prototype.hasOwnProperty.call(BLOCK_FIELDS, value);
}

/** Field table for a block type: built-in table, extension table for `x-…`, or `undefined`. */
export function blockFields(blockType: unknown): readonly FieldSpec[] | undefined {
  if (isBuiltinBlockType(blockType)) return BLOCK_FIELDS[blockType];
  if (typeof blockType === 'string' && blockType.startsWith('x-')) return EXTENSION_BLOCK_FIELDS;
  return undefined;
}

export function fieldNames(fields: readonly FieldSpec[]): ReadonlySet<string> {
  return new Set(fields.map(f => f.name));
}

/** Fields `update` may set on a block of this type. */
export function updatableFields(fields: readonly FieldSpec[]): FieldSpec[] {
  return fields.filter(f => !f.structural);
}
