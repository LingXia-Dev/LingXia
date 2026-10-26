// The View language reference (docs/view/reference), derived from schema.json, the
// authoring API types, the real Tailwind lowering and the sources that raise UI_* codes.
// `--check` fails on drift instead of writing.
import { readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { build } from 'esbuild';
import { readSchema, elementStyle, generate, packageRoot, repositoryRoot } from './generate.mjs';

const header = '_Generated from the View schema (`schema.json`) and the compiler; do not edit._\n';
// A code span that may itself hold backticks (template literal types).
const code = text => String(text).includes('`') ? `\`\` ${text} \`\`` : `\`${text}\``;
const cell = text => String(text).replace(/\|/g, '\\|').replace(/\n/g, ' ');
const table = (head, rows) => [`| ${head.join(' | ')} |`, `| ${head.map(() => '---').join(' | ')} |`,
  ...rows.map(row => `| ${row.map(cell).join(' | ')} |`)].join('\n');
const list = items => items.map(code).join(', ');

// ---- authoring API types --------------------------------------------------------------

async function apiMembers(name) {
  const path = join(packageRoot, 'contracts/api.d.ts');
  const file = ts.createSourceFile(path, await readFile(path, 'utf8'), ts.ScriptTarget.ES2020, true);
  const members = [];
  const visit = node => {
    if (ts.isInterfaceDeclaration(node) && node.name.text === name) {
      // A doc comment may cover several members declared on one line; it describes the first.
      for (const member of node.members) {
        if (!ts.isPropertySignature(member)) continue;
        const docs = ts.getJSDocCommentsAndTags(member).map(doc => ts.getTextOfJSDocComment(doc.comment) ?? '').join(' ').trim();
        members.push({ name: member.name.getText(file), type: member.type.getText(file).replace(/\s+/g, ' '),
          optional: Boolean(member.questionToken), docs });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  if (!members.length) throw Error(`api.d.ts: interface ${name} not found`);
  return members;
}

// ---- elements -------------------------------------------------------------------------

const childKinds = {
  any: 'any elements and text',
  content: 'text, or noninteractive content elements (a composed control)',
  text: 'text only',
  runs: 'text, `<span>` and `<link>` runs',
  none: 'none',
};

async function elements(schema) {
  const common = (await apiMembers('Props')).filter(member => !['style', 'children'].includes(member.name));
  const out = [header, '# Elements', '',
    'Every intrinsic element of the View language: its children, props and binding.',
    'Styles each element accepts are in [styles.md](styles.md).', '',
    '## Props every element takes', '',
    table(['Prop', 'Type', 'Notes'], common.map(member => [code(member.name), code(member.type), member.docs])), '',
    '`style` takes the element\'s own style subset; `className` takes complete Tailwind utilities.', ''];
  const parents = Object.fromEntries(Object.entries(schema.admission).filter(([, entry]) => entry.parent).map(([tag, entry]) => [tag, entry.parent]));
  for (const [name, element] of Object.entries(schema.elements)) {
    const admission = schema.admission[element.wire];
    out.push(`## \`<${name}>\``, '');
    // A run may hold runs, but a link never holds another link.
    const children = element.children === 'runs' && element.wire === 'link' ? 'text and `<span>` runs; links do not nest' : childKinds[element.children];
    const facts = [`Children: ${children}.`];
    if (element.wire !== name) facts.push(`Renders as ${code(element.wire)}.`);
    if (parents[element.wire]) facts.push(`Only inside ${code(`<${parents[element.wire]}>`)}.`);
    if (admission.root) facts.push('The page root.');
    facts.push(admission.content ? 'May sit inside a composed `<button>` or `<checkbox>`.' : 'Not allowed inside a composed `<button>` or `<checkbox>`.');
    out.push(facts.join(' '), '');
    if (element.binding) {
      const { type, event } = element.binding;
      out.push(`Binding: \`bind={local}\` with a \`useLocal<${type}>\` cell, or \`value\` plus \`${event}(value: ${type})\`; not both.`, '');
    }
    const props = Object.entries(element.props);
    if (props.length) out.push(table(['Prop', 'Type', 'Required'], props.map(([prop, entry]) => [code(prop), code(entry.type), entry.required ? 'yes' : ''])), '');
    else out.push('No element-specific props.', '');
  }
  return out.join('\n');
}

// ---- styles ---------------------------------------------------------------------------

function styles(schema) {
  const accepted = Object.fromEntries(Object.entries(schema.elements).map(([name, element]) => [name, elementStyle(schema, element)]));
  const names = Object.keys(schema.elements);
  const out = [header, '# Style properties', '',
    'CSS-named keys of the `style` prop and the targets of every utility. A key is accepted only by the',
    'elements listed; anything else fails the build with `UI_STYLE` (inline) or `UI_CLASS_ELEMENT` (utility).',
    'Numbers are logical pixels. **Paint** keys are the only ones `focus:`, `active:` and `hover:` may change.', '',
    table(['Key', 'Type', 'CSS', 'Paint', 'Accepted by'], Object.entries(schema.style).filter(([, entry]) => !entry.internal).map(([key, entry]) => {
      const by = names.filter(name => accepted[name].includes(key));
      return [code(key), code(entry.type), [entry.css ?? []].flat().map(code).join(', '), entry.paint ? 'yes' : '',
        by.length === names.length ? 'every element' : by.length > names.length / 2
          ? `all but ${names.filter(name => !by.includes(name)).map(name => code(`<${name}>`)).join(' ')}` : by.map(name => code(`<${name}>`)).join(' ')];
    })), '',
    '`transition` and `animation` exist on the wire but are set only by the gated motion utilities.', '',
    '## By element', ''];
  for (const name of names) out.push(`### \`<${name}>\``, '', accepted[name].filter(key => !schema.style[key].internal).map(code).join(', '), '');
  return out.join('\n');
}

// ---- utilities and variants -------------------------------------------------------------

const contractPath = 'packages/lingxia-ui/src/generated/contract.js';

// The Tailwind lowering bound to this generation's contract, not the committed one, so one
// `generate` pass converges. Packages stay external and load from their resolved files.
async function lowering(generated) {
  const contract = generated[contractPath];
  if (!contract) throw Error(`docs.mjs: the generation lacks ${contractPath}`);
  // This module sits in the compiler's package, so its ESM resolution matches the compiler's imports.
  const theme = import.meta.resolve('tailwindcss/theme.css');
  const plugin = { name: 'fresh-contract', setup(builder) {
    builder.onResolve({ filter: /^[^./]/ }, ({ path }) => ({ path: path.startsWith('node:') ? path : import.meta.resolve(path), external: true }));
    builder.onResolve({ filter: /generated\/contract\.js$/ }, ({ path, resolveDir }) => ({ path: resolve(resolveDir, path) }));
    builder.onLoad({ filter: /generated[\\/]contract\.js$/ }, () => ({ contents: contract, loader: 'js' }));
  } };
  const result = await build({ entryPoints: [join(packageRoot, 'compiler/tailwind.mjs')], bundle: true, write: false, format: 'esm',
    platform: 'node', legalComments: 'none', plugins: [plugin], define: { 'import.meta.resolve': '__lingxiaResolve' },
    banner: { js: `const __lingxiaResolve = specifier => { if (specifier !== 'tailwindcss/theme.css') throw Error('docs.mjs: unexpected resolve ' + specifier); return ${JSON.stringify(theme)}; };` } });
  const source = result.outputFiles[0].text;
  return import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
}

async function designSystem() {
  const { __unstable__loadDesignSystem: load } = await import('tailwindcss');
  const theme = await readFile(new URL(import.meta.resolve('tailwindcss/theme.css')), 'utf8');
  return load(`${theme}\n@tailwind utilities;`);
}

const strip = name => name.replace(/^-/, '');

function family(name, roots) {
  const bare = strip(name);
  let best;
  for (const root of roots) if ((bare === root || bare.startsWith(`${root}-`)) && (!best || root.length > best.length)) best = root;
  return best ?? bare.split('-')[0];
}

const gated = message => message.startsWith('UI_ANIMATION');
// Messages name concrete values; collapse them so one reason summarizes a family.
const reason = message => gated(message) ? 'gated motion' : message
  .replace(/^UI_[A-Z_]+: /, '')
  .replace(/^([a-z-]+): .*$/, '$1')
  .replace(/:.*$/, '')
  .replace(/ \(.*\)$/, '')
  .trim();

// The style keys a rule sets; empty when the utility lowers to nothing the renderer draws.
const effects = rule => [...Object.keys(rule.style ?? {}), ...(rule.gradient ? ['backgroundImage'] : []), ...(rule.shadow ? ['boxShadow'] : []),
  ...(rule.backdrop ? ['backdropFilter'] : []), ...(rule.transform ? ['translate'] : []), ...(rule.numeric ? ['fontVariantNumeric'] : [])];

const sections = [
  ['Layout', /^(display|overflow|order)$/],
  ['Position', /^(position|top|right|bottom|left|inset.*|zIndex)$/],
  ['Flex and grid items', /^(flex(Grow|Shrink|Basis)|alignSelf|justifySelf|grid(Column|Row).*)$/],
  ['Flex and grid containers', /^(flexDirection|flexWrap|alignItems|justifyContent|alignContent|justifyItems|gap|rowGap|columnGap|grid(Template|Auto).*)$/],
  ['Sizing', /^(width|height|minWidth|minHeight|maxWidth|maxHeight|aspectRatio)$/],
  ['Margin', /^margin/], ['Padding', /^padding/],
  ['Typography', /^(color|font.*|lineHeight|letterSpacing|text.*|whiteSpace|wordBreak|overflowWrap|lineClamp|userSelect)$/],
  ['Backgrounds', /^background/], ['Radius', /Radius$/], ['Borders', /^border/],
  ['Effects', /^(opacity|boxShadow|backdropFilter)$/], ['Transforms', /^(translate|rotate|scale|transformOrigin)$/],
  ['Scrolling', /^(overflowX|overflowY|overscrollBehavior)$/], ['Editors', /^placeholderColor$/], ['Images and vectors', /^objectFit$/],
];
// The last key decides: `truncate` ends in whiteSpace, `space-x-*` in a margin.
const sectionOf = rule => sections.find(([, test]) => test.test(effects(rule).at(-1)))?.[0] ?? 'Other';

function utilities(system, rules) {
  const names = system.getClassList().map(([name]) => name);
  const roots = [...system.utilities.keys('functional')].map(strip);
  const ok = new Map(), errors = new Map();
  for (const rule of rules) {
    if (rule.error) { if (!errors.has(rule.name)) errors.set(rule.name, rule.error.message); }
    else if (!ok.has(rule.name)) ok.set(rule.name, rule);
  }
  const families = new Map();
  for (const name of names) {
    const key = family(name, roots);
    if (!families.has(key)) families.set(key, { supported: [], inert: [], rejected: new Map(), section: undefined });
    const entry = families.get(key);
    const rule = ok.get(name);
    if (rule && effects(rule).length) { entry.supported.push(name); entry.section ??= sectionOf(rule); }
    else if (rule) entry.inert.push(name);
    else {
      const why = reason(errors.get(name) ?? 'not generated');
      entry.rejected.set(why, (entry.rejected.get(why) ?? 0) + 1);
    }
  }
  const sample = items => items.length <= 6 ? list(items) : `${list(items.slice(0, 5))}, … (${items.length})`;
  const total = map => [...map.values()].reduce((a, b) => a + b, 0);
  const reasons = map => [...map].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, 3)
    .map(([why, count]) => `${why} (${count})`).join('; ');
  const grouped = {};
  for (const [key, entry] of [...families].sort(([a], [b]) => strip(a).localeCompare(strip(b)))) {
    const where = entry.supported.length ? entry.section
      : entry.rejected.size === 1 && entry.rejected.has('gated motion') ? 'Gated' : entry.rejected.size ? 'Refused' : 'Inert';
    (grouped[where] ??= []).push([key, entry]);
  }
  const out = [header, '# Utilities', '',
    'Every utility of Tailwind\'s default theme, run through the native lowering and grouped by family.',
    'Your `@theme` tokens add names to the same families (`bg-primary`, `rounded-card`); arbitrary values',
    '(`w-[72px]`, `grid-cols-[1fr_auto]`) follow the rules of the family they extend. A supported utility',
    'is still refused on an element that does not render its style ([styles.md](styles.md)).', ''];
  for (const [title] of [...sections, ['Other']]) {
    const rows = grouped[title];
    if (rows?.length) out.push(`## ${title}`, '', table(['Family', 'Supported', 'Refused'],
      rows.map(([key, entry]) => [code(key), sample(entry.supported), reasons(entry.rejected)])), '');
  }
  if (grouped.Gated?.length)
    out.push('## Gated: motion', '', 'Refused unless the page opts into the gated motion lowering (see the motion chapter):', '',
      grouped.Gated.map(([key, entry]) => `${code(key)} (${total(entry.rejected)})`).join(', '), '');
  const inert = [...families].filter(([, entry]) => entry.inert.length)
    .map(([key, entry]) => entry.inert.length === 1 ? code(entry.inert[0]) : `${code(key)} (${entry.inert.length})`);
  if (inert.length) out.push('## Accepted without effect', '', 'These compile but set nothing the renderer draws:', '', inert.join(', '), '');
  if (grouped.Refused?.length)
    out.push('## Not supported', '', table(['Family', 'Utilities', 'Reason'], grouped.Refused.map(([key, entry]) =>
      [code(key), total(entry.rejected), reasons(entry.rejected)])), '');
  return out.join('\n');
}

// Functional variants need a value to form a candidate; these are representative.
const variantSamples = {
  not: 'not-hover', group: 'group-hover', peer: 'peer-hover', in: 'in-focus', has: 'has-checked', aria: 'aria-checked',
  data: 'data-[open]', nth: 'nth-3', 'nth-last': 'nth-last-3', 'nth-of-type': 'nth-of-type-3', 'nth-last-of-type': 'nth-last-of-type-3',
  supports: 'supports-[display:grid]', max: 'max-md', min: 'min-md', '@max': '@max-md', '@': '@md', '@min': '@min-md',
};

const probe = 'opacity-50';
const variantCandidates = system => system.getVariants().map(variant => [variant.name, `${variantSamples[variant.name] ?? variant.name}:${probe}`]);

function variants(schema, system, rules) {
  const candidates = variantCandidates(system);
  const result = name => {
    const matched = rules.filter(rule => rule.name === name);
    const error = matched.find(rule => rule.error)?.error.message;
    return matched.length && !error ? null : error ?? 'not generated';
  };
  const wireTags = field => schema.fields.find(entry => entry.key === field).tags;
  const on = field => Object.entries(schema.elements).filter(([, element]) => wireTags(field).includes(element.wire)).map(([name]) => code(`<${name}>`)).join(' ');
  const disableable = Object.entries(schema.elements).filter(([, element]) => element.props.disabled).map(([name]) => code(`<${name}>`)).join(' ');
  const admission = {
    focus: `${on('focusStyle')}; paint only`, active: `${on('pressedStyle')}; paint only`,
    hover: 'any element, only when `env.pointer` is `fine`; paint only', disabled: `${disableable}; replaces the whole class cascade`,
    placeholder: `editors; \`text-*\` color only`,
  };
  const supported = [], refused = [];
  for (const [name, candidate] of candidates) {
    const sample = candidate.slice(0, -probe.length - 1);
    const error = name === 'placeholder' ? result('placeholder:text-red-500') : result(candidate);
    if (error) refused.push([code(`${name}:`), code(`${sample}:`), error === 'not generated'
      ? 'matches no rule; the build reports it as `UI_CLASS_UNKNOWN`' : error.replace(/^UI_[A-Z_]+: /, '')]);
    else supported.push([code(`${name}:`), admission[name] ?? 'any element']);
  }
  return [header, '# Variants', '',
    'Every Tailwind variant, tested against the native lowering. Supported variants may be stacked',
    '(`dark:active:bg-muted`). Conditions on DOM state the View does not have are refused at build time.', '',
    '## Supported', '', table(['Variant', 'Applies to'], supported), '',
    '`space-x-*`, `space-y-*` and `divide-*` are utilities, not variants: they style every child but the last.', '',
    '## Refused', '', table(['Variant', 'Probed as', 'Reason'], refused), ''].join('\n');
}

// ---- limits, environment, semantics -----------------------------------------------------

const limitNotes = {
  nodes: 'Nodes in one rendered frame, list rows included.',
  depth: 'Nesting depth of the rendered tree.',
  animationBytes: 'Animation definitions per page, as UTF-8 JSON (gated motion).',
  itemKeyBytes: 'All `itemKeys` of a page, as UTF-8 JSON.',
  imageBytes: 'Encoded size of one image resource.',
  sourceBytes: 'Length of one image `src`.',
  visibleImageBytes: 'Decoded image bytes one page paints at once; past it an image gets `onError` and a placeholder.',
  decodedImageBytes: 'Decoded-image cache shared by every page; it evicts, never fails.',
  runs: 'Runs in one `<text>`: each `<span>`, `<link>` and the text between them.',
  transitionMs: 'Longest transition duration or delay, in milliseconds (gated motion).',
};

const size = value => value >= 1 << 20 && value % (1 << 20) === 0 ? `${value / (1 << 20)} MiB`
  : value >= 1 << 10 && value % (1 << 10) === 0 ? `${value / (1 << 10)} KiB` : null;

function limits(schema) {
  for (const key of Object.keys(limitNotes)) if (!(key in schema.limits)) throw Error(`docs.mjs: limit note for unknown limit ${key}`);
  return [header, '# Limits', '',
    'Page-wide bounds the runtime and the native decoder both enforce. Exceeding one fails the frame',
    '(`UI_LIMIT`, `UI_LIST`, `UI_TEXT`, `UI_ANIMATION`), except the image budgets noted below.', '',
    table(['Limit', 'Value', 'Meaning'], Object.entries(schema.limits).map(([key, value]) => {
      const bytes = /Bytes$/.test(key) ? size(value) : null;
      return [code(key), bytes ? `${value} (${bytes})` : value, limitNotes[key] ?? ''];
    })), '', `Wire ABI: ${schema.uiAbi}. Highest field id: ${schema.fields.length - 1}.`, ''].join('\n');
}

async function environment() {
  const members = await apiMembers('Env');
  return [header, '# Environment', '',
    '`view.env` (from `render(view)` or `useView()`): read-only, and a change re-renders the page.', '',
    table(['Field', 'Type', 'Notes'], members.map(member => [code(member.name + (member.optional ? '?' : '')), code(member.type), member.docs])), ''].join('\n');
}

function semantics(schema) {
  const { roles, liveRegions, actions } = schema.semantics;
  const authored = schema.options.role;
  const projected = role => roles.includes(role) ? role : roles.find(name => name.replace(/-/g, '') === role);
  return [header, '# Semantics', '',
    'The accessibility tree the platform adapter exposes. Elements get their role from what they are;',
    'the `role` prop sets one of the authored roles on any non-control element.', '',
    '## Authored roles', '', table(['`role`', 'Exposed as'], authored.map(role => [code(role), code(projected(role))])), '',
    '`headingLevel` (1..6) applies only to `role="heading"`.', '',
    '## All exposed roles', '', list(roles), '',
    '## Live regions', '', `\`accessibilityLiveRegion\`: ${list(schema.options['live-region'])}. Announced politeness: ${list(liveRegions)}.`, '',
    '## Text actions', '', `Selectable text supports ${list(actions)}.`, ''].join('\n');
}

// ---- error codes ------------------------------------------------------------------------

const errorNotes = {
  UI_ACTION: 'An action call was malformed or its reply invalid.',
  UI_ANIMATION: 'Motion is gated, or an animation definition is invalid or over budget.',
  UI_BOOTSTRAP_TIMEOUT: 'Logic did not deliver the first page state in time.',
  UI_BRIDGE: 'The View-Logic connection failed or was replaced.',
  UI_BUFFER: 'A malformed wire buffer reached the native decoder.',
  UI_BUTTON: 'Invalid button label, or an interactive element inside a composed control.',
  UI_CHANNEL: 'Invalid `useChannel` call or channel failure.',
  UI_CHECK: 'Invalid checkbox or switch value, label or binding.',
  UI_CHROME: 'Invalid host chrome geometry from the platform adapter.',
  UI_CHILD: 'An element has children it does not accept, or lacks its required parent.',
  UI_CLASS: 'A class string is not a string, too long, or names a utility the build did not compile.',
  UI_CLASS_ELEMENT: 'A supported utility on an element that does not render it, or a state variant the element never enters.',
  UI_CLASS_UNKNOWN: 'Not a Tailwind utility; the message names the nearest supported one when close.',
  UI_CLASS_UNSUPPORTED: 'Tailwind CSS the native renderer cannot draw.',
  UI_COLOR: 'A color that is not a hex value or `currentColor` reached the decoder.',
  UI_COMMIT: 'A commit without a prepared candidate.',
  UI_COMPAT: 'The installed host is older than the compiled page; update the host app.',
  UI_COMPOSITE: 'A composited effect (backdrop, opacity group) is out of bounds.',
  UI_CSS: 'The page stylesheet has something other than tokens, or more than one is imported.',
  UI_DOM: 'A DOM global (`window`, `document`, …) is used in View code.',
  UI_ENVIRONMENT: 'The host reported an invalid environment.',
  UI_ERROR: 'A failure without its own code.',
  UI_FIELD: 'A wire field on a tag that does not accept it.',
  UI_FIELDS: 'A malformed field list reached the native decoder.',
  UI_FILTER: 'A backdrop filter outside its bounds.',
  UI_FRAME: 'The platform declined a frame; the next one retries.',
  UI_FRAGMENT: 'A JSX fragment; return one element.',
  UI_GRADIENT: 'A gradient with geometry its kind does not take, or invalid stops.',
  UI_GRID: 'An invalid grid track list or line.',
  UI_IMAGE: 'Invalid image source, alt, retry key or callback, or a failed load.',
  UI_INPUT: 'Invalid editor prop, binding or text.',
  UI_KEY: 'A non-string or duplicate sibling key.',
  UI_LEAVE: 'Invalid `useLeaveGuard` call or answer.',
  UI_LIMIT: 'A page limit was exceeded.',
  UI_LIST: 'Invalid list description, keys, anchor or row height.',
  UI_LOCAL: 'A duplicate `useLocal` name in one component, or `set` on an unmounted cell.',
  UI_MODULE: 'The compiled page module failed to load.',
  UI_PAGE: 'A call reached a page that was already discarded.',
  UI_PARENT: 'A node listed before its parent reached the native decoder.',
  UI_PATCH: 'An invalid state patch from Logic.',
  UI_PLOT: 'Invalid plot props.',
  UI_PROP: 'A prop the element does not take, or an invalid value.',
  UI_RADIO: 'Invalid radio options, value or label.',
  UI_RENDER: 'A state change during render, or a recursive render.',
  UI_REQUEST: 'Invalid `useRequest` call.',
  UI_ROOT: 'The page did not return exactly one element.',
  UI_SCOPE: 'A hook or JSX used outside render.',
  UI_SCROLL: 'Invalid scroll position from the host.',
  UI_SESSION: 'A call reached a page session that is not loaded.',
  UI_SHADOW: 'An invalid or over-budget box shadow.',
  UI_SLIDER: 'Invalid slider range, step, value or label.',
  UI_STALE: 'A candidate from another render.',
  UI_STATE: 'Invalid page state from Logic.',
  UI_STREAM: 'Invalid `useStream` call or stream failure.',
  UI_STYLE: 'A style key the element does not render, or an invalid combination (sticky, state paint).',
  UI_STYLE_VALUE: 'A style value out of range or of the wrong kind.',
  UI_SURFACE: 'The platform surface was lost or resized mid-frame; the host recovers.',
  UI_TAG: 'Not an element of the View language.',
  UI_TEST_ID: 'An invalid `testId` reached the native decoder.',
  UI_TEXT: 'A `<span>`/`<link>` outside `<text>`, a bad link label, or too many runs.',
  UI_TIMEOUT: 'View code exceeded its time budget; the page needs a reload.',
  UI_TRANSFORM: 'A transform the element cannot take (rotate/scale on editors, scrolls, sliders).',
  UI_TRANSITION: 'An invalid transition definition (gated motion).',
  UI_VECTOR: 'Invalid vector paths, gradients or viewBox.',
  UI_VIEWPORT: 'The host reported an invalid viewport.',
  UI_WORKER: 'The View worker thread failed to start.',
};

async function files(dir, test) {
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) { if (!['tests', 'generated', 'node_modules'].includes(entry.name)) out.push(...await files(path, test)); }
    else if (test(entry.name)) out.push(path);
  }
  return out.sort();
}

// Native modules keep their tests after `#[cfg(test)]`; only shipped code raises codes.
const shipped = (path, text) => path.endsWith('.rs') ? text.split(/^#\[cfg\(test\)\]/m)[0] : text;

async function errors() {
  const sources = [
    // The compatibility banner is emitted by the compiler but runs when the page loads.
    ['build', join(packageRoot, 'compiler'), name => name.endsWith('.mjs') && name !== 'compatibility.mjs'],
    ['runtime', join(packageRoot, 'compiler'), name => name === 'compatibility.mjs'],
    ['runtime', join(packageRoot, 'src'), name => name.endsWith('.js')],
    ['native', join(repositoryRoot, 'crates/lingxia-ui/src'), name => name.endsWith('.rs') && !/tests?\.rs$|_tests\.rs$/.test(name)],
    ['native', join(repositoryRoot, 'crates/lingxia/src/gpu_page'), name => name.endsWith('.rs') && !/tests?\.rs$/.test(name)],
  ];
  const found = new Map();
  for (const [where, dir, test] of sources) for (const path of await files(dir, test)) {
    const text = shipped(path, await readFile(path, 'utf8'));
    for (const match of text.matchAll(/["'`](UI_[A-Z][A-Z_]*[A-Z])(?=[:"'`])/g)) {
      const entry = found.get(match[1]) ?? { where: new Set() };
      entry.where.add(where);
      found.set(match[1], entry);
    }
  }
  for (const key of Object.keys(errorNotes)) if (!found.has(key)) throw Error(`docs.mjs: error note for ${key}, which no source raises`);
  const order = ['build', 'runtime', 'native'];
  const rows = [...found].sort(([a], [b]) => a.localeCompare(b))
    .map(([key, entry]) => [code(key), order.filter(where => entry.where.has(where)).join(', '), errorNotes[key] ?? '']);
  return [header, '# Error codes', '',
    'Every message starts with its code: `CODE: detail`. **build** codes stop `lingxia build` with',
    '`file:line:col`; **runtime** codes come from the View runtime with the JSX trace (`at <tag> (file:line:col)`);',
    '**native** codes come from the host decoder, layout or renderer and keep the last good frame.', '',
    table(['Code', 'Raised at', 'Meaning'], rows), ''].join('\n');
}

/**
 * Map of repository-relative path to generated reference page. `generated` is the
 * contract generation the pages describe; it defaults to a fresh one from `schema`.
 */
export async function docs(schema, generated) {
  generated ??= await generate(schema);
  const [system, { compileUtilities }] = await Promise.all([designSystem(), lowering(generated)]);
  // One lowering pass covers every utility and every variant probe.
  const names = system.getClassList().map(([name]) => name);
  const rules = await compileUtilities(new Set([...names, ...variantCandidates(system).map(([, candidate]) => candidate), 'placeholder:text-red-500']));
  const base = 'docs/view/reference';
  return {
    [`${base}/elements.md`]: await elements(schema),
    [`${base}/styles.md`]: styles(schema),
    [`${base}/utilities.md`]: utilities(system, rules),
    [`${base}/variants.md`]: variants(schema, system, rules),
    [`${base}/limits.md`]: limits(schema),
    [`${base}/errors.md`]: await errors(),
    [`${base}/environment.md`]: await environment(),
    [`${base}/semantics.md`]: semantics(schema),
  };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const schema = await readSchema();
  // Once generate.mjs includes the reference, its generation already holds these pages.
  const fresh = await generate(schema);
  const generated = Object.fromEntries(Object.entries(fresh).filter(([path]) => path.startsWith('docs/view/reference/')));
  if (!Object.keys(generated).length) Object.assign(generated, await docs(schema, fresh));
  if (process.argv.includes('--check')) {
    const stale = [];
    for (const [path, content] of Object.entries(generated))
      if ((await readFile(join(repositoryRoot, path), 'utf8').catch(() => null))?.replace(/\r\n/g, '\n') !== content) stale.push(path);
    if (stale.length) {
      console.error(`View reference is stale; run node ${relative(process.cwd(), fileURLToPath(import.meta.url))}:\n  ${stale.join('\n  ')}`);
      process.exit(1);
    }
  } else for (const [path, content] of Object.entries(generated)) {
    await mkdir(dirname(join(repositoryRoot, path)), { recursive: true });
    await writeFile(join(repositoryRoot, path), content);
  }
}
