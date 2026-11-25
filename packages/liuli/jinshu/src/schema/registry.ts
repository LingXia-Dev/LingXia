import { BLOCK_FIELDS, LIST_ITEM_FIELDS, attrFields, updatableFields, type FieldSpec } from './fields';
import type {
  BuiltinBlockType,
  BlockSpec,
  FilePolicy,
  InlineSpec,
  JinshuRegistry,
  JsonSchema,
} from './types';

export function defineBlock<TAttrs extends Record<string, unknown>>(spec: BlockSpec<TAttrs>): BlockSpec<TAttrs> {
  return Object.freeze({ ...spec });
}

export function defineInline<TAttrs extends Record<string, unknown>>(spec: InlineSpec<TAttrs>): InlineSpec<TAttrs> {
  return Object.freeze({ ...spec });
}

export function createRegistry(specs?: {
  readonly blocks?: readonly BlockSpec<any>[];
  readonly inlines?: readonly InlineSpec<any>[];
}): JinshuRegistry {
  const blocks = new Map<string, BlockSpec<any>>();
  const inlines = new Map<string, InlineSpec<any>>();
  if (specs?.blocks) {
    for (const b of specs.blocks) blocks.set(b.type, b);
  }
  if (specs?.inlines) {
    for (const i of specs.inlines) inlines.set(i.inline, i);
  }
  return { blocks, inlines };
}

export const emptyRegistry: JinshuRegistry = createRegistry();

/** Validates a runtime value against a `JsonSchema` subset, returning error messages. */
export function validateJsonSchema(value: unknown, schema: JsonSchema<unknown>, path = 'attrs'): string[] {
  const errors: string[] = [];
  if (schema.enum !== undefined) {
    if (!schema.enum.some(item => Object.is(item, value))) {
      errors.push(`${path}: must be one of ${JSON.stringify(schema.enum)}`);
      return errors;
    }
  }
  if (schema.type !== undefined) {
    switch (schema.type) {
      case 'null':
        if (value !== null) errors.push(`${path}: expected null`);
        return errors;
      case 'boolean':
        if (typeof value !== 'boolean') errors.push(`${path}: expected boolean`);
        return errors;
      case 'string':
        if (typeof value !== 'string') {
          errors.push(`${path}: expected string`);
          return errors;
        }
        if (schema.minLength !== undefined && value.length < schema.minLength) {
          errors.push(`${path}: string length must be >= ${schema.minLength}`);
        }
        if (schema.maxLength !== undefined && value.length > schema.maxLength) {
          errors.push(`${path}: string length must be <= ${schema.maxLength}`);
        }
        return errors;
      case 'number':
      case 'integer':
        if (typeof value !== 'number' || !Number.isFinite(value)) {
          errors.push(`${path}: expected ${schema.type}`);
          return errors;
        }
        if (schema.type === 'integer' && !Number.isInteger(value)) {
          errors.push(`${path}: expected integer`);
        }
        if (schema.minimum !== undefined && value < schema.minimum) {
          errors.push(`${path}: must be >= ${schema.minimum}`);
        }
        if (schema.maximum !== undefined && value > schema.maximum) {
          errors.push(`${path}: must be <= ${schema.maximum}`);
        }
        return errors;
      case 'array':
        if (!Array.isArray(value)) {
          errors.push(`${path}: expected array`);
          return errors;
        }
        if (schema.items) {
          for (let i = 0; i < value.length; i++) {
            errors.push(...validateJsonSchema(value[i], schema.items, `${path}[${i}]`));
          }
        }
        return errors;
      case 'object':
        if (typeof value !== 'object' || value === null || Array.isArray(value)) {
          errors.push(`${path}: expected object`);
          return errors;
        }
        const record = value as Record<string, unknown>;
        if (schema.required) {
          for (const key of schema.required) {
            if (!(key in record) || record[key] === undefined) {
              errors.push(`${path}.${key}: required property is missing`);
            }
          }
        }
        if (schema.properties) {
          for (const [key, propSchema] of Object.entries(schema.properties)) {
            if (key in record && record[key] !== undefined) {
              errors.push(...validateJsonSchema(record[key], propSchema, `${path}.${key}`));
            }
          }
        }
        if (schema.additionalProperties === false && schema.properties) {
          for (const key of Object.keys(record)) {
            if (!(key in schema.properties)) {
              errors.push(`${path}.${key}: unexpected property`);
            }
          }
        }
        return errors;
    }
  }
  return errors;
}

export interface ExportedAiSchema {
  readonly markupGuide: string;
  readonly customBlocks: readonly {
    readonly type: string;
    readonly version: number;
    readonly content: BlockSpec['content'];
    readonly description: string;
    readonly attrsSchema: JsonSchema<unknown>;
    readonly example?: Readonly<Record<string, unknown>>;
  }[];
  readonly customInlines: readonly {
    readonly inline: string;
    readonly version: number;
    readonly description: string;
    readonly attrsSchema: JsonSchema<unknown>;
    readonly example?: Readonly<Record<string, unknown>>;
  }[];
  readonly operationsSchema: Readonly<Record<string, unknown>>;
}

/** Exports Jinshu Markup quick reference, registered extension specs, and operation JSON Schema for LLMs. */
export function exportAiSchema(
  registry: JinshuRegistry = emptyRegistry,
  options?: { readonly filePolicy?: FilePolicy },
): ExportedAiSchema {
  const allowInlineFile = options?.filePolicy?.inline !== 'never';
  const customBlocks = Array.from(registry.blocks.values()).map(spec => ({
    type: spec.type,
    version: spec.version,
    content: spec.content,
    description: spec.aiDescription,
    attrsSchema: spec.attrsSchema,
    ...(spec.aiExample ? { example: spec.aiExample } : {}),
  }));
  const customInlines = Array.from(registry.inlines.values()).map(spec => ({
    inline: spec.inline,
    version: spec.version,
    description: spec.aiDescription,
    attrsSchema: spec.attrsSchema,
    ...(spec.aiExample ? { example: spec.aiExample } : {}),
  }));

  const markupLines = [
    'Jinshu Markup (strict XML subset, lowercase tags, double-quoted attrs, explicit closing tags).',
    'An attribute has the same name and unit as the JSON field (durationMs="125400", width="480"); lists are comma lists (widths="2,1").',
    'Blocks (attributes in brackets are optional):',
    ...BLOCK_MARKUP_ORDER.map(type => `  ${blockMarkupLine(type, allowInlineFile)}`),
    '  <x-scope-name id [version] [fallback]><attrs>{JSON}</attrs>inline or block content</x-scope-name> for custom blocks.',
    'Inline: <b>, <i>, <u>, <s>, <code>, <mark [tone="warning|success|danger"]>, <sub>, <sup>, <a href="...">, <br/>, <math>...</math>,',
    '  <x-scope-name id [fallback]/> for an inline atom (echo its id to keep it; add <attrs>{JSON}</attrs> to change it).',
    'Canonical nesting order: a > mark > b > i > u > s > sub/sup > code. Escape only &lt; &gt; &amp; &quot;.',
    'edit matches `find` against the plain text of the field; `replace` is inline markup (plain text keeps the marks at the match).',
    'update sets non-structural fields only; change runs/items/blocks/rows/columns with replace.',
  ];

  const operationsSchema = buildOperationsSchema();

  return {
    markupGuide: markupLines.join('\n'),
    customBlocks,
    customInlines,
    operationsSchema,
  };
}

const BLOCK_MARKUP_ORDER: readonly BuiltinBlockType[] = [
  'paragraph', 'heading', 'quote', 'callout', 'details', 'list', 'code', 'math', 'table', 'columns',
  'divider', 'image', 'video', 'audio', 'file',
];

const BLOCK_TAG: Readonly<Record<BuiltinBlockType, string>> = {
  paragraph: 'p', heading: 'h1..h4', quote: 'quote', callout: 'callout', details: 'details', list: 'list',
  code: 'code', math: 'math', table: 'table', columns: 'columns', divider: 'divider', image: 'image',
  video: 'video', audio: 'audio', file: 'file',
};

const BLOCK_BODY: Partial<Record<BuiltinBlockType, string>> = {
  paragraph: 'inline',
  heading: 'inline',
  quote: 'blocks',
  callout: '<title>inline</title> blocks',
  details: '<summary>inline</summary> blocks',
  list: '<li id [checked="true"]>inline, or <p>inline</p> + child blocks</li>',
  code: 'a ``` fence line, the code, the same fence line (longer fence if the code has ```)',
  math: 'LaTeX',
  table: '<tr id><th|td>inline</th|td></tr> (headerRow="true": the first row is the header)',
  columns: '<col id>blocks</col>',
  image: '<caption>inline</caption>',
  video: '<caption>inline</caption> <chapter atMs>title</chapter> <track src lang label/>',
  audio: '<chapter atMs>title</chapter> transcript paragraphs',
};

function attrText(f: FieldSpec): string {
  const value = f.enum ? `"${f.enum.join('|')}"` : '';
  const text = `${f.name}${value ? `=${value}` : ''}`;
  return f.required && f.default === undefined ? text : `[${text}]`;
}

/** One markup quick-reference line per block type, generated from the field table. */
function blockMarkupLine(type: BuiltinBlockType, allowInlineFile: boolean): string {
  const fields = attrFields(BLOCK_FIELDS[type]).filter(f => !(type === 'file' && f.name === 'display' && !allowInlineFile));
  const attrs = fields.map(attrText).join(' ');
  const tag = BLOCK_TAG[type];
  const body = BLOCK_BODY[type];
  const note = type === 'file' && !allowInlineFile ? ' (inline display disabled by host policy)' : '';
  return body ? `<${tag} ${attrs}>${body}</${tag}>${note}` : `<${tag} ${attrs}/>${note}`;
}

function fieldJsonSchema(f: FieldSpec): Record<string, unknown> {
  const base: Record<string, unknown> = {};
  switch (f.type) {
    case 'id':
    case 'string': base.type = 'string'; break;
    case 'integer': base.type = 'integer'; break;
    case 'number': base.type = 'number'; break;
    case 'boolean': base.type = 'boolean'; break;
    case 'true': base.const = true; break;
    case 'object': base.type = 'object'; break;
    case 'numberList':
    case 'integerList': base.type = 'array'; base.items = { type: f.type === 'integerList' ? 'integer' : 'number' }; break;
    case 'enumList': base.type = 'array'; base.items = { enum: f.enum }; break;
    default: base.type = 'array'; break;
  }
  if (f.enum && f.type !== 'enumList') base.enum = f.enum;
  if (f.min !== undefined) base.minimum = f.min;
  return base;
}

const position = {
  type: 'object',
  description: 'Exactly one of { before: id } | { after: id } | { start: containerId | "doc" } | { end: containerId | "doc" }.',
  minProperties: 1,
  maxProperties: 1,
  properties: { before: { type: 'string' }, after: { type: 'string' }, start: { type: 'string' }, end: { type: 'string' } },
};
const note = { type: 'string', description: 'Human-readable reason shown in review & revision history.' };
const blockInputs = { type: 'array', minItems: 1, description: 'Structured BlockInput[] (JSON blocks).' };

/** Per-op JSON Schema (`oneOf`), with `update` attrs described from the field table. */
function buildOperationsSchema(): Record<string, unknown> {
  const opSchema = (op: string, required: readonly string[], properties: Record<string, unknown>, description: string) => ({
    type: 'object',
    description,
    required: ['op', ...required],
    additionalProperties: false,
    properties: { op: { const: op }, note, ...properties },
  });
  const content = (required: readonly string[], props: Record<string, unknown>, op: string, description: string) => ({
    oneOf: [
      opSchema(op, [...required, 'markup'], { ...props, markup: { type: 'string', description: 'Jinshu Markup fragment.' } }, description),
      opSchema(op, [...required, 'blocks'], { ...props, blocks: blockInputs }, description),
    ],
  });
  const updatable: Record<string, unknown> = {};
  for (const [type, fields] of Object.entries(BLOCK_FIELDS)) {
    const names = updatableFields(fields).map(f => f.name);
    if (names.length > 0) updatable[type] = names.join(', ');
  }
  updatable['list item'] = updatableFields(LIST_ITEM_FIELDS).map(f => f.name).join(', ');
  const attrProps: Record<string, unknown> = {};
  for (const fields of [...Object.values(BLOCK_FIELDS), LIST_ITEM_FIELDS]) {
    for (const f of updatableFields(fields)) {
      if (!(f.name in attrProps)) attrProps[f.name] = fieldJsonSchema(f);
    }
  }
  return {
    type: 'array',
    description: 'Atomic list of JinshuOp operations applied in order; any failure leaves the document unchanged.',
    items: {
      oneOf: [
        content(['at'], { at: position }, 'insert', 'Insert new blocks at a position.'),
        content(['id'], { id: { type: 'string' } }, 'replace', 'Replace a block; the first new block keeps its id.'),
        opSchema('edit', ['id', 'find', 'replace'], {
          id: { type: 'string', description: 'Block or list-item id.' },
          field: { description: 'runs | code | title | summary | caption | { cell: [rowId, col] }.' },
          lineRange: { type: 'array', items: { type: 'integer' }, minItems: 2, maxItems: 2, description: '1-based inclusive [start, end] lines (code only).' },
          find: { type: 'string', minLength: 1, description: 'Exact plain text to find in the field (not markup).' },
          replace: { type: 'string', description: 'Inline markup; plain text keeps the marks at the match.' },
          all: { type: 'boolean', description: 'Replace every occurrence instead of requiring one.' },
        }, 'Find-and-replace inside one field of one block.'),
        opSchema('update', ['id', 'attrs'], {
          id: { type: 'string' },
          attrs: {
            type: 'object',
            minProperties: 1,
            properties: attrProps,
            description: `Fields to set (null removes an optional one). Updatable per type: ${Object.entries(updatable).map(([t, n]) => `${t}: ${n}`).join('; ')}. Custom blocks: their attrs.`,
          },
        }, 'Set non-structural fields of a block, list item, or custom block attrs.'),
        opSchema('tableRow', ['id', 'action'], {
          id: { type: 'string', description: 'Table block id.' },
          action: { enum: ['insert', 'delete', 'move'] },
          rowId: { type: 'string', description: 'Row to delete/move, or the id for the inserted row.' },
          before: { type: 'string', description: 'Anchor row id.' },
          after: { type: 'string', description: 'Anchor row id.' },
          cells: { type: 'array', items: { type: 'array' }, description: 'Inline[] per cell for insert.' },
        }, 'Insert, delete or move a table row by row id.'),
        opSchema('tableCol', ['id', 'action', 'index'], {
          id: { type: 'string' },
          action: { enum: ['insert', 'delete', 'move'] },
          index: { type: 'integer', minimum: 0 },
          toIndex: { type: 'integer', minimum: 0 },
          header: { type: 'array', description: 'Header cell Inline[] for insert.' },
          align: { enum: ['start', 'center', 'end'] },
          width: { type: 'number', exclusiveMinimum: 0 },
        }, 'Insert, delete or move a table column by index.'),
        opSchema('delete', ['id'], { id: { type: 'string' } }, 'Delete a block or list item.'),
        opSchema('move', ['id', 'to'], { id: { type: 'string' }, to: position }, 'Move a block.'),
        opSchema('setMeta', [], { title: { type: 'string' }, lang: { type: 'string' }, meta: { type: 'object' } }, 'Set document title, language or metadata.'),
        opSchema('rewrite', ['markup'], { markup: { type: 'string' } }, 'Replace the whole document with markup; blocks keep ids by id or content.'),
      ],
    },
  };
}
