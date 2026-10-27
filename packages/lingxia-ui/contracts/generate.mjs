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

function validate(schema) {
  validateRoles(schema.roles ?? []);
  const ids = schema.fields.map(field => field.id);
  if (ids.some((id, index) => id !== index)) throw Error('schema.json: field ids must be dense and ordered');
  const keys = new Set();
  for (const field of schema.fields) {
    if (keys.has(field.key)) throw Error(`schema.json: duplicate field key ${field.key}`);
    keys.add(field.key);
    for (const tag of [field.tags ?? [], field.parent ?? []].flat())
      if (!schema.tags.includes(tag)) throw Error(`schema.json: field ${field.key} names unknown tag ${tag}`);
  }
  for (const [name, value] of Object.entries(schema.limits ?? {}))
    if (!Number.isSafeInteger(value) || value <= 0) throw Error(`schema.json: limit ${name} must be a positive integer`);
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

const roleVariant = name => name.split('-').map(part => part[0].toUpperCase() + part.slice(1)).join('');

function validateRoles(roles) {
  const names = roles.map(role => role.name);
  if (!names.length || new Set(names).size !== names.length) throw Error('schema.json: role names must be unique');
  for (const role of roles) {
    if (!/^[a-z]+(?:-[a-z0-9]+)*$/.test(role.name) || typeof role.doc !== 'string') throw Error(`schema.json: role ${role.name} needs a kebab-case name and a doc`);
    for (const [background, ratio] of Object.entries(role.on ?? {}))
      if (!names.includes(background) || !(ratio >= 1 && ratio <= 21)) throw Error(`schema.json: role ${role.name} needs a known background and a 1-21 contrast`);
  }
}

function themeRust(schema) {
  const roles = schema.roles;
  const variants = roles.map((role, slot) => `    /// ${role.doc}\n    ${roleVariant(role.name)} = ${slot},`).join('\n');
  const pairs = roles.flatMap(role => Object.entries(role.on ?? {}).map(([background, ratio]) =>
    `    (Role::${roleVariant(role.name)}, Role::${roleVariant(background)}, ${Number(ratio).toFixed(1)}),`)).join('\n');
  return `// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.
//! Roles are palette slots in order; append-only.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Role {
${variants}
}

pub const ROLE_COUNT: usize = ${roles.length};

impl Role {
    pub const ALL: [Role; ROLE_COUNT] = [
${roles.map(role => `        Role::${roleVariant(role.name)},`).join('\n')}
    ];

    pub const fn name(self) -> &'static str {
        match self {
${roles.map(role => `            Role::${roleVariant(role.name)} => ${JSON.stringify(role.name)},`).join('\n')}
        }
    }

    pub fn from_name(name: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| role.name() == name)
    }
}

/// (foreground, background, minimum WCAG contrast) every resolved palette meets.
pub const CONTRAST: &[(Role, Role, f32)] = &[
${pairs}
];
`;
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
  const limits = Object.keys(schema.limits).map(name => `    pub const ${screaming(name)}: usize = ${schema.limits[name]};`).join('\n');
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
/** Colour roles in palette-slot order. */
export const roles = ${JSON.stringify(schema.roles.map(role => role.name))};
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

    /** The core's editor limits; an input mirror holds itself to them. */
${['editorTextBytes', 'inputRequestBytes', 'inputBatchEdits', 'clipboardItems'].map(name => `    public static final int LIMIT_${constant(name)} = ${schema.limits[name]};`).join('\n')}

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


// ---- frame contract decoders ----------------------------------------------------------------

const camel = key => key.replace(/[-_]([a-z0-9])/g, (_, c) => c.toUpperCase());
const typeName = name => name;

/** Parses a frame type string: `{ base, optional, dims: [n|null...] }`. */
function frameType(ty) {
  let optional = false, dims = [];
  if (ty.endsWith('?')) { optional = true; ty = ty.slice(0, -1); }
  for (let m; (m = ty.match(/\[(\d*)\]$/)); ty = ty.slice(0, m.index)) dims.unshift(m[1] === '' ? null : Number(m[1]));
  return { base: ty, optional, dims };
}

function frameJava(schema) {
  const { types, messages, doc } = schema.frame;
  const prim = { string: 'String', bool: 'boolean', u8: 'long', u32: 'long', u64: 'long', usize: 'long', i64: 'long', f32: 'float', f64: 'double', any: 'Object' };
  const boxed = { boolean: 'Boolean', long: 'Long', float: 'Float', double: 'Double' };
  const kindOf = base => prim[base] ? 'prim' : types[base]?.enum ? 'enum' : 'object';
  const javaType = ({ base, dims, optional }) => {
    const element = kindOf(base) === 'prim' ? prim[base] : kindOf(base) === 'enum' ? 'String' : base;
    if (dims.length) return element + '[]'.repeat(dims.length);
    return optional && boxed[element] ? boxed[element] : element;
  };
  const reader = (key, t) => {
    const k = JSON.stringify(key);
    if (t.dims.length) {
      const element = kindOf(t.base) === 'prim' ? prim[t.base] : kindOf(t.base) === 'enum' ? 'String' : t.base;
      return `return GpuFrame.array(json.optJSONArray(${k}), ${t.dims.length}, ${JSON.stringify(element)});`
        .replace(/^return GpuFrame\.array\((.*)\);$/, (_, args) => `return (${javaType(t)}) GpuFrame.array(${args});`);
    }
    const kind = kindOf(t.base);
    if (kind === 'object') return `JSONObject o = json.optJSONObject(${k}); return o == null ? null : new ${t.base}(o);`;
    const javaT = kind === 'enum' ? 'String' : prim[t.base];
    if (t.optional && javaT !== 'String' && javaT !== 'Object')
      return `if (json.isNull(${k})) return null; return ${opt(javaT, k)};`;
    if (javaT === 'String') return `return json.isNull(${k}) ? null : json.optString(${k}, null);`;
    if (javaT === 'Object') return `return json.isNull(${k}) ? null : json.opt(${k});`;
    return `return ${opt(javaT, k)};`;
  };
  const opt = (javaT, k) => ({ boolean: `json.optBoolean(${k})`, long: `json.optLong(${k})`, float: `(float) json.optDouble(${k})`, double: `json.optDouble(${k})` })[javaT];
  const fieldsOf = def => def.tagged
    ? Object.fromEntries([[def.tagged, 'string'], ...Object.values(def.variants).flatMap(fields => Object.entries(fields).map(([k, v]) => [k, v.endsWith('?') ? v : `${v}?`]))])
    : def;
  const classes = [], constants = [];
  for (const [name, def] of Object.entries(types)) {
    if (def.enum) { constants.push(...def.enum.map(value => `    public static final String ${constant(name)}_${constant(value)} = ${JSON.stringify(value)};`)); continue; }
    if (def.tagged) constants.push(...Object.keys(def.variants).map(value => `    public static final String ${constant(name)}_${constant(value)} = ${JSON.stringify(value)};`));
    const methods = Object.entries(fieldsOf(def)).filter(([key]) => key !== '...').map(([key, ty]) => {
      const t = frameType(ty);
      return `        /** \`${key}\`: ${ty} */\n        public ${javaType(t)} ${camel(key) === 'class' ? 'klass' : camel(key)}() { ${reader(key, t)} }`;
    });
    classes.push(`    public static final class ${name} {
        public final JSONObject json;
        public ${name}(JSONObject json) { this.json = json; }
${methods.join('\n')}
    }`);
  }
  return `${header}package com.lingxia.ui.generated;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Typed readers over the frame contract (schema.json \`frame\`): ${doc.split(':')[0]}.
 * Messages: ${Object.entries(messages).map(([name, type]) => `${name} = ${type}`).join(', ')}.
 */
public final class GpuFrame {
    private GpuFrame() {}

${constants.join('\n')}

${classes.join('\n\n')}

    /** A JSON list as a Java array of \`element\` (nested \`depth\` deep); null when absent. */
    static Object array(JSONArray list, int depth, String element) {
        if (list == null) return null;
        int n = list.length();
        if (depth > 1) {
            Object[] out = (Object[]) java.lang.reflect.Array.newInstance(arrayClass(element, depth - 1), n);
            for (int i = 0; i < n; i++) out[i] = array(list.optJSONArray(i), depth - 1, element);
            return out;
        }
        switch (element) {
            case "float": { float[] out = new float[n]; for (int i = 0; i < n; i++) out[i] = (float) list.optDouble(i); return out; }
            case "double": { double[] out = new double[n]; for (int i = 0; i < n; i++) out[i] = list.optDouble(i); return out; }
            case "long": { long[] out = new long[n]; for (int i = 0; i < n; i++) out[i] = list.optLong(i); return out; }
            case "boolean": { boolean[] out = new boolean[n]; for (int i = 0; i < n; i++) out[i] = list.optBoolean(i); return out; }
            case "String": { String[] out = new String[n]; for (int i = 0; i < n; i++) out[i] = list.isNull(i) ? null : list.optString(i); return out; }
            case "Object": { Object[] out = new Object[n]; for (int i = 0; i < n; i++) out[i] = list.opt(i); return out; }
            default: {
                Object[] out = (Object[]) java.lang.reflect.Array.newInstance(arrayClass(element, 0), n);
                for (int i = 0; i < n; i++) out[i] = wrap(element, list.optJSONObject(i));
                return out;
            }
        }
    }

    private static Class<?> arrayClass(String element, int depth) {
        Class<?> base;
        switch (element) {
            case "float": base = float.class; break;
            case "double": base = double.class; break;
            case "long": base = long.class; break;
            case "boolean": base = boolean.class; break;
            case "String": base = String.class; break;
            case "Object": base = Object.class; break;
            default: base = typeClass(element);
        }
        for (int i = 0; i < depth; i++) base = java.lang.reflect.Array.newInstance(base, 0).getClass();
        return base;
    }

    private static Class<?> typeClass(String name) {
        switch (name) {
${Object.entries(types).filter(([, def]) => !def.enum).map(([name]) => `            case ${JSON.stringify(name)}: return ${name}.class;`).join('\n')}
            default: throw new IllegalArgumentException(name);
        }
    }

    private static Object wrap(String name, JSONObject json) {
        if (json == null) return null;
        switch (name) {
${Object.entries(types).filter(([, def]) => !def.enum).map(([name]) => `            case ${JSON.stringify(name)}: return new ${name}(json);`).join('\n')}
            default: throw new IllegalArgumentException(name);
        }
    }
}
`;
}

function frameSwift(schema) {
  const { types, messages } = schema.frame;
  const prim = { string: 'String', bool: 'Bool', u8: 'UInt8', u32: 'UInt32', u64: 'UInt64', usize: 'UInt64', i64: 'Int64', f32: 'Float', f64: 'Double', any: 'GpuJSON' };
  const swiftType = ty => {
    const t = frameType(ty);
    let element = prim[t.base] ?? (types[t.base]?.enum ? 'String' : `GpuFrame${t.base}`);
    for (const _ of t.dims) element = `[${element}]`;
    return t.optional ? `${element}?` : element;
  };
  const fieldsOf = def => def.tagged
    ? Object.fromEntries([[def.tagged, 'string'], ...Object.values(def.variants).flatMap(fields => Object.entries(fields).map(([k, v]) => [k, v.endsWith('?') ? v : `${v}?`]))])
    : def;
  const structs = Object.entries(types).filter(([, def]) => !def.enum).map(([name, def]) => {
    const fields = Object.entries(fieldsOf(def)).filter(([key]) => key !== '...');
    const props = fields.map(([key, ty]) => `    public let ${camel(key)}: ${swiftType(ty)}`);
    const keys = fields.map(([key]) => `        case ${camel(key)} = ${JSON.stringify(key)}`);
    return `/// \`${name}\` in the frame contract.
public struct GpuFrame${name}: Decodable, Sendable {
${props.join('\n')}

    enum CodingKeys: String, CodingKey {
${keys.join('\n')}
    }
}`;
  });
  const enums = Object.entries(types).filter(([, def]) => def.enum).map(([name, def]) =>
    `/// \`${name}\` values.\npublic enum GpuFrame${name}: String, Sendable {\n${def.enum.map(value => `    case ${camel(value)} = ${JSON.stringify(value)}`).join('\n')}\n}`);
  return `// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.
// The frame contract every Apple platform decodes: ${Object.entries(messages).map(([name, type]) => `${name} = ${type}`).join(', ')}.
import Foundation

/// Any JSON value, for contract fields typed \`any\`.
public enum GpuJSON: Decodable, Sendable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([GpuJSON])
    case object([String: GpuJSON])

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(Double.self) { self = .number(value) }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode([GpuJSON].self) { self = .array(value) }
        else { self = .object(try container.decode([String: GpuJSON].self)) }
    }
}

${enums.join('\n\n')}

${structs.join('\n\n')}
`;
}

/** Map of repository-relative path to generated content. */
export async function generate(schema) {
  validate(schema);
  validateAuthor(schema);
  const api = await readFile(join(packageRoot, 'contracts/api.d.ts'), 'utf8');
  const generated = {
    'crates/lingxia-ui/src/protocol/generated.rs': rust(schema),
    'crates/lingxia-theme/src/generated.rs': themeRust(schema),
    'crates/lingxia-lxapp/src/page/ui_limits.rs': lxappLimits(schema),
    'packages/lingxia-ui/src/generated/wire.js': wire(schema),
    'packages/lingxia-ui/src/generated/contract.js': contract(schema, commonProps(api)),
    'packages/lingxia-ui/src/generated/authoring.d.ts': authoring(schema, api),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/ui/generated/GpuSemantics.java': androidSemantics(schema),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/ui/generated/GpuInput.java': androidInput(schema),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/ui/generated/GpuFrame.java': frameJava(schema),
    'lingxia-sdk/apple/Sources/Platform/Gpu/GpuFrame.swift': frameSwift(schema),
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
    // Unchanged outputs keep their bytes and mtime: platform builds watch some of them.
    for (const [path, content] of Object.entries(await generate(schema))) {
      const target = join(repositoryRoot, path);
      if (await readFile(target, 'utf8').catch(() => undefined) === content) continue;
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, content);
      console.log(`wrote ${path}`);
    }
  }
};
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1])
  main().catch(error => { console.error(error); process.exit(1); });
