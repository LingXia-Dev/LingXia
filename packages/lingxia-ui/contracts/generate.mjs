// Every artifact derived from schema.json. `--check` fails on drift instead of writing.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
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

function wire(schema) {
  const options = schema.fields.filter(field => schema.options[field.kind]).map(field => [field.key, schema.options[field.kind]]);
  return `${header}export const uiAbi = ${schema.uiAbi};
export const maxField = ${schema.fields.length - 1};
export const tags = ${object(schema.tags.map((name, id) => [name, id]))};
export const field = ${object(schema.fields.map(field => [field.key, field.id]))};
export const fieldOptions = ${object(options)};
`;
}

const pascal = name => name.replace(/(^|-)([a-z])/g, (_, __, c) => c.toUpperCase());

/** Style properties an element accepts, from its style groups. */
export function elementStyle(schema, element) {
  const groups = new Map(Object.entries(schema.styleGroups));
  for (const [name, entry] of Object.entries(schema.style)) {
    if (!groups.has(entry.group)) groups.set(entry.group, []);
    if (!groups.get(entry.group).includes(name)) groups.get(entry.group).push(name);
  }
  return [...new Set(element.style.flatMap(group => {
    if (!groups.has(group)) throw Error(`schema.json: unknown style group ${group}`);
    return groups.get(group);
  }))];
}

function validateAuthor(schema) {
  const keys = new Set(schema.fields.map(field => field.key));
  for (const [name, entry] of Object.entries(schema.style))
    for (const target of [entry.wire].flat())
      if (!keys.has(target)) throw Error(`schema.json: style ${name} lowers to unknown field ${target}`);
  for (const [name, element] of Object.entries(schema.elements)) {
    if (!schema.tags.includes(element.wire)) throw Error(`schema.json: element ${name} uses unknown tag ${element.wire}`);
    elementStyle(schema, element);
  }
}

function authoring(schema, api) {
  const style = Object.entries(schema.style).filter(([, entry]) => !entry.internal)
    .map(([name, entry]) => `  ${name}?: ${entry.type};`).join('\n');
  const lines = [];
  for (const [name, element] of Object.entries(schema.elements)) {
    const type = pascal(name);
    const keys = elementStyle(schema, element).filter(key => !schema.style[key].internal).map(key => JSON.stringify(key)).join(' | ');
    lines.push(`export type ${type}Style = Pick<Style, ${keys}>;`);
    const children = { any: 'Child', content: 'Child', text: 'TextChild', none: 'never' }[element.children];
    const props = Object.entries(element.props).map(([prop, entry]) => `${prop}${entry.required ? '' : '?'}: ${entry.type};`);
    const base = `Omit<Props, "style" | "children"> & { style?: ${type}Style; children?: ${children}${props.length ? '; ' + props.join(' ') : ''} }`;
    const binding = element.binding;
    if (binding) {
      const payload = binding.type === 'string' ? 'string' : `ReadonlyData<${binding.type}>`;
      lines.push(`export type ${type}Props = ${base} & (`
        + `{ bind: Local<${binding.type}>; value?: never; ${binding.event}?: never } | `
        + `{ bind?: never; value?: ${binding.type}; ${binding.event}?: (value: ${payload}) => void });`);
    } else lines.push(`export type ${type}Props = ${base};`);
  }
  const intrinsics = Object.keys(schema.elements).map(name => `      ${JSON.stringify(name)}: ${pascal(name)}Props;`).join('\n');
  return `${header}${api.replace(/^\/\/.*\n/gm, '')}
/** CSS-named style; each element accepts the subset its \`*Style\` type lists. */
export interface Style {
${style}
}

${lines.join('\n')}

declare global {
  namespace JSX {
    type ElementType = keyof IntrinsicElements | ((props: any) => ComponentResult);
    interface Element { readonly __ui: unique symbol }
    interface IntrinsicAttributes { key?: string }
    interface ElementChildrenAttribute { children: {} }
    interface IntrinsicElements {
${intrinsics}
    }
  }
}
`;
}

function contract(schema) {
  const elements = Object.fromEntries(Object.entries(schema.elements).map(([name, element]) => [name, {
    wire: element.wire,
    children: element.children,
    style: elementStyle(schema, element),
    props: Object.fromEntries(Object.entries(element.props).map(([prop, entry]) => [prop, Boolean(entry.required)])),
    ...(element.binding ? { binding: element.binding } : {}),
  }]));
  const style = Object.fromEntries(Object.entries(schema.style).map(([name, entry]) => [name, {
    lower: entry.lower, wire: entry.wire, ...(entry.paint ? { paint: true } : {}), ...(entry.internal ? { internal: true } : {}),
  }]));
  return `${header}// Element admission and the author-style lowering table.
export const elements = ${JSON.stringify(elements)};
export const style = ${JSON.stringify(style)};
`;
}

// Native session tests evaluate the real runtime without the page compiler; the
// bundle reads generated modules from this generation, not from disk.
async function nativeTestRuntime(generated) {
  const entry = `import * as ui from './src/internal.js'; import { publishHost } from './src/host.js'; Object.assign(globalThis, ui, { __lingxiaPublish: publishHost });`;
  const fresh = new Map(Object.entries(generated).map(([path, contents]) => [join(repositoryRoot, path), contents]));
  const plugin = { name: 'fresh-contract', setup(builder) {
    builder.onResolve({ filter: /generated\/[^/]+\.js$/ }, ({ path, resolveDir }) => ({ path: resolve(resolveDir, path) }));
    builder.onLoad({ filter: /generated[\\/][^\\/]+\.js$/ }, ({ path }) => fresh.has(path) ? { contents: fresh.get(path), loader: 'js' } : undefined);
  } };
  const result = await build({ stdin: { contents: entry, resolveDir: packageRoot }, bundle: true, write: false, format: 'iife',
    target: 'es2020', legalComments: 'none', absWorkingDir: packageRoot, plugins: [plugin] });
  return header + result.outputFiles[0].text;
}

/** Map of repository-relative path to generated content. */
export async function generate(schema) {
  validate(schema);
  validateAuthor(schema);
  const api = await readFile(join(packageRoot, 'contracts/api.d.ts'), 'utf8');
  const generated = {
    'crates/lingxia-ui/src/protocol/generated.rs': rust(schema),
    'packages/lingxia-ui/src/generated/wire.js': wire(schema),
    'packages/lingxia-ui/src/generated/contract.js': contract(schema),
    'packages/lingxia-ui/src/generated/authoring.d.ts': authoring(schema, api),
  };
  generated['crates/lingxia-ui/src/session/tests/runtime.js'] = await nativeTestRuntime(generated);
  return generated;
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
