// Every artifact derived from schema.json. `--check` fails on drift instead of writing.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import ts from 'typescript';

export const packageRoot = fileURLToPath(new URL('..', import.meta.url));
export const repositoryRoot = join(packageRoot, '../..');

export async function readSchema() {
  return JSON.parse(await readFile(join(packageRoot, 'contracts/schema.json'), 'utf8'));
}

const header = '// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.\n';
const screaming = name => name.replace(/-/g, '_').replace(/([a-z0-9])([A-Z])/g, '$1_$2').toUpperCase();
const object = entries => JSON.stringify(Object.fromEntries(entries));

const childKinds = ['none', 'text', 'content', 'any', 'rows', 'options', 'runs'];
const limitNames = ['nodes', 'depth', 'animationBytes', 'itemKeyBytes', 'imageBytes', 'sourceBytes', 'visibleImageBytes', 'decodedImageBytes', 'runs', 'transitionMs', 'evalBudgetMs', 'evalBudgetDebugMultiplier'];

function validate(schema) {
  const ids = schema.fields.map(field => field.id);
  if (ids.some((id, index) => id !== index)) throw Error('schema.json: field ids must be dense and ordered');
  const keys = new Set();
  for (const field of schema.fields) {
    if (keys.has(field.key)) throw Error(`schema.json: duplicate field key ${field.key}`);
    keys.add(field.key);
    for (const tag of [field.tags ?? [], field.parent ?? []].flat())
      if (!schema.tags.includes(tag)) throw Error(`schema.json: field ${field.key} names unknown tag ${tag}`);
  }
  for (const name of limitNames)
    if (!Number.isSafeInteger(schema.limits?.[name]) || schema.limits[name] <= 0) throw Error(`schema.json: missing limit ${name}`);
  for (const [token, pair] of Object.entries(schema.defaultColors ?? {}))
    if (!Array.isArray(pair) || pair.length !== 2 || pair.some(c => !/^#[0-9a-f]{6}$/.test(c))) throw Error(`schema.json: default color ${token} needs a light and dark #rrggbb`);
  for (const [key, entry] of Object.entries(schema.environment ?? {}))
    if (!['none', 'external', ...Object.keys(environmentChecks)].includes(entry.check) || !['device', 'push', 'session'].includes(entry.origin) || typeof entry.type !== 'string')
      throw Error(`schema.json: environment ${key} needs a type, a known check and an origin`);
  const admitted = Object.keys(schema.admission ?? {});
  if (admitted.join() !== schema.tags.join()) throw Error('schema.json: admission must list every tag in tag order');
  for (const [tag, entry] of Object.entries(schema.admission)) {
    if (!childKinds.includes(entry.children)) throw Error(`schema.json: tag ${tag} has unknown children ${entry.children}`);
    if (entry.parent !== undefined && !schema.tags.includes(entry.parent)) throw Error(`schema.json: tag ${tag} names unknown parent ${entry.parent}`);
    for (const field of entry.requires ?? [])
      if (!keys.has(field)) throw Error(`schema.json: tag ${tag} requires unknown field ${field}`);
  }
}

const environmentChecks = {
  enum: (key, entry) => `matches!(value[${JSON.stringify(key)}].as_str(), ${entry.values.map(v => `Some(${JSON.stringify(v)})`).join(' | ')})`,
  bool: key => `value[${JSON.stringify(key)}].is_boolean()`,
  range: (key, entry) => `value[${JSON.stringify(key)}].as_f64().is_some_and(|n| n.is_finite() && (${entry.range[0]}.0..=${entry.range[1]}.0).contains(&n))`,
  name: key => `value[${JSON.stringify(key)}].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128)`,
  edges: key => `["top", "right", "bottom", "left"].iter().all(|edge| value[${JSON.stringify(key)}][edge].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n)))`,
  keyboard: key => `value[${JSON.stringify(key)}]["height"].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))`,
  renderer: key => `(matches!(value[${JSON.stringify(key)}]["fallback"].as_str(), None | Some("driver")) && value[${JSON.stringify(key)}].as_object().is_some_and(|r| r.len() == 1))`,
};

/** The native environment snapshot check, from the schema's \`environment\` section. */
function environmentRust(schema) {
  const checks = Object.entries(schema.environment).filter(([, entry]) => environmentChecks[entry.check]).map(([key, entry]) => {
    const check = environmentChecks[entry.check](key, entry);
    return entry.optional ? `(value.get(${JSON.stringify(key)}).is_none() || ${check})` : check;
  });
  const keys = Object.entries(schema.environment).map(([key, entry]) => `    (${JSON.stringify(key)}, ${JSON.stringify(entry.origin)}),`).join('\n');
  return `/// Environment keys and where each comes from: \`device\` facts, a host \`push\`, or the \`session\`.
pub const ENVIRONMENT_KEYS: &[(&str, &str)] = &[
${keys}
];

/// Whether a native environment snapshot (device facts plus pushes) is well formed.
pub fn check_environment(value: &serde_json::Value) -> bool {
    value.is_object()
        && ${checks.join('\n        && ')}
}`;
}

function rust(schema) {
  const tags = schema.tags.map((name, id) => `    pub const ${screaming(name)}: usize = ${id};`).join('\n');
  const fields = schema.fields.map(field => `    pub const ${screaming(field.key)}: usize = ${field.id};`).join('\n');
  const table = schema.fields.map(field => `    (${JSON.stringify(field.wire)}, ${JSON.stringify(field.kind)}),`).join('\n');
  const options = Object.entries(schema.options)
    .map(([kind, values]) => `        ${JSON.stringify(kind)} => Some(&[${values.map(value => JSON.stringify(value)).join(', ')}]),`).join('\n');
  const tagConst = name => `tag::${screaming(name)}`;
  const fieldConst = key => `field::${screaming(key)}`;
  const children = kind => kind.replace(/(^|-)([a-z])/g, (_, __, c) => c.toUpperCase());
  const admission = schema.tags.map(name => {
    const entry = schema.admission[name];
    return `    TagAdmission { children: Children::${children(entry.children)}, content: ${Boolean(entry.content)}, `
      + `parent: ${entry.parent ? `Some(${tagConst(entry.parent)})` : 'None'}, root: ${Boolean(entry.root)}, keyed: ${Boolean(entry.keyed)}, `
      + `requires: &[${(entry.requires ?? []).map(fieldConst).join(', ')}] },`;
  }).join('\n');
  const fieldAdmission = schema.fields.map(field => `    FieldAdmission { tags: ${field.tags ? `Some(&[${field.tags.map(tagConst).join(', ')}])` : 'None'}, `
    + `parent: ${field.parent ? `Some(${tagConst(field.parent)})` : 'None'} },`).join('\n');
  const limits = limitNames.map(name => `    pub const ${screaming(name)}: usize = ${schema.limits[name]};`).join('\n');
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

/// Page-wide bounds both sides of the wire enforce.
pub mod limits {
${limits}
}

${environmentRust(schema)}

/// Colors of unstyled elements as (token, light, dark), until the theme system supplies them.
pub const DEFAULT_COLORS: &[(&str, &str, &str)] = &[
${Object.entries(schema.defaultColors).map(([token, [light, dark]]) => `    (${JSON.stringify(token)}, ${JSON.stringify(light.slice(1))}, ${JSON.stringify(dark.slice(1))}),`).join('\n')}
];

/// What a wire tag may contain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Children {
    /// A leaf.
    None,
    /// A leaf carrying inline text.
    Text,
    /// Inline text, or noninteractive content descendants.
    Content,
    Any,
    /// Virtual list row wrappers.
    Rows,
    /// Only tags whose admission names this tag as parent.
    Options,
    /// Inline text, or only run tags whose admission names this tag as parent.
    Runs,
}

#[derive(Clone, Copy, Debug)]
pub struct TagAdmission {
    pub children: Children,
    /// May sit inside a \`Children::Content\` control.
    pub content: bool,
    pub parent: Option<usize>,
    pub root: bool,
    /// Requires a non-empty key.
    pub keyed: bool,
    pub requires: &'static [usize],
}

/// Indexed by tag id.
pub const TAG_ADMISSION: &[TagAdmission] = &[
${admission}
];

/// The tags a field applies to (\`None\`: every tag) and the parent tag it requires.
#[derive(Clone, Copy, Debug)]
pub struct FieldAdmission {
    pub tags: Option<&'static [usize]>,
    pub parent: Option<usize>,
}

/// Indexed by field id.
pub const FIELD_ADMISSION: &[FieldAdmission] = &[
${fieldAdmission}
];
`;
}

function lxappLimits(schema) {
  return `${header}//! GPU page resource bounds lxapp enforces before bytes reach the View.

/// Largest encoded image a GPU page may load.
pub(crate) const IMAGE_BYTES: usize = ${schema.limits.imageBytes};
/// Longest image source string.
pub(crate) const SOURCE_BYTES: usize = ${schema.limits.sourceBytes};
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
    const children = { any: 'Child', content: 'Child', text: 'TextChild', runs: 'RunChild', none: 'never' }[element.children];
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
/** The page's environment (\`view.env\`): read-only, and a change re-renders the page. */
export interface Env {
${Object.entries(schema.environment).filter(([, entry]) => !entry.internal).map(([key, entry]) => `${entry.doc ? `  /** ${entry.doc} */\n` : ''}  readonly ${key}${entry.absent ? '?' : ''}: ${entry.type};`).join('\n')}
}

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

/** Member names of `interface Props` in api.d.ts: the props every element accepts. */
function commonProps(api) {
  const file = ts.createSourceFile('api.d.ts', api, ts.ScriptTarget.ES2020, true);
  const names = [];
  const visit = node => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === 'Props')
      for (const member of node.members) if (ts.isPropertySignature(member)) names.push(member.name.getText(file));
    ts.forEachChild(node, visit);
  };
  visit(file);
  if (!names.length) throw Error('api.d.ts: interface Props not found');
  return names;
}

function contract(schema, common) {
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
  const css = {}, keywords = {};
  for (const [name, entry] of Object.entries(schema.style)) {
    for (const property of [entry.css ?? []].flat()) {
      if (css[property]) throw Error(`schema.json: css ${property} sets both ${css[property]} and ${name}`);
      css[property] = name;
    }
    const literals = entry.type.split('|').map(part => part.trim());
    if (literals.every(part => /^"[^"]*"$/.test(part))) keywords[name] = literals.map(part => JSON.parse(part));
  }
  const admission = {
    tags: schema.admission,
    fields: Object.fromEntries(schema.fields.filter(field => field.tags).map(field => [field.key, field.tags])),
  };
  return `${header}// Element admission and the author-style lowering table.
export const elements = ${JSON.stringify(elements)};
export const style = ${JSON.stringify(style)};
/** Page-wide bounds native decode enforces too. */
export const limits = ${JSON.stringify(schema.limits)};
/** Wire-tag nesting and the tags each restricted field applies to. */
export const admission = ${JSON.stringify(admission)};
/** CSS longhand to the author style key it sets. */
export const css = ${JSON.stringify(css)};
/** Keyword-valued style keys and the author values they accept. */
export const keywords = ${JSON.stringify(keywords)};
/** Enumerated values by option kind. */
export const options = ${JSON.stringify(schema.options)};
/** Props every element accepts, read from \`interface Props\` in contracts/api.d.ts. */
export const commonProps = ${JSON.stringify(common)};
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

const constant = name => screaming(name).replace(/__+/g, '_');

// The Android adapter maps the semantic projection with these names only.
function androidSemantics(schema) {
  const { roles, liveRegions, events, actions, keys } = schema.semantics;
  const lines = [
    ...roles.map(role => `    public static final String ROLE_${constant(role)} = ${JSON.stringify(role)};`),
    ...liveRegions.map(kind => `    public static final String LIVE_${constant(kind)} = ${JSON.stringify(kind)};`),
    ...events.map(kind => `    public static final String EVENT_${constant(kind)} = ${JSON.stringify(kind)};`),
    ...actions.map(kind => `    public static final String ACTION_${constant(kind)} = ${JSON.stringify(kind)};`),
    ...Object.entries(keys).map(([name, key]) => `    public static final String KEY_${constant(name)} = ${JSON.stringify(key)};`),
  ];
  return `${header}package com.lingxia.ui.generated;

/** Semantic node roles, live-region kinds, event kinds and JSON keys of the shared projection. */
public final class GpuSemantics {
    private GpuSemantics() {}

${lines.join('\n')}
}
`;
}

// Editor attributes the Android input connection maps onto EditorInfo and autofill.
function androidInput(schema) {
  const { autofill, keys } = schema.input;
  const options = kind => schema.options[kind].map(value => `    public static final String ${kind === 'enter-key-hint' ? 'ENTER' : kind === 'auto-capitalize' ? 'CAPITALIZE' : 'KEYBOARD'}_${constant(value)} = ${JSON.stringify(value)};`);
  const hints = Object.entries(autofill).map(([token, hint]) => `            case ${JSON.stringify(token)}: return ${JSON.stringify(hint)};`);
  return `${header}package com.lingxia.ui.generated;

/** Editor keyboards, IME actions, capitalization, autofill hints and snapshot keys. */
public final class GpuInput {
    private GpuInput() {}

${[...options('input-keyboard'), ...options('enter-key-hint'), ...options('auto-capitalize'),
  ...Object.entries(keys).map(([name, key]) => `    public static final String KEY_${constant(name)} = ${JSON.stringify(key)};`)].join('\n')}

    /** The Android autofill hint for an HTML autocomplete token, or null. */
    public static String autofillHint(String token) {
        switch (token) {
${hints.join('\n')}
            default: return null;
        }
    }
}
`;
}

/** Map of repository-relative path to generated content. */
export async function generate(schema) {
  validate(schema);
  validateAuthor(schema);
  const api = await readFile(join(packageRoot, 'contracts/api.d.ts'), 'utf8');
  const generated = {
    'crates/lingxia-ui/src/protocol/generated.rs': rust(schema),
    'crates/lingxia-lxapp/src/page/ui_limits.rs': lxappLimits(schema),
    'packages/lingxia-ui/src/generated/wire.js': wire(schema),
    'packages/lingxia-ui/src/generated/contract.js': contract(schema, commonProps(api)),
    'packages/lingxia-ui/src/generated/authoring.d.ts': authoring(schema, api),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/ui/generated/GpuSemantics.java': androidSemantics(schema),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/ui/generated/GpuInput.java': androidInput(schema),
  };
  generated['crates/lingxia-ui/src/session/tests/runtime.js'] = await nativeTestRuntime(generated);
  Object.assign(generated, await (await import('./docs.mjs')).docs(schema, generated));
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

// Not top-level await: docs.mjs imports this module back, and would wait on its evaluation.
const main = async () => {
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
};
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1])
  main().catch(error => { console.error(error); process.exit(1); });
