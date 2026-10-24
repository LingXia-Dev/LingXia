// Every artifact derived from schema.json. `--check` fails on drift instead of writing.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

export const packageRoot = fileURLToPath(new URL('..', import.meta.url));
export const repositoryRoot = join(packageRoot, '../..');

export async function readSchema() {
  return JSON.parse(await readFile(join(packageRoot, 'contracts/schema.json'), 'utf8'));
}

const header = '// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.\n';
const screaming = name => name.replace(/-/g, '_').replace(/([a-z0-9])([A-Z])/g, '$1_$2').toUpperCase();
const object = entries => JSON.stringify(Object.fromEntries(entries));

function validate(schema) {
  const ids = schema.fields.map(field => field.id);
  if (ids.some((id, index) => id !== index)) throw Error('schema.json: field ids must be dense and ordered');
  const keys = new Set();
  for (const field of schema.fields) {
    if (keys.has(field.key)) throw Error(`schema.json: duplicate field key ${field.key}`);
    keys.add(field.key);
  }
}

function rust(schema) {
  const tags = schema.tags.map((name, id) => `    pub const ${screaming(name)}: usize = ${id};`).join('\n');
  const fields = schema.fields.map(field => `    pub const ${screaming(field.key)}: usize = ${field.id};`).join('\n');
  const table = schema.fields.map(field => `    (${JSON.stringify(field.wire)}, ${JSON.stringify(field.kind)}),`).join('\n');
  const options = Object.entries(schema.options)
    .map(([kind, values]) => `        ${JSON.stringify(kind)} => Some(&[${values.map(value => JSON.stringify(value)).join(', ')}]),`).join('\n');
  return `${header}//! Wire ids are append-only; native validation stays independent of the JS runtime.

pub const UI_ABI: u32 = ${schema.uiAbi};

/// Wire tag ids.
pub mod tag {
${tags}
}

/// Wire field ids.
pub mod field {
${fields}
}

pub const TAGS: &[&str] = &[${schema.tags.map(name => JSON.stringify(name)).join(', ')}];

/// Wire name and value kind, indexed by field id.
pub const FIELDS: &[(&str, &str)] = &[
${table}
];

/// Enumerated values accepted for an option-valued field kind.
pub fn options(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
${options}
        _ => None,
    }
}
`;
}

const styleKinds = new Set(['number', 'string', 'alignment', 'justification', 'font-family', 'font-style', 'flex-wrap',
  'flex-direction', 'flex-display', 'flex-basis', 'scroll-axis', 'animation', 'word-break', 'overflow-wrap',
  'text-alignment', 'text-decoration-line', 'gradient', 'shadow', 'stack-offset']);

function wire(schema) {
  const byKind = test => schema.fields.filter(field => test(field.kind));
  const map = (fields, key = field => field.wire) => object(fields.map(field => [key(field), field.id]));
  const fieldId = key => schema.fields.find(field => field.key === key).id;
  return `${header}export const tags = ${object(schema.tags.map((name, id) => [name, id]))};
export const field = ${object(schema.fields.map(field => [field.key, field.id]))};
export const fields = ${map(byKind(kind => styleKinds.has(kind)))};
export const vectorField = ${fieldId('vectorSource')};
export const dimensionPercentFields = ${object(['width', 'height', 'minWidth', 'minHeight', 'maxWidth', 'maxHeight'].map(name => [name, fieldId(`${name}Percent`)]))};
export const styleOptions = ${object(schema.fields.filter(field => schema.options[field.kind]).map(field => [field.wire, schema.options[field.kind]]))};
export const buttonProps = ${map(byKind(kind => ['handle', 'flag', 'radio-label'].includes(kind)))};
export const listFields = ${map(byKind(kind => kind.startsWith('list-')))};
export const reachProps = ${map(byKind(kind => kind === 'reach-handle'))};
export const imageProps = ${map(byKind(kind => kind.startsWith('image-')))};
export const inputProps = ${map(byKind(kind => kind.startsWith('input-')))};
export const checkProps = ${map(byKind(kind => kind.startsWith('check-')), field => field.wire === 'checked' ? 'value' : field.wire)};
export const radioProps = ${map(byKind(kind => kind.startsWith('radio-')))};
export const sliderProps = ${map(byKind(kind => kind.startsWith('slider-') || kind === 'radio-label'))};
export const gridProps = ${map(byKind(kind => kind.startsWith('grid-')))};
export const textProps = ${map(byKind(kind => kind.startsWith('text-')))};
export const plotProps = ${object(['plotValues', 'plotKind', 'plotColors', 'accessibilityLabel', 'plotSmooth', 'plotArea', 'onPlotSelect',
    'plotSelectedIndex', 'plotDomainMin', 'plotDomainMax', 'plotPieLayout', 'plotMarkerEmphasisScale']
    .map(key => [schema.fields[fieldId(key)].wire, fieldId(key)]))};
`;
}

function styleTypes(schema) {
  const typed = new Set([...styleKinds].filter(kind => !['animation', 'stack-offset'].includes(kind)));
  const dimensions = ['width', 'height', 'minWidth', 'minHeight', 'maxWidth', 'maxHeight'];
  const lines = schema.fields.filter(field => typed.has(field.kind)).map(field => {
    const values = schema.options[field.kind];
    const type = values ? values.map(value => JSON.stringify(value)).join(' | ')
      : field.wire === 'height' || field.wire === 'flexBasis' ? 'number | `${number}%` | "auto"'
      : dimensions.includes(field.wire) ? 'number | `${number}%`'
      : field.kind === 'shadow' ? 'ReadonlyArray<BoxShadow> | "none"'
      : field.kind === 'gradient' ? 'LinearGradient | "none"' : field.kind;
    return `  ${field.wire}?: ${type};`;
  });
  return `${header}export interface BoxShadow { offsetX?: number; offsetY?: number; blur?: number; spread?: number; color?: string }
export interface LinearGradient { direction?: "top" | "right" | "bottom" | "left" | "top-right" | "bottom-right" | "bottom-left" | "top-left"; angle?: number; colorSpace?: "srgb" | "oklab"; stops: ReadonlyArray<{ offset: number; color: string }> }
export interface Style {
  offset?: { x?: number; y?: number };
${lines.join('\n')}
}
`;
}

// Native session tests evaluate the real runtime without the page compiler.
async function nativeTestRuntime() {
  const entry = `import * as ui from './src/internal.js'; import { publishHost } from './src/host.js'; Object.assign(globalThis, ui, { __lingxiaPublish: publishHost });`;
  const result = await build({ stdin: { contents: entry, resolveDir: packageRoot }, bundle: true, write: false, format: 'iife', target: 'es2020', legalComments: 'none', absWorkingDir: packageRoot });
  return header + result.outputFiles[0].text;
}

/** Map of repository-relative path to generated content. */
export async function generate(schema) {
  validate(schema);
  return {
    'crates/lingxia-ui/src/session/tests/runtime.js': await nativeTestRuntime(),
    'crates/lingxia-ui/src/protocol/generated.rs': rust(schema),
    'packages/lingxia-ui/src/generated/wire.js': wire(schema),
    'packages/lingxia-ui/src/generated/style.d.ts': styleTypes(schema),
  };
}

/** Paths whose committed content differs from a fresh generation. */
export async function drift(schema, root = repositoryRoot) {
  const stale = [];
  for (const [path, content] of Object.entries(await generate(schema))) {
    const current = await readFile(join(root, path), 'utf8').catch(() => null);
    if (current?.replace(/\r\n/g, '\n') !== content) stale.push(path);
  }
  return stale;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const schema = await readSchema();
  if (process.argv.includes('--check')) {
    const stale = await drift(schema);
    if (stale.length) {
      console.error(`Generated contract artifacts are stale; run node ${relative(process.cwd(), fileURLToPath(import.meta.url))}:\n  ${stale.join('\n  ')}`);
      process.exit(1);
    }
  } else {
    for (const [path, content] of Object.entries(await generate(schema))) {
      await mkdir(dirname(join(repositoryRoot, path)), { recursive: true });
      await writeFile(join(repositoryRoot, path), content);
    }
  }
}
