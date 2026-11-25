// Every artifact derived from schema.json. `--check` fails on drift instead of writing.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
// Java accessors may not reuse a keyword or an Object method (`wait()`), so those take a suffix.
const javaMethod = name => name === 'class' ? 'klass' : ['wait', 'notify', 'notifyAll', 'getClass', 'hashCode', 'toString', 'equals', 'clone', 'finalize'].includes(name) ? `${name}Value` : name;

export const contractsRoot = fileURLToPath(new URL('.', import.meta.url));
export const repositoryRoot = join(contractsRoot, '../..');
export const packageRoot = join(repositoryRoot, 'packages/lingxia-view');
// esbuild, TypeScript and Tailwind are @lingxia/view's dependencies: resolve them from there.
export const { resolveDependency, loadDependency } = await import(pathToFileURL(join(packageRoot, 'compiler/dependencies.mjs')).href);
const { build } = await loadDependency('esbuild');
const ts = (await loadDependency('typescript')).default;

export async function readSchema() {
  return JSON.parse(await readFile(join(contractsRoot, 'schema.json'), 'utf8'));
}

const header = '// Generated from contracts/view/schema.json; do not edit.\n';
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
    if (field.ignorable !== undefined && field.ignorable !== true) throw Error(`schema.json: field ${field.key} ignorable must be true or absent`);
  }
  validateScales(schema);
  for (const [name, value] of Object.entries(schema.limits ?? {}))
    if (!Number.isSafeInteger(value) || value <= 0) throw Error(`schema.json: limit ${name} must be a positive integer`);
  for (const [key, entry] of Object.entries(schema.environment ?? {}))
    if (!['none', 'external', ...Object.keys(environmentChecks)].includes(entry.check) || !['device', 'push', 'session'].includes(entry.origin) || typeof entry.type !== 'string')
      throw Error(`schema.json: environment ${key} needs a type, a known check and an origin`);
  // A reserved tag id keeps its slot: nothing may name it, and native rejects it on the wire.
  const reserved = tag => tag.startsWith('reserved-');
  for (const field of schema.fields)
    if ([field.tags ?? [], field.parent ?? []].flat().some(reserved)) throw Error(`schema.json: field ${field.key} names a reserved tag`);
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
  capsule: key => `(value[${JSON.stringify(key)}].is_null() || (["x", "y", "width", "height"].iter().all(|side| value[${JSON.stringify(key)}][side].as_f64().is_some_and(|n| n.is_finite() && (-16384.0..=16384.0).contains(&n))) && ["width", "height"].iter().all(|side| value[${JSON.stringify(key)}][side].as_f64().is_some_and(|n| n >= 0.0))))`,
  keyboard: key => `value[${JSON.stringify(key)}]["height"].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))`,
  hinge: key => `(value[${JSON.stringify(key)}].is_null() || (value[${JSON.stringify(key)}].is_object() && ["x", "y", "width", "height"].iter().all(|k| value[${JSON.stringify(key)}][k].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))) && matches!(value[${JSON.stringify(key)}]["orientation"].as_str(), Some("vertical" | "horizontal")) && matches!(value[${JSON.stringify(key)}]["posture"].as_str(), Some("flat" | "half-opened"))))`,
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

function validateScales({ scales, scaledFields, fields }) {
  const names = scales.map(slot => slot.name);
  if (new Set(names).size !== names.length) throw Error('schema.json: scale slot names must be unique');
  const keys = new Set(fields.map(field => field.key));
  for (const slot of scales) {
    if (!/^[a-z]+(?:\.[A-Za-z0-9]+)?$/.test(slot.name) || typeof slot.doc !== 'string') throw Error(`schema.json: scale ${slot.name} needs a dotted name and a doc`);
    if (!scaledFields[slot.family]) throw Error(`schema.json: scale ${slot.name} has a family no field takes`);
  }
  for (const [family, members] of Object.entries(scaledFields))
    for (const key of members) if (!keys.has(key)) throw Error(`schema.json: scaledFields.${family} names unknown field ${key}`);
}

// The built-in pack's value per slot; `em` has none, it is the element's own font size.
function scaleDefaults({ scales }) {
  const pack = JSON.parse(readFileSync(join(repositoryRoot, 'crates/lingxia-theme/packs/default.json'), 'utf8'));
  return scales.map(({ pack: path }) => {
    if (!path) return null;
    const value = path.split('.').reduce((at, part) => at?.[part], pack);
    if (value === undefined) throw Error(`schema.json: scale pack path ${path} is not in the default pack`);
    const bezier = typeof value === 'string' && value.match(/^cubic-bezier\(([^)]*)\)$/);
    return bezier ? bezier[1].split(',').map(Number) : value;
  });
}

function validateRoles(roles) {
  const names = roles.map(role => role.name);
  if (!names.length || new Set(names).size !== names.length) throw Error('schema.json: role names must be unique');
  for (const role of roles) {
    if (!/^[a-z]+(?:-[a-z0-9]+)*$/.test(role.name) || typeof role.doc !== 'string') throw Error(`schema.json: role ${role.name} needs a kebab-case name and a doc`);
    if (!['authored', 'fixed', 'derived'].includes(role.tier)) throw Error(`schema.json: role ${role.name} needs a tier: authored, fixed or derived`);
    for (const [background, ratio] of Object.entries(role.on ?? {}))
      if (!names.includes(background) || !(ratio >= 1 && ratio <= 21)) throw Error(`schema.json: role ${role.name} needs a known background and a 1-21 contrast`);
  }
}

function themeRust(schema) {
  const roles = schema.roles;
  const variants = roles.map((role, slot) => `    /// ${role.doc}\n    ${roleVariant(role.name)} = ${slot},`).join('\n');
  const pairs = roles.flatMap(role => Object.entries(role.on ?? {}).map(([background, ratio]) =>
    `    (Role::${roleVariant(role.name)}, Role::${roleVariant(background)}, ${Number(ratio).toFixed(1)}),`)).join('\n');
  return `// Generated from contracts/view/schema.json; do not edit.
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

    pub const fn tier(self) -> Tier {
        match self {
${['authored', 'fixed'].map(tier => `            ${roles.filter(role => role.tier === tier).map(role => `Role::${roleVariant(role.name)}`).join('\n            | ')} => Tier::${roleVariant(tier)},`).join('\n')}
            _ => Tier::Derived,
        }
    }
}

/// Who gives a role its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// The pack writes it.
    Authored,
    /// The same in every pack unless a pack sets it.
    Fixed,
    /// Computed from the pack's authored roles unless a pack sets it.
    Derived,
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
  const id = new Map(schema.fields.map(field => [field.key, field.id]));
  // The fields some style keys lower to, once each, in wire order.
  const wireFields = names => [...new Set(names.flatMap(name => [].concat(schema.style[name].wire)))].sort((a, b) => id.get(a) - id.get(b));
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
/// Scale slots by wire index, with their family; append-only like field ids.
pub const SCALES: &[(&str, &str)] = &[
${schema.scales.map(slot => `    (${JSON.stringify(slot.name)}, ${JSON.stringify(slot.family)}),`).join('\n')}
];

/// What a slider part record may carry: the wire fields of the \`part\` style group.
pub const PART_FIELDS: &[usize] = &[${wireFields(schema.styleGroups.part).map(fieldConst).join(', ')}];
/// Paint-only fields a state delta may carry: the wire fields of every \`paint\` style.
pub const FOCUS_PAINT_FIELDS: &[usize] = &[${wireFields(Object.keys(schema.style).filter(name => schema.style[name].paint)).map(fieldConst).join(', ')}];

/// Fields that take a \`[slot, times, plus]\` reference, by the family of the slot.
pub const SCALED_FIELDS: &[(&str, &[usize])] = &[
${Object.entries(schema.scaledFields).map(([family, keys]) => `    (${JSON.stringify(family)}, &[${keys.map(fieldConst).join(', ')}]),`).join('\n')}
];

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
// Accessibility-only fields a page drops, with one warning, on a host older than them.
export const ignorableFields = ${JSON.stringify(schema.fields.filter(field => field.ignorable).map(field => field.key))};
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
    for (const form of [element, element.block].filter(Boolean)) {
      if (!schema.tags.includes(form.wire) || form.wire.startsWith('reserved-')) throw Error(`schema.json: element ${name} uses unknown tag ${form.wire}`);
      elementStyle(schema, form);
    }
  }
}

// "an <input> or a <button>".
const tagList = tags => tags.map(tag => `${/^[aeiou]/.test(tag) ? 'an' : 'a'} <${tag}>`).join(' or ');

/** schema.json `rules` with every default message filled in: what the runtime, the compiler and the docs read. */
export function rules(schema, common) {
  const { role = {}, elementRole = {}, prop = {}, run = {}, trait = {}, exclusive = {} } = schema.rules ?? {};
  const kinds = new Set([...Object.keys(schema.elements), ...Object.values(schema.elements).filter(element => element.block).map(element => element.block.wire)]);
  const known = (where, tags) => { for (const tag of tags ?? []) if (!kinds.has(tag)) throw Error(`schema.json: rules.${where} names unknown element ${tag}`); };
  const roles = new Set(schema.options.role);
  const out = { role: {}, elementRole: {}, prop: {}, run: {}, trait: {}, exclusive: {} };
  const rolesKnown = (where, list) => { for (const name of list ?? []) if (!roles.has(name)) throw Error(`schema.json: rules.${where} names unknown role ${name}`); };
  for (const [name, list] of Object.entries(role)) {
    if (!roles.has(name)) throw Error(`schema.json: rules.role names unknown role ${name}`);
    out.role[name] = list.map(rule => {
      known(`role.${name}`, rule.tags);
      return { ...rule, error: rule.error ?? `role="${name}" belongs on ${tagList(rule.tags)}` };
    });
  }
  for (const [tag, rule] of Object.entries(elementRole)) {
    known('elementRole', [tag]);
    rolesKnown(`elementRole.${tag}`, [...rule.roles, ...(rule.implied ? [rule.implied] : [])]);
    out.elementRole[tag] = rule;
  }
  const props = new Set([...(common ?? []), ...Object.values(schema.elements).flatMap(element => Object.keys(element.props))]);
  // A trait's props need its key, and a form of the key may refuse some: the same rules as any prop's.
  for (const [name, entry] of Object.entries(trait)) {
    known(`trait.${name}`, entry.on);
    // The key is on every element of the trait; a member an element lacks is simply not there.
    for (const tag of entry.on) if (!schema.elements[tag].props[entry.key]) throw Error(`schema.json: rules.trait.${name}: <${tag}> has no prop ${entry.key}`);
    for (const member of entry.props) if (!entry.on.some(tag => schema.elements[tag].props[member])) throw Error(`schema.json: rules.trait.${name}: no element has ${member}`);
    out.trait[name] = entry;
    for (const member of entry.props) {
      const derived = [{ on: entry.on, with: [entry.key], error: entry.error.replace('{prop}', member) }];
      for (const form of entry.forms ?? []) if (form.without?.[member])
        derived.push({ on: entry.on, not: { [entry.key]: form.values }, error: form.without[member] });
      (out.prop[member] ??= []).push(...derived);
    }
  }
  // At most one of an exclusive group, and a companion only with its own member.
  for (const [name, entry] of Object.entries(exclusive)) {
    known(`exclusive.${name}`, entry.on);
    for (const member of [...entry.props, ...Object.values(entry.with ?? {}).flat(), ...(entry.orElse ? [entry.orElse] : [])])
      for (const tag of entry.on) if (!schema.elements[tag].props[member]) throw Error(`schema.json: rules.exclusive.${name}: <${tag}> has no prop ${member}`);
    out.exclusive[name] = entry;
    for (const member of entry.props) {
      const others = entry.props.filter(other => other !== member);
      (out.prop[member] ??= []).push({ on: entry.on, without: others, error: entry.error.replace('{prop}', member).replace('{other}', others.join(' or ')) });
      for (const companion of entry.with?.[member] ?? [])
        (out.prop[companion] ??= []).push({ on: entry.on, with: [member], error: entry.companion.replace('{prop}', companion) });
      if (entry.with?.[member]) (out.prop[member] ??= []).push({ on: entry.on, with: entry.with[member], error: entry.needs.replace('{prop}', member) });
    }
  }
  for (const [name, list] of Object.entries(prop)) {
    if (common && !props.has(name)) throw Error(`schema.json: rules.prop names unknown prop ${name}`);
    (out.prop[name] ??= []).push(...list.map(rule => {
      for (const part of [rule, ...(rule.any ?? [])]) {
        known(`prop.${name}`, [...(part.tags ?? []), ...(part.on ?? [])]);
        rolesKnown(`prop.${name}`, part.role);
      }
      if (!rule.error) throw Error(`schema.json: rules.prop.${name} needs an error that names the fix`);
      return rule;
    }));
  }
  for (const [tag, list] of Object.entries(run)) {
    if (schema.elements[tag]?.children !== 'runs') throw Error(`schema.json: rules.run.${tag} is not a run element`);
    if (common) for (const name of list) if (!common.includes(name)) throw Error(`schema.json: rules.run.${tag} names ${name}, which not every element takes`);
    out.run[tag] = list;
  }
  return out;
}

const propName = prop => /^[A-Za-z_$][\w$]*$/.test(prop) ? prop : JSON.stringify(prop);
// TypeScript reads a JSDoc comment only when it starts a line.
const jsdoc = entry => entry?.doc ? `\n  /** ${entry.doc} */ ` : '';
const member = (prop, entry) => `${jsdoc(entry)}${propName(prop)}${entry.required ? '' : '?'}: ${entry.type};`;
// A prop a form refuses keeps its doc, so the hover of a union member still has one.
const refused = (prop, entry) => `${jsdoc(entry)}${propName(prop)}?: never;`;

/** The schema with each element and component prop's `doc` resolved from `propDocs`; every prop and style key needs one. */
export function documented(schema) {
  const missing = [];
  const elements = Object.fromEntries(Object.entries(schema.elements).map(([tag, element]) => {
    const resolve = props => Object.fromEntries(Object.entries(props).map(([prop, entry]) => {
      const doc = entry.doc ?? schema.propDocs[prop];
      if (!doc) missing.push(`<${tag}> ${prop}`);
      return [prop, { ...entry, doc }];
    }));
    return [tag, { ...element, props: resolve(element.props), ...(element.component ? { component: { ...element.component, props: resolve(element.component.props) } } : {}) }];
  }));
  for (const [key, entry] of Object.entries(schema.style)) if (!entry.internal && !entry.doc) missing.push(`style ${key}`);
  if (missing.length) throw Error(`schema.json: needs a doc: ${missing.join(', ')}`);
  return { ...schema, elements };
}

/**
 * Whether a trait's rule can be a type union. A union keyed on a handler cannot survive
 * `Omit<…Props, K>` and a spread (TypeScript tells union members apart only by literal keys), so
 * a wrapper forwarding the props would fail; that rule is left to the build.
 */
export const typedTrait = (element, entry) => !/=>/.test(element.props[entry.key].type);

/** `(without the key: its props never) | (one member per form of the key)`. */
function traitUnion(element, entry) {
  const own = prop => element.props[prop];
  const keyType = own(entry.key).type;
  const props = entry.props.filter(own);
  const absent = `{ ${jsdoc(own(entry.key))}${propName(entry.key)}?: ${/\bboolean\b/.test(keyType) ? 'false | undefined' : 'undefined'}; ${props.map(prop => refused(prop, own(prop))).join(' ')} }`;
  const forms = entry.forms ?? [{ values: null }];
  const present = forms.map(form => `{ ${jsdoc(own(entry.key))}${propName(entry.key)}: ${form.values ? form.values.map(value => JSON.stringify(value)).join(' | ') : keyType}; ${props.map(prop => form.without?.[prop] ? refused(prop, own(prop)) : member(prop, own(prop))).join(' ')} }`);
  // Absent last and the widest form just before it: TypeScript blames the last member a
  // mistake comes closest to, so `open` alone asks for `popover`, not for `popover="manual"`.
  return `(${[...present.reverse(), absent].join(' | ')})`;
}

/** `(one member per prop with its companions, the others never) | (none set)`. */
function exclusiveUnion(element, entry) {
  const companions = prop => entry.with?.[prop] ?? [];
  const never = props => props.map(prop => refused(prop, element.props[prop])).join(' ');
  const all = [...entry.props, ...entry.props.flatMap(companions)];
  const one = prop => `{ ${member(prop, { ...element.props[prop], required: true })} ${companions(prop).map(other => member(other, { ...element.props[other], required: true })).join(' ')} ${never(all.filter(other => other !== prop && !companions(prop).includes(other)))} }`;
  // `orElse`: with none of the group set, that prop is required.
  const none = `{ ${never(all)}${entry.orElse ? ` ${member(entry.orElse, { ...element.props[entry.orElse], required: true })}` : ''} }`;
  return `(${[...entry.props.map(one).reverse(), none].join(' | ')})`;
}

/** The roles `<tag>` may carry: every role whose rules all admit it. */
function rolesOn(schema, table, tag) {
  return schema.options.role.filter(name => (table.elementRole[tag]?.roles.includes(name) ?? true)
    && (table.role[name] ?? []).every(rule => !rule.tags || rule.tags.includes(tag)));
}

// One line per style group, shown on hover over `StyleGroup.<Name>`.
const styleGroupDocs = {
  box: 'Size, margin, position, placement as an item, opacity, background, border, shadow, filters and transforms.',
  padding: 'Padding, physical and flow-relative.',
  direction: 'The main axis of a flex container.',
  align: 'How a container aligns its children.',
  flex: 'Whether a flex container wraps.',
  gap: 'Space between children.',
  grid: 'Grid tracks and auto placement.',
  text: 'Text colour, font, line height, spacing, alignment, decoration, wrapping and selection; inherited.',
  textBlock: 'Line clamping, overflow and clipping a background to the text.',
  scroll: 'Which axes scroll.',
  overscroll: 'Overscroll hand-off and snapping.',
  editor: 'The placeholder colour of an editor.',
  parts: 'The parts of a control.',
  media: 'How an image or video fits its box.',
  imagePlacement: 'Where a fitted image or video sits in its box.',
  accent: 'The accent colour of a control.',
  ellipsis: 'How cut text ends.',
  run: 'Colour, font and decoration of a run inside text.',
};

/** The authored keys of each style group, named as \`elements.*.style\` names them. */
function styleGroupKeys(schema) {
  const groups = new Map();
  for (const name of new Set(Object.values(schema.elements).flatMap(element => [element, element.block].filter(Boolean).flatMap(form => form.style)))) {
    if (!styleGroupDocs[name]) throw Error(`generate.mjs: style group ${name} needs a line in styleGroupDocs`);
    groups.set(name, elementStyle(schema, { style: [name] }).filter(key => !schema.style[key].internal));
  }
  return groups;
}

function authoring(source, api) {
  const schema = documented(source);
  const table = rules(schema), common = commonProps(api);
  const styleMember = (key, indent) => `${indent}/** ${schema.style[key].doc} */\n${indent}${key}?: ${schema.style[key].type};`;
  const style = Object.keys(schema.style).filter(key => !schema.style[key].internal).map(key => styleMember(key, '  ')).join('\n');
  const styleGroups = styleGroupKeys(schema);
  const groupName = name => `StyleGroup.${pascal(name)}`;
  const groupTypes = [...styleGroups].filter(([, keys]) => keys.length).map(([name, keys]) =>
    `  /** ${styleGroupDocs[name]} */\n  export interface ${pascal(name)} {\n${keys.map(key => styleMember(key, '    ')).join('\n')}\n  }`).join('\n');
  const lines = [];
  for (const [name, element] of Object.entries(schema.elements)) {
    const type = pascal(name);
    // An element with a block form (`<link>` outside `<text>`) takes either form's style and children.
    const accepted = [...new Set([element, element.block].filter(Boolean).flatMap(form => form.style))].filter(group => styleGroups.get(group).length);
    lines.push(`export type ${type}Style = ${accepted.length ? accepted.map(groupName).join(' & ') : '{}'};`);
    const children = { any: 'Child', content: 'Child', text: 'TextChild', runs: 'RunChild', none: 'never' }[element.block?.children ?? element.children];
    // A run with no block form takes only its run props, so `<span id>` fails in tsc; a `<link>`
    // may be a block link, which takes them all, so its run props are checked at build.
    const runOnly = table.run[name] && !element.block ? common.filter(prop => !table.run[name].includes(prop) && prop !== 'style' && prop !== 'children') : [];
    // A role the rules keep off this element fails in tsc, not at render.
    const allowed = rolesOn(schema, table, name), narrowed = allowed.length < schema.options.role.length && !runOnly.includes('role');
    if (narrowed) lines.push(`/** The roles a \`<${name}>\` takes; reference/elements.md says where the others go. */`,
      `export type ${type}Role = ${allowed.length ? allowed.map(entry => JSON.stringify(entry)).join(' | ') : 'never'};`);
    const role = narrowed ? `role?: ${type}Role; ` : '';
    const omitted = ['style', 'children', ...(narrowed ? ['role'] : []), ...runOnly];
    // A trait's key and props move out of the flat props into a union: without the key they are `never`.
    const traits = Object.values(table.trait).filter(entry => entry.on.includes(name) && typedTrait(element, entry));
    const groups = Object.values(table.exclusive).filter(entry => entry.on.includes(name));
    const traitProps = new Set([...traits.flatMap(entry => [entry.key, ...entry.props]), ...groups.flatMap(entry => [...entry.props, ...Object.values(entry.with ?? {}).flat()])]);
    const flat = Object.entries(element.props).filter(([prop]) => !traitProps.has(prop)).map(([prop, entry]) => member(prop, entry));
    const unions = [...traits.map(entry => traitUnion(element, entry)), ...groups.map(entry => exclusiveUnion(element, entry))].join(' & ');
    const base = `Omit<Props, ${omitted.map(prop => JSON.stringify(prop)).join(' | ')}> & { ${role}style?: ${type}Style; children?: ${children};${flat.join('')}\n}${unions ? ` & ${unions}` : ''}`;
    const binding = element.binding;
    if (binding) {
      const payload = binding.type === 'string' ? 'string' : `ReadonlyData<${binding.type}>`;
      // `end` hears a change once it is finished; `pair` is the two-value form of the control.
      const { end, pair } = binding;
      const ended = end ? `; ${end}?: (value: ${payload}) => void` : '';
      const single = pair ? `; ${pair.values}?: never; ${pair.labels}?: never` : '';
      const both = `readonly [${binding.type}, ${binding.type}]`;
      lines.push(`export type ${type}Props = ${base} & (`
        + `{ bind: Local<${binding.type}>; value?: never; ${binding.event}?: never${ended}${single} } | `
        + `{ bind?: never; value?: ${binding.type}; ${binding.event}?: (value: ${payload}) => void${ended}${single} }`
        + (pair ? ` | { bind?: never; value?: never; ${pair.values}: ${both}; ${pair.labels}: readonly [string, string]; `
          + `${binding.event}?: (values: ${both}) => void${end ? `; ${end}?: (values: ${both}) => void` : ''} }` : '') + ');');
    } else lines.push(`export type ${type}Props = ${base};`);
    // A component's props over the element's, generic in its item type.
    if (element.component) lines.push(`export interface Data${element.component.name}Props<T> extends Omit<${type}Props, ${element.component.replaces.map(prop => JSON.stringify(prop)).join(' | ')}> {`,
      ...Object.entries(element.component.props).map(([prop, entry]) => member(prop, entry).slice(1)), '}');
  }
  // An element behind a component (`<List>`) is not written as JSX.
  const intrinsics = Object.entries(schema.elements).filter(([, element]) => !element.component).map(([name]) => `      ${JSON.stringify(name)}: ${pascal(name)}Props;`).join('\n');
  return `${header}${api.replace(/^\/\/.*\n/gm, '')}
/** The page's environment (\`view.env\`): read-only, and a change re-renders the page. */
export interface Env {
${Object.entries(schema.environment).filter(([, entry]) => !entry.internal).map(([key, entry]) => `${entry.doc ? `  /** ${entry.doc} */\n` : ''}  readonly ${key}${entry.absent ? '?' : ''}: ${entry.type};`).join('\n')}
}

/** A theme colour role; it resolves per appearance, so \`dark:\` is never needed on one. */
export type ColorRole = ${schema.roles.map(role => JSON.stringify(role.name)).join(' | ')};
/** A role at an alpha percent, 0 to 100 (\`'accent/12'\`). */
export type ColorAlpha = \`\${ColorRole}/\${number}\`;
/** A page colour: a role, a role at an alpha percent, or \`'transparent'\`. */
export type ColorValue = ColorRole | ColorAlpha | "transparent";
/** A \`<plot>\` series colour: a role, usually \`'chart-1'\` to \`'chart-8'\`, or one at an alpha percent. */
export type ColorSeries = ColorRole | ColorAlpha;

/** CSS-named style; each element accepts the subset its \`*Style\` type lists. */
export interface Style {
${style}
}

/** Style keys by group; each element's \`*Style\` joins the groups it accepts. */
export namespace StyleGroup {
${groupTypes}
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
      for (const member of node.members) if (ts.isPropertySignature(member)) names.push(ts.isStringLiteral(member.name) ? member.name.text : member.name.getText(file));
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
    ...(element.component ? { component: element.component.name } : {}),
  }]));
  // A block form is an element kind of its own, never written as JSX: `<link>` outside `<text>`.
  const blocks = Object.fromEntries(Object.entries(schema.elements).filter(([, element]) => element.block).map(([name, element]) => [element.block.wire, {
    wire: element.block.wire, children: element.block.children, style: elementStyle(schema, element.block), props: elements[name].props, of: name,
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
export const blocks = ${JSON.stringify(blocks)};
/** An element or block kind by name. */
export const kindOf = name => Object.hasOwn(elements, name) ? elements[name] : blocks[name];
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
/** Props every element accepts, read from \`interface Props\` in contracts/view/api.d.ts. */
export const commonProps = ${JSON.stringify(common)};
/** Authoring admission the wire does not carry: where a role goes, and what an element's role allows. */
export const authoringRules = ${JSON.stringify(rules(schema, common))};
/** Colour roles in palette-slot order. */
export const roles = ${JSON.stringify(schema.roles.map(role => role.name))};
/** Scale slots in wire order: a \`[slot, times, plus]\` reference names one by index. */
export const scales = ${JSON.stringify(schema.scales.map(({ name, family }) => ({ name, family })))};
/** Each slot's value in the built-in pack: where a step cannot follow the pack, it takes this. */
export const scaleDefaults = ${JSON.stringify(scaleDefaults(schema))};
/** Scale family to the wire fields whose fields take its references. */
export const scaledFields = ${JSON.stringify(schema.scaledFields)};
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
  return `${header}package com.lingxia.view.generated;

/** Semantic node roles, live-region kinds, event kinds and JSON keys of the shared projection. */
public final class GpuSemantics {
    private GpuSemantics() {}

${lines.join('\n')}
}
`;
}

// Raw-value enums, so an adapter can switch over a role exhaustively.
function swiftSemantics(schema) {
  const { roles, liveRegions, events, actions, keys } = schema.semantics;
  const keywords = new Set(['switch', 'default', 'case', 'class', 'enum', 'in', 'is', 'as', 'func', 'let', 'var', 'return', 'import', 'protocol', 'struct', 'self', 'Self', 'true', 'false', 'nil', 'where', 'while', 'for', 'if', 'else', 'do', 'repeat', 'break', 'continue', 'guard', 'throw', 'throws', 'try', 'operator', 'static', 'public', 'private', 'internal', 'extension', 'subscript', 'init', 'deinit', 'any', 'Any', 'Type']);
  const camel = name => name.replace(/[-_]([a-z])/g, (_, letter) => letter.toUpperCase());
  const member = (name, value) => {
    const id = keywords.has(camel(name)) ? `\`${camel(name)}\`` : camel(name);
    return camel(name) === value ? `        case ${id}` : `        case ${id} = ${JSON.stringify(value)}`;
  };
  const block = (name, doc, entries) => `    /// ${doc}\n    public enum ${name}: String, Sendable, CaseIterable {\n${entries.map(([key, value]) => member(key, value)).join('\n')}\n    }`;
  return `${header}/// Semantic node roles, live-region kinds, event kinds and JSON keys of the shared projection.
public enum GpuSemantics {
${[
    block('Role', 'A semantic node\'s role.', roles.map(role => [role, role])),
    block('LiveRegion', 'How a live region announces a change.', liveRegions.map(kind => [kind, kind])),
    block('Event', 'An event the projection raises.', events.map(kind => [kind, kind])),
    block('Action', 'An action a node offers besides activation.', actions.map(kind => [kind, kind])),
    block('Key', 'A JSON key of a semantic node.', Object.entries(keys)),
  ].join('\n\n')}
}
`;
}

// Editor attributes the Android input connection maps onto EditorInfo and autofill.
function androidInput(schema) {
  const { autofill, keys } = schema.input;
  const options = kind => schema.options[kind].map(value => `    public static final String ${kind === 'enter-key-hint' ? 'ENTER' : kind === 'auto-capitalize' ? 'CAPITALIZE' : 'KEYBOARD'}_${constant(value)} = ${JSON.stringify(value)};`);
  const hints = Object.entries(autofill).map(([token, hint]) => `            case ${JSON.stringify(token)}: return ${JSON.stringify(hint)};`);
  return `${header}package com.lingxia.view.generated;

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
  const { types, messages, doc, fieldDocs = {} } = schema.frame;
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
  const requests = new Set(schema.frame.requests ?? []);
  // Builders take what the reader returns, except nested request objects, which their own builders make.
  const paramType = ty => {
    const t = frameType(ty);
    const element = kindOf(t.base) === 'prim' ? prim[t.base] : kindOf(t.base) === 'enum' ? 'String' : 'JSONObject';
    if (t.dims.length) return element + '[]'.repeat(t.dims.length);
    return t.optional && boxed[element] ? boxed[element] : element;
  };
  const builders = (name, def) => Object.entries(def.variants).map(([variant, fields]) => {
    const params = Object.entries(fields).map(([key, ty]) => `${paramType(ty)} ${camel(key)}`);
    const pairs = Object.keys(fields).map(key => `, ${JSON.stringify(key)}, ${camel(key)}`).join('');
    return `        /** A \`${variant}\` ${name}. */\n        public static JSONObject ${camel(variant)}(${params.join(', ')}) { return GpuFrame.message(${JSON.stringify(def.tagged)}, ${JSON.stringify(variant)}${pairs}); }`;
  });
  const classes = [], constants = [];
  for (const [name, def] of Object.entries(types)) {
    if (def.enum) { constants.push(...def.enum.map(value => `    public static final String ${constant(name)}_${constant(value)} = ${JSON.stringify(value)};`)); continue; }
    if (def.tagged) constants.push(...Object.keys(def.variants).map(value => `    public static final String ${constant(name)}_${constant(value)} = ${JSON.stringify(value)};`));
    const methods = Object.entries(fieldsOf(def)).map(([key, ty]) => {
      const t = frameType(ty);
      const note = fieldDocs[`${name}.${key}`];
      return `        /** \`${key}\`: ${ty}${note ? `. ${note}` : ''} */\n        public ${javaType(t)} ${javaMethod(camel(key))}() { ${reader(key, t)} }`;
    });
    if (requests.has(name)) methods.push(...builders(name, def));
    classes.push(`    public static final class ${name} {
        public final JSONObject json;
        public ${name}(JSONObject json) { this.json = json; }
${methods.join('\n')}
    }`);
  }
  return `${header}package com.lingxia.view.generated;

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

    /** A request object: the tag, then key/value pairs; arrays become JSON lists, null stays null. */
    static JSONObject message(String tag, String variant, Object... pairs) {
        JSONObject out = new JSONObject();
        try {
            out.put(tag, variant);
            for (int i = 0; i < pairs.length; i += 2) out.put((String) pairs[i], json(pairs[i + 1]));
        } catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
        return out;
    }

    private static Object json(Object value) {
        if (value == null) return JSONObject.NULL;
        if (value instanceof float[]) { JSONArray out = new JSONArray(); for (float v : (float[]) value) { try { out.put((double) v); } catch (org.json.JSONException e) { throw new IllegalArgumentException(e); } } return out; }
        if (value instanceof double[]) { JSONArray out = new JSONArray(); for (double v : (double[]) value) { try { out.put(v); } catch (org.json.JSONException e) { throw new IllegalArgumentException(e); } } return out; }
        if (value instanceof long[]) { JSONArray out = new JSONArray(); for (long v : (long[]) value) out.put(v); return out; }
        if (value instanceof boolean[]) { JSONArray out = new JSONArray(); for (boolean v : (boolean[]) value) out.put(v); return out; }
        if (value instanceof Object[]) { JSONArray out = new JSONArray(); for (Object v : (Object[]) value) out.put(json(v)); return out; }
        return value;
    }

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
  const { types, messages, fieldDocs = {} } = schema.frame;
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
    const fields = Object.entries(fieldsOf(def));
    const props = fields.map(([key, ty]) => `${fieldDocs[`${name}.${key}`] ? `    /// ${fieldDocs[`${name}.${key}`]}\n` : ''}    public let ${camel(key)}: ${swiftType(ty)}`);
    const keys = fields.map(([key]) => `        case ${camel(key)} = ${JSON.stringify(key)}`);
    return `/// \`${name}\` in the frame contract.
public struct GpuFrame${name}: Decodable, Sendable {
${props.join('\n')}

    enum CodingKeys: String, CodingKey {
${keys.join('\n')}
    }
}`;
  });
  const requests = new Set(schema.frame.requests ?? []);
  const messageType = ty => {
    const t = frameType(ty);
    let element = prim[t.base] ?? (types[t.base]?.enum ? 'String' : requests.has(t.base) ? `GpuFrame${t.base}Message` : `GpuFrame${t.base}`);
    for (const _ of t.dims) element = `[${element}]`;
    return t.optional ? `${element}?` : element;
  };
  const messages_ = [...requests].map(name => {
    const def = types[name];
    const cases = Object.entries(def.variants).map(([variant, fields]) => {
      const params = Object.entries(fields).map(([key, ty]) => `${camel(key)}: ${messageType(ty)}`);
      return `    case ${camel(variant)}${params.length ? `(${params.join(', ')})` : ''}`;
    });
    const encodes = Object.entries(def.variants).map(([variant, fields]) => {
      const keys = Object.keys(fields);
      const bind = keys.length ? `(${keys.map(key => `let ${camel(key)}`).join(', ')})` : '';
      const lines = keys.map(key => `            try container.encode(${camel(key)}, forKey: GpuFrameKey(${JSON.stringify(key)}))`);
      return `        case .${camel(variant)}${bind}:\n            try container.encode(${JSON.stringify(variant)}, forKey: GpuFrameKey(${JSON.stringify(def.tagged)}))${lines.length ? '\n' + lines.join('\n') : ''}`;
    });
    return `/// A \`${name}\` the platform sends to the core.
public enum GpuFrame${name}Message: Encodable, Sendable {
${cases.join('\n')}

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: GpuFrameKey.self)
        switch self {
${encodes.join('\n')}
        }
    }
}`;
  });
  const enums = Object.entries(types).filter(([, def]) => def.enum).map(([name, def]) =>
    `/// \`${name}\` values.\npublic enum GpuFrame${name}: String, Sendable {\n${def.enum.map(value => `    case ${camel(value)} = ${JSON.stringify(value)}`).join('\n')}\n}`);
  return `// Generated from contracts/view/schema.json; do not edit.
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

/// The core's editor limits; an input mirror holds itself to them, since a rejected edit fails the session.
public enum GpuFrameInputLimits {
${['editorTextBytes', 'inputRequestBytes', 'inputBatchEdits', 'clipboardItems'].map(name => `    public static let ${name} = ${schema.limits[name]}`).join('\n')}
}

/// A JSON key named at run time, for request encoders.
public struct GpuFrameKey: CodingKey {
    public let stringValue: String
    public var intValue: Int? { nil }
    public init(_ string: String) { stringValue = string }
    public init?(stringValue: String) { self.stringValue = stringValue }
    public init?(intValue: Int) { nil }
}

${messages_.join('\n\n')}
`;
}

/** The default pack as lingxia-theme resolves it, accent family included; a Rust test records palette.json. */
async function palette(schema) {
  const resolved = JSON.parse(await readFile(join(contractsRoot, 'palette.json'), 'utf8'));
  const roles = schema.roles.map(role => role.name);
  for (const appearance of ['light', 'dark']) {
    const recorded = Object.keys(resolved[appearance] ?? {});
    if (recorded.join() !== roles.join())
      throw Error(`contracts/view/palette.json: ${appearance} roles differ from schema.json; record with LINGXIA_UPDATE_PALETTE=1 cargo test -p lingxia-theme`);
  }
  const head = '// Generated from contracts/view/palette.json; do not edit.\n';
  const js = `${head}/** The default theme pack resolved per appearance, derived roles included: role → #rrggbbaa. */
export const defaultPalette = Object.freeze({
${['light', 'dark'].map(appearance => `  ${appearance}: Object.freeze(${JSON.stringify(resolved[appearance])}),`).join('\n')}
});
`;
  const dts = `${head}export type Role = ${roles.map(role => JSON.stringify(role)).join(' | ')};
/** The default theme pack resolved per appearance, derived roles included: role → #rrggbbaa. */
export declare const defaultPalette: { readonly light: Readonly<Record<Role, string>>; readonly dark: Readonly<Record<Role, string>> };
`;
  return { 'packages/lingxia-view/src/generated/palette.js': js, 'packages/lingxia-view/src/generated/palette.d.ts': dts };
}

/** Map of repository-relative path to generated content. */
export async function generate(schema) {
  validate(schema);
  validateAuthor(schema);
  const api = await readFile(join(contractsRoot, 'api.d.ts'), 'utf8');
  const generated = {
    'crates/lingxia-view/src/protocol/generated.rs': rust(schema),
    'crates/lingxia-theme/src/generated.rs': themeRust(schema),
    'crates/lingxia-lxapp/src/page/view_limits.rs': lxappLimits(schema),
    'packages/lingxia-view/src/generated/wire.js': wire(schema),
    'packages/lingxia-view/src/generated/contract.js': contract(schema, commonProps(api)),
    'packages/lingxia-view/src/generated/authoring.d.ts': authoring(schema, api),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/view/generated/GpuSemantics.java': androidSemantics(schema),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/view/generated/GpuInput.java': androidInput(schema),
    'lingxia-sdk/android/lingxia/src/main/java/com/lingxia/view/generated/GpuFrame.java': frameJava(schema),
    'lingxia-sdk/apple/Sources/Platform/View/GpuFrame.swift': frameSwift(schema),
    'lingxia-sdk/apple/Sources/Platform/View/GpuSemantics.swift': swiftSemantics(schema),
  };
  Object.assign(generated, (await import('./arkts.mjs')).arkts(schema));
  Object.assign(generated, await palette(schema));
  generated['crates/lingxia-view/src/session/tests/runtime.js'] = await nativeTestRuntime(generated);
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
