// The View language reference (docs/view/reference), derived from schema.json, the
// authoring API types, the real Tailwind lowering and the sources that raise UI_* codes.
// `--check` fails on drift instead of writing.
import { readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { readSchema, documented, typedTrait, elementStyle, rules, generate, contractsRoot, packageRoot, repositoryRoot, resolveDependency, loadDependency } from './generate.mjs';

const ts = (await loadDependency('typescript')).default;
const { build } = await loadDependency('esbuild');

const header = '_Generated from the View schema (`schema.json`) and the compiler; do not edit._\n';
// A code span that may itself hold backticks (template literal types).
const code = text => String(text).includes('`') ? `\`\` ${text} \`\`` : `\`${text}\``;
const cell = text => String(text).replace(/\|/g, '\\|').replace(/\n/g, ' ');
const table = (head, rows) => [`| ${head.join(' | ')} |`, `| ${head.map(() => '---').join(' | ')} |`,
  ...rows.map(row => `| ${row.map(cell).join(' | ')} |`)].join('\n');
const list = items => items.map(code).join(', ');

// ---- authoring API types --------------------------------------------------------------

async function apiMembers(name) {
  const path = join(contractsRoot, 'api.d.ts');
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
    out.push(`## \`<${shown(schema, name)}>\``, '');
    // A run may hold runs, but a link never holds another link.
    const children = element.children === 'runs' && element.wire === 'link' ? 'text and `<span>` runs; links do not nest' : childKinds[element.children];
    const facts = [element.block ? `Inside ${code(`<${parents[element.wire]}>`)}, a run: children ${children}.` : `Children: ${children}.`];
    if (element.wire !== name) facts.push(`Renders as ${code(element.wire)}.`);
    if (element.block) facts.push(`Elsewhere, a block link (${code(element.block.wire)}): a pressable box whose children are ${childKinds[element.block.children]}, styled like \`<button>\`; a block link never sits inside another (\`UI_LINK\`).`);
    else if (parents[element.wire]) facts.push(`Only inside ${code(`<${parents[element.wire]}>`)}.`);
    const run = rules(schema).run[name];
    if (run) facts.push(`${element.block ? 'As a run it' : 'It'} takes, of the props every element takes, only ${run.filter(prop => prop !== 'children').map(code).join(', ')}; the rest go on its \`<text>\` (\`UI_PROP\`). Its \`data-testid\` finds the run itself: its rect is the run's own words, so a tap lands on them.`);
    for (const trait of Object.values(rules(schema).trait).filter(entry => entry.on.includes(name))) {
      const refused = (trait.forms ?? []).flatMap(form => Object.keys(form.without ?? {}).map(prop => `${code(prop)} not with ${form.values.map(value => code(`${trait.key}=${JSON.stringify(value)}`)).join(' or ')}`));
      facts.push(`${trait.props.filter(prop => element.props[prop]).map(code).join(', ')} need ${code(trait.key)}${refused.length ? `; ${refused.join('; ')}` : ''}: ${typedTrait(element, trait) ? 'tsc and the build refuse' : 'the build refuses'} them otherwise (\`UI_PROP\`).`);
    }
    for (const group of Object.values(rules(schema).exclusive).filter(entry => entry.on.includes(name)))
      facts.push(`At most one of ${group.props.map(prop => `${code(prop)}${group.with?.[prop] ? ` (and its ${group.with[prop].map(code).join(', ')})` : ''}`).join(' or ')} (\`UI_PROP\`).`);
    if (name === 'link') facts.push('Needs `to`, `onClick`, `popoverTarget` or `interestFor`, disabled or not: `disabled` only stops the press (`UI_LINK`). Enter activates it; Space does not.');
    if (admission.root) facts.push('The page root.');
    // Controls nest inside a pressable control; scrolls, lists and webviews are refused there.
    if (name === 'scroll' || element.wire === 'list' || name === 'webview') facts.push('Not allowed inside a pressable control (`UI_BUTTON`).');
    if (name === 'webview') facts.push('Moves live with a translation. Under opacity, a mask, a blur, a scale or a rotation it shows a still picture of the page and takes no input until the effect ends. Scaling or rotating it outside entering and leaving fails the build (`UI_TRANSFORM`); a scale or rotation applied at run time shows the still picture.');
    if (name === 'webview') facts.push('Its page\'s `prefers-color-scheme` follows the View page\'s appearance, live.');
    out.push(facts.join(' '), '');
    if (element.binding) {
      const { type, event } = element.binding;
      out.push(`Binding: \`bind={local}\` with a \`useLocal<${type}>\` cell, or \`value\` plus \`${event}(value: ${type})\`, not \`value\` with \`bind\`. Beside \`bind\`, \`${event}\` hears each change after the cell took it and may rewrite the cell.${event === 'onInput' ? ' `onInput` runs on every value change, a composition included.' : ''}`, '');
      const { end, pair } = element.binding;
      if (end) out.push(`\`${end}(value)\` runs once a change is finished: when the press that made it lifts, or at once for a key or assistive step.`, '');
      if (pair) out.push(`Two values: \`${pair.values}={[lower, upper]}\` with \`${pair.labels}={[name, name]}\` makes a control with two thumbs that cannot cross; \`${event}\`${end ? ` and \`${end}\`` : ''} then take \`[lower, upper]\`.`, '');
    }
    if (element.component) out.push(`A component from \`@lingxia/view\`, rendered as the \`list\` element.`, '');
    const own = Object.entries(element.props).filter(([prop]) => !element.component?.replaces.includes(prop));
    const props = [...Object.entries(element.component?.props ?? {}), ...own];
    if (props.length) out.push(table(['Prop', 'Type', 'Required', 'Notes'], props.map(([prop, entry]) => [code(prop), code(entry.type), entry.required ? 'yes' : '', entry.doc])), '');
    else out.push('No element-specific props.', '');
    if (propNotes[name]) out.push(propNotes[name], '');
  }
  return out.join('\n');
}

// The name an author writes: `<List>` for the element behind that component.
const shown = (schema, name) => schema.elements[name]?.component?.name ?? name;

// What a prop does where its type does not say it.
const propNotes = {
  checkbox: '`defaultChecked`: where a checkbox without `value` or `bind` starts; it applies once, and a later change does nothing.',
  switch: '`defaultChecked`: where a switch without `value` or `bind` starts; it applies once, and a later change does nothing.',
  slider: '`defaultValue`: where a slider without `value` or `bind` starts, snapped to `step`; it applies once, and a later change does nothing.',
  select: '`defaultValue`: the option a select without `value` or `bind` starts on; it applies once, and a later change does nothing.',
  scroll: '`bind` and `snapIndex` are the index of the snap point a `scrollSnapType` scroll rests on, not its offset: `bind={local}` with a number `useLocal` cell, or `snapIndex={n}` with `onSnapChange(index)` to follow it from the page\'s or Logic\'s state. Not both (`UI_PROP`). After a swipe the element waits for the page\'s answer; if the page keeps its index, the element returns to it. The answer may come through a Logic action `onSnapChange` starts; the wait ends at 400 ms. Only actions the handler calls directly count: one started after an `await` or in a timer is not waited for, and the page is taken to keep its index after the next frame. The same holds on a dragged element.',
  video: '`src` plays one file; `sources` offers the same video at several qualities; not both (`UI_PROP`). Props apply when they change: a new `src` or `sources` starts over from the beginning, showing the last frame until the new first one; `paused`, `muted`, `loop`, `playbackRate` and `volume`: each applies on mount and when its value changes; writing the same value again does nothing, so a handle\'s `play()` or `pause()` stands until that prop itself changes. To act, use the player handle. A named player is the advanced case: `player={useVideo(name)}` on the `<video>` that gives the source makes it the page\'s player, and another `<video player>` with no source, such as a full-screen one, shows that same player. A component that forwards a source types it with `VideoSourceProps` and spreads it whole.',
  webview: '`handlesDrags`: the web page handles its own drags (a map, a canvas); a drag starting on it never passes to the page. Off by default. When the screen takes a drag the page had started, the page gets `pointercancel` and `touchcancel`; on HarmonyOS they are synthetic (`isTrusted` false), and the page\'s own overflow scrolling, fixed when the finger lands, keeps scrolling.',
};

// ---- styles ---------------------------------------------------------------------------

function styles(schema) {
  const accepted = Object.fromEntries(Object.entries(schema.elements).map(([name, element]) => [name, elementStyle(schema, element)]));
  const names = Object.keys(schema.elements);
  const out = [header, '# Style properties', '',
    'CSS-named keys of the `style` prop and the targets of every utility. A key is accepted only by the',
    'elements listed; anything else fails the build with `UI_STYLE` (inline) or `UI_CLASS_ELEMENT` (utility).',
    'Numbers are logical pixels. **Paint** keys are the only ones `focus:`, `active:` and `hover:` may change.', '',
    table(['Key', 'Type', 'CSS', 'Paint', 'Accepted by', 'Notes'], Object.entries(schema.style).filter(([, entry]) => !entry.internal).map(([key, entry]) => {
      const by = names.filter(name => accepted[name].includes(key));
      return [code(key), code(entry.type), [entry.css ?? []].flat().map(code).join(', '), entry.paint ? 'yes' : '',
        by.length === names.length ? 'every element' : by.length > names.length / 2
          ? `all but ${names.filter(name => !by.includes(name)).map(name => code(`<${shown(schema, name)}>`)).join(' ')}` : by.map(name => code(`<${shown(schema, name)}>`)).join(' '), entry.doc];
    })), '',
    '`transition` and `animation` are set only by the motion utilities (`transition-*`, `animate-*`).', '',
    '## By element', ''];
  for (const name of names) out.push(`### \`<${shown(schema, name)}>\``, '', accepted[name].filter(key => !schema.style[key].internal).map(code).join(', '), '');
  out.push('## Colour roles', '',
    'Every colour is one of these roles, optionally with an alpha percent (`accent/12`). The utility drops a `text-`',
    'prefix (`text-muted`, not `text-text-muted`). The theme pack resolves each role per appearance and proves the',
    'listed contrast minimums. `defaultPalette` from `@lingxia/view/theme` holds the default pack resolved for light,', 'dark, `high-contrast-light` and `high-contrast-dark`, derived accent roles included, for proving contrast in code.',
    'In TypeScript a role is `ColorRole`, an app colour `AppColor` (`app.<name>`), a style colour `ColorValue` (either,',
    'with an optional `/NN`, or `transparent`) and a `<plot>` series `ColorSeries`. A theme\'s own `colors` add',
    'role-shaped classes and `app.<name>` values beside the roles.', '',
    'A pack writes the authored roles. Fixed roles are the same in every pack, and derived roles are computed from the',
    'authored ones and the accent seed, unless a pack sets them.', '',
    table(['Role', 'Set by', 'Minimum contrast', 'Use'], schema.roles.filter(role => !role.internal).map(role => [code(role.name), { authored: 'the pack', fixed: 'fixed', derived: 'derived' }[role.tier],
      Object.entries(role.on ?? {}).map(([background, ratio]) => `${ratio}:1 on ${code(background)}`).join(', '), role.doc])), '');
  out.push('### Hover and pressed fills', '',
    'A pressed or hovered fill deepens; where that would make its label hard to read, it lightens instead. A pack decides',
    'the direction once per fill, from its pressed state: a brand fill with dark text in dark appearance brightens on press.',
    'A lighter press keeps its label at 4.5:1 and the fill at 3:1 (or its rest contrast) against `surface` and `canvas`; a',
    'written fill that holds neither way fails the build, naming the nearest fill that passes.', '',
    table(['Fill and label', 'Rest', 'Hover', 'Pressed'], [
      ['`#c6f36b` under `#1a2410` (light): deeper', '`#c6f36b` 12.6:1', '`#bae564` 11.1:1', '`#aed65d` 9.6:1'],
      ['`#5285ff` under `#0a1420` (dark): lighter', '`#5285ff` 5.4:1', '`#5e97ff` 6.5:1', '`#69a8ff` 7.6:1'],
    ]), '');
  return out.join('\n');
}

// ---- utilities and variants -------------------------------------------------------------

const contractPath = 'packages/lingxia-view/src/generated/contract.js';

// The Tailwind lowering bound to this generation's contract, not the committed one, so one
// `generate` pass converges. Packages stay external and load from their resolved files.
async function lowering(generated) {
  const contract = generated[contractPath];
  if (!contract) throw Error(`docs.mjs: the generation lacks ${contractPath}`);
  // This module sits in the compiler's package, so its ESM resolution matches the compiler's imports.
  const theme = resolveDependency('tailwindcss/theme.css');
  const plugin = { name: 'fresh-contract', setup(builder) {
    // `kind`: a Windows entry path (`D:\…`) also starts with neither `.` nor `/`.
    builder.onResolve({ filter: /^[^./]/ }, ({ path, kind }) => kind === 'entry-point' ? undefined
      : ({ path: path.startsWith('node:') ? path : resolveDependency(path), external: true }));
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
  const { __unstable__loadDesignSystem: load } = await loadDependency('tailwindcss');
  const theme = await readFile(new URL(resolveDependency('tailwindcss/theme.css')), 'utf8');
  const { roleTheme, viewExtensions } = await import(pathToFileURL(join(packageRoot, 'compiler/tailwind.mjs')).href);
  return load(`${theme}\n${roleTheme}\n${viewExtensions}\n@tailwind utilities;`);
}

const strip = name => name.replace(/^-/, '');

function family(name, roots) {
  const bare = strip(name);
  let best;
  for (const root of roots) if ((bare === root || bare.startsWith(`${root}-`)) && (!best || root.length > best.length)) best = root;
  return best ?? bare.split('-')[0];
}

// Messages name concrete values; collapse them so one reason summarizes a family.
const reason = message => message
  .replace(/^UI_[A-Z_]+: /, '')
  .replace(/^([a-z-]+): .*$/, '$1')
  .replace(/:.*$/, '')
  .replace(/ \(.*\)$/, '')
  .trim();

// The style keys a rule sets; empty when the utility lowers to nothing the renderer draws.
const effects = rule => [...Object.keys(rule.style ?? {}), ...(rule.gradient ? ['backgroundImage'] : []), ...(rule.shadow ? ['boxShadow'] : []),
  ...(rule.backdrop ? ['backdropFilter'] : []), ...(rule.transform ? ['translate'] : []), ...(rule.numeric ? ['fontVariantNumeric'] : []),
  ...(rule.motion ? ['transition'] : []), ...(rule.mask ? ['maskImage'] : [])];

const sections = [
  ['Layout', /^(display|overflow|order)$/],
  ['Position', /^(position|top|right|bottom|left|inset.*|zIndex)$/],
  ['Flex and grid items', /^(flex(Grow|Shrink|Basis)|alignSelf|justifySelf|grid(Column|Row).*)$/],
  ['Flex and grid containers', /^(flexDirection|flexWrap|alignItems|justifyContent|alignContent|justifyItems|gap|rowGap|columnGap|grid(Template|Auto).*)$/],
  ['Sizing', /^(width|height|minWidth|minHeight|maxWidth|maxHeight|aspectRatio)$/],
  ['Margin', /^margin/], ['Padding', /^padding/],
  ['Typography', /^(color|font.*|lineHeight|letterSpacing|text.*|whiteSpace|wordBreak|overflowWrap|lineClamp|userSelect)$/],
  ['Backgrounds', /^background/], ['Radius', /Radius$/], ['Borders', /^border/],
  ['Effects', /^(opacity|boxShadow|backdropFilter|maskImage)$/], ['Transforms', /^(translate|rotate|scale|transformOrigin)$/],
  ['Motion', /^(transition|animation)$/], ['Scrolling', /^(overflowX|overflowY|overscrollBehavior)$/], ['Editors', /^placeholderColor$/], ['Images and vectors', /^object(Fit|Position)$/],
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
    const where = entry.supported.length ? entry.section : entry.rejected.size ? 'Refused' : 'Inert';
    (grouped[where] ??= []).push([key, entry]);
  }
  const out = [header, '# Utilities', '',
    'Every utility of Tailwind\'s default theme, run through the native lowering and grouped by family.',
    'Colour families accept only roles (`bg-surface`, `text-muted`); your `@theme` tokens add other names (`rounded-card`); arbitrary values',
    '(`w-[72px]`, `grid-cols-[1fr_auto]`) follow the rules of the family they extend. A supported utility',
    'is still refused on an element that does not render its style ([styles.md](styles.md)).',
    'Unlike Tailwind, `ring-offset-<n>` leaves a transparent gap between box and ring; `ring-offset-<role>` fills it. Rings are strokes, not shadows, yet they ease with `transition` and `transition-shadow`.', ''];
  for (const [title] of [...sections, ['Other']]) {
    const rows = grouped[title];
    if (rows?.length) out.push(`## ${title}`, '', table(['Family', 'Supported', 'Refused'],
      rows.map(([key, entry]) => [code(key), sample(entry.supported), reasons(entry.rejected)])), '');
  }
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
// `group-active:` is supported while the other `group-*` variants are refused, so it is probed too.
const variantCandidates = system => [...system.getVariants().map(variant => [variant.name, `${variantSamples[variant.name] ?? variant.name}:${probe}`]),
  ['group-active', `group-active:${probe}`]];

function variants(schema, system, rules) {
  const candidates = variantCandidates(system);
  const result = name => {
    const matched = rules.filter(rule => rule.name === name);
    const error = matched.find(rule => rule.error)?.error.message;
    return matched.length && !error ? null : error ?? 'not generated';
  };
  const wireTags = field => schema.fields.find(entry => entry.key === field).tags;
  const on = field => Object.entries(schema.elements).filter(([, element]) => wireTags(field).includes(element.wire)).map(([name]) => code(`<${shown(schema, name)}>`)).join(' ');
  const disableable = Object.entries(schema.elements).filter(([, element]) => element.props.disabled).map(([name]) => code(`<${name}>`)).join(' ');
  const line = part => `\`<slider>\`; its ${part}: \`bg-*\`, \`h-*\`, \`rounded-*\`; after \`active:\` \`hover:\` \`focus-visible:\` or \`disabled:\`, eased over 150 ms`;
  const parts = { track: '`<slider>` and `<switch>`; its track: `bg-*`, `h-*`, `rounded-*`, and `w-*` on a switch; after `active:` `hover:` `focus-visible:` or `disabled:`, and `checked:` on a switch (while it reads checked, bound or not; another state may not keep part of its paint on the same part), eased over 150 ms',
    range: line('range, min to value'), secondary: line('secondary range, min to `secondaryValue`'),
    thumb: '`<slider>` and `<switch>`; its thumb: `bg-*`, `size-*`, `rounded-*`, `border-*`, `shadow-*`, `ring-*` (with `ring-offset-*`, `inset-ring-*`), `scale-*`; after a state like the track' };
  const admission = {
    focus: `${on('focusStyle')}; paint only`, 'focus-visible': `${on('focusVisibleStyle')}; while focused from a key, switch or assistive tool (editors on any focus); paint only`, active: `${on('pressedStyle')}; paint only`,
    hover: 'any element, while a mouse, stylus or trackpad is over it; paint only', disabled: `${disableable}; replaces the whole class cascade`,
    placeholder: `editors; \`text-*\` color only`,
    dark: 'any element; not on colours, which follow appearance',
    backdrop: '`<dialog>` only; styles its scrim, which fades in and out with a dialog that has `starting:`/`exit:` styles, on its timing; `backdrop:starting:`/`backdrop:exit:` (paint only) and `backdrop:duration-*`/`ease-*`/`delay-*` change that',
    'group-active': 'any element inside a pressable control; while that control is pressed; paint only',
    starting: 'any element; paint only, the values it enters from',
    has: 'any element, alone; `has-checked:` (a descendant checkbox, switch or radio option that reads checked, bound or not), `has-disabled:` (a disabled descendant) and `has-focus:`; paint only',
    'focus-within': 'any element; while it or a descendant has focus; paint only',
    'side-top': 'any element in an anchored popover, the popover included; while that popover sits above its anchor, after any flip; paint only', 'side-bottom': 'any element in an anchored popover, the popover included; while that popover sits below its anchor, after any flip; paint only',
    'side-start': 'any element in an anchored popover, the popover included; while that popover sits at the inline start of its anchor (its left in LTR), after any flip; paint only', 'side-end': 'any element in an anchored popover, the popover included; while that popover sits at the inline end of its anchor (its right in LTR), after any flip; paint only',
    ...parts,
    data: 'any element; matches its own `data-*` prop (`data-active:`, `data-[state=open]:`), layout included',
    regular: 'any element; matches when the nearest `<SizeClassScope>` (or `env.sizeClass`) is `"regular"`, layout included',
    exit: 'any element; paint only, the values a removed element leaves toward',
  };
  const supported = [], refused = [];
  for (const [name, candidate] of candidates) {
    const sample = candidate.slice(0, -probe.length - 1);
    const error = name === 'placeholder' ? result('placeholder:text-muted') : parts[name] ? result(`${name}:bg-accent`) : result(candidate);
    if (error) refused.push([code(`${name}:`), code(`${sample}:`), error === 'not generated'
      ? 'matches no rule; the build reports it as `UI_CLASS_UNKNOWN`' : error.replace(/^UI_[A-Z_]+: /, '')]);
    else supported.push([code(`${name}:`), admission[name] ?? 'any element']);
  }
  return [header, '# Variants', '',
    'Every Tailwind variant, tested against the native lowering. Supported variants may be stacked',
    '(`md:active:bg-surface-sunken`). Conditions on DOM state the View does not have are refused at build time.', '',
    '## Supported', '', table(['Variant', 'Applies to'], supported), '',
    'There is no `group-checked:` or `peer-checked:`: a container reads the controls inside it with `has-checked:`.', '',
    '`space-x-*`, `space-y-*` and `divide-*` are utilities, not variants: they style every child but the last.', '',
    '## Refused', '', table(['Variant', 'Probed as', 'Reason'], refused), ''].join('\n');
}

// ---- limits, environment, semantics -----------------------------------------------------

const limitNotes = {
  nodes: 'Nodes in one rendered frame, list rows included.',
  depth: 'Nesting depth of the rendered tree.',
  animationBytes: 'Animation definitions per page, as UTF-8 JSON.',
  itemKeyBytes: 'All `itemKeys` of a page, as UTF-8 JSON.',
  imageBytes: 'Encoded size of one image resource.',
  sourceBytes: 'Length of one image `src`.',
  visibleImageBytes: 'Decoded image bytes one page paints at once; past it an image gets `onError` and a placeholder.',
  decodedImageBytes: 'Decoded-image cache shared by every page; it evicts, never fails.',
  transitionMs: 'Longest transition duration or delay, in milliseconds.',
  evalBudgetMs: 'CPU-time budget for one View render or callback. A render that finishes past it is kept and warned about (`UI_OVERRUN`); a callback past it keeps its effects.',
  evalBudgetDebugMultiplier: 'Multiplier on that budget in unoptimized (debug) builds.',
  evalWarmupMultiplier: 'Budget multiple for a module\'s first render after it loads, reloads or is hot-swapped (that render also warms the engine), and for the render an appearance, language, text-size or size-class change causes, which runs the whole page again.',
  evalLimitMs: 'CPU time at which any render or callback is interrupted (ten times that in wall time for a hang) and the page fails with UI_TIMEOUT; scaled like `evalBudgetMs` in debug builds. It only catches runaway scripts, so a slow device running a long render keeps the page.',
  editorTextBytes: 'Text one editor holds, as UTF-8.',
  inputRequestBytes: 'One input-method request from the platform, as UTF-8 JSON.',
  pasteHtmlBytes: 'The HTML flavour an `onPaste` event carries; a larger one is listed in `omitted`.',
  pasteRequestBytes: 'One paste request from the platform, its text and HTML as UTF-8 JSON.',
  inputBatchEdits: 'Edits in one input-method batch.',
  clipboardItems: 'Clipboard items one paste reads.',
  wireValues: 'Values in one encoded frame.',
  textBytes: 'Text of one node, as UTF-8; a `<text>` with `<span>` and `<link>` runs counts all of them together.',
  identifierBytes: 'One `key` or `data-testid`, as UTF-8.',
  listRows: 'Rows one `<List>` holds.',
  gridTracks: 'Tracks one grid template expands to.',
  vectorSourceBytes: 'One `<vector>` source (paths, gradients, viewBox) as JSON.',
  vectorPathBytes: 'One path `d` string.',
  vectorPathSegments: 'Segments of one path `d` (each move, line, curve and close; an arc may take several); past it the path fails at build when `d` is written out, at render otherwise. Split it into several paths.',
  vectorPathWork: 'Path elements one `<vector>` draws, strokes outlined.',
  vectorWork: 'Path elements all vectors of one frame draw.',
  plotWork: 'Geometry all plots of one frame draw.',
  shadowMaskBytes: 'Blurred shadow masks one frame rasterizes.',
  compositeFrames: 'Frame-sized working surfaces nested opacity, clip and backdrop layers may hold at once (at least 64 MiB); past it the deepest groups flatten.',
  leaseFrames: 'Frame-sized GPU textures a page may hold at once across prepared frames and kept animation layers (at least 64 MiB, under `compositeBytes`); past it the frame draws without the kept layer.',
  compositeBytes: 'Absolute ceiling on those working surfaces, for very large windows.',
  ghosts: 'Removed elements still drawing their exit at once; past it further removals vanish at once.',
  layoutMotions: 'Elements moving to a new layout position under `transition-layout` at once; past it further moves land at once.',
  blurLayers: 'Elements a frame draws with `blur-*`; past it further ones draw sharp.',
  dragSnapPoints: 'Snap points one `drag` element declares in `dragSnapPoints`.',
  topLayer: 'Dialogs and popovers open at once; a further one stays closed (`UI_BUDGET`) and fires no `onClose` or `onToggle`.',
  dataAttributeBytes: '`data-*` props of a page, as UTF-8 JSON; automation reads them, nothing draws them.',
  webMessageBytes: 'One `window.lingxia.postMessage` payload of a `<webview>` page, as UTF-8 JSON; past it the call throws in the web page.',
  lineHeightMultiplier: 'Largest unitless `lineHeight`, a multiple of the font size; a larger number is almost always meant as points, so it fails (build and render) and names `"<n>px"` or a `leading-*` class.',
  styleNumber: 'Largest length in points a style takes (sizes, spacing, insets, translations, line heights; negative where the style allows it); past it the render fails with `UI_STYLE_VALUE`.',
  webMessageQueue: 'Queued (`{ queue: true }`) messages of one `<webview>` waiting for `onMessage`; past it the oldest is dropped and `onError` reports `WEBVIEW_MESSAGE_DROPPED` once.',
  outlineGlyphs: "Glyphs the platform outlines (Apple's PingFang UI faces) in one frame, each filled as a path; past it frames slow down.",
};

const size = value => value >= 1 << 20 && value % (1 << 20) === 0 ? `${value / (1 << 20)} MiB`
  : value >= 1 << 10 && value % (1 << 10) === 0 ? `${value / (1 << 10)} KiB` : null;

// Engine and wire bounds an author cannot act on; they are documented for work on the renderer.
const engineLimits = new Set(['decodedImageBytes', 'evalBudgetDebugMultiplier', 'evalWarmupMultiplier',
  'evalLimitMs', 'inputRequestBytes', 'pasteRequestBytes', 'inputBatchEdits',
  'clipboardItems', 'wireValues', 'shadowMaskBytes', 'compositeFrames', 'leaseFrames', 'compositeBytes', 'outlineGlyphs',
  'ghosts', 'layoutMotions']);

function limitTable(schema, keep) {
  return table(['Limit', 'Value', 'Meaning'], Object.entries(schema.limits).filter(([key]) => keep(key)).map(([key, value]) => {
    const bytes = /Bytes$/.test(key) ? size(value) : null;
    return [code(key), bytes ? `${value} (${bytes})` : value, limitNotes[key] ?? ''];
  }));
}

function limits(schema) {
  for (const key of Object.keys(limitNotes)) if (!(key in schema.limits)) throw Error(`docs.mjs: limit note for unknown limit ${key}`);
  for (const key of Object.keys(schema.limits)) if (!limitNotes[key]) throw Error(`docs.mjs: limit ${key} needs a note`);
  return [header, '# Limits', '',
    'Page-wide bounds the runtime and the native decoder both enforce. Exceeding one fails the frame',
    '(`UI_LIMIT`, `UI_LIST`, `UI_TEXT`, `UI_ANIMATION`), except the image budgets noted below.', '',
    limitTable(schema, key => !engineLimits.has(key)), ''].join('\n');
}

function engineLimitsPage(schema) {
  return [header, '# View engine limits', '',
    'Wire and renderer bounds behind the public [limits](../view/reference/limits.md): they keep a frame',
    'decodable and its GPU memory bounded, and page code cannot act on them.', '',
    limitTable(schema, key => engineLimits.has(key)), '',
    `Wire ABI: ${schema.uiAbi}. Highest field id: ${schema.fields.length - 1}.`, '',
    '## Dev-session globals', '',
    'An isolate created while a `lingxia dev` session is attached also has these globals. They exist only',
    'there; a page must not depend on them, and production isolates are created without them.', '',
    '- `__lingxiaClock()`: microseconds, as a number, since the first dev-session isolate of the process',
    '  was created, for timing View code on a device.',
    '- `__lingxiaClock.phases = true` opts the page into eval-phase totals: the runtime times its render',
    '  phases with the clock and reports them in the frame metrics (`viewPhases`). Off, a span costs one',
    '  property read.',
    '- `__lingxiaWarn(message)`: the runtime\'s dev diagnostics (`UI_COMPAT`, `UI_INPUT`, style clamps). The',
    '  host logs them with the frame\'s budget warnings; at most 32 wait between collections.', ''].join('\n');
}

async function environment(schema) {
  const members = Object.entries(schema.environment).filter(([, entry]) => !entry.internal);
  return [header, '# Environment', '',
    '`view.env` (from `render(view)` or `useView()`, or `useEnv()` in a component): read-only, and a change re-renders the page.', '',
    table(['Field', 'Type', 'Notes'], members.map(([name, entry]) => [code(name + (entry.absent ? '?' : '')), code(entry.type), entry.doc ?? ''])), ''].join('\n');
}

function semantics(schema) {
  const { roles, liveRegions, actions } = schema.semantics;
  const authored = schema.options.role, admits = rules(schema);
  // Where a role goes, from schema.json `rules`; a failing combination stops the build (`UI_PROP`).
  const where = role => (admits.role[role] ?? []).map(rule => [rule.tags.map(tag => code(`<${tag === 'link-box' ? 'link' : tag}>`)).join(' or '),
    ...(rule.with ?? []).map(prop => `with ${code(prop)}`)].join(' ')).join('; ') || 'any non-control element';
  const projected = role => role === 'img' ? 'image' : roles.includes(role) ? role : roles.find(name => name.replace(/-/g, '') === role);
  return [header, '# Semantics', '',
    'The accessibility tree the platform adapter exposes. Elements get their role from what they are;',
    'the `role` prop sets one of the authored roles on any non-control element.', '',
    '## Authored roles', '', table(['`role`', 'Goes on', 'Exposed as'], authored.map(role => [code(role), where(role), code(projected(role))])), '',
    ...Object.entries(admits.elementRole).filter(([, rule]) => rule.roles.length).map(([tag, rule]) => `A ${tag === 'link-box' ? `block ${code('<link>')}` : code(`<${shown(schema, tag)}>`)} takes only ${list(rule.roles)} (\`UI_PROP\`).`), '',
    `${Object.entries(admits.elementRole).filter(([, rule]) => !rule.roles.length).map(([tag]) => code(`<${shown(schema, tag)}>`)).join(', ')} keep their own role and take no \`role\` (\`UI_PROP\`).`, '',
    '## Props with conditions', '',
    'A literal combination that breaks one of these fails the build; a computed one fails at render (`UI_PROP`).', '',
    table(['Prop', 'Needs'], Object.entries(admits.prop).map(([prop, list]) => [code(prop), list.map(rule => `${rule.on ? `on ${rule.on.map(tag => code(`<${tag}>`)).join(', ')}: ` : ''}${code(rule.error)}`).join(' · ')])), '',
    '## All exposed roles', '', list(roles), '',
    '## Live regions', '', `\`aria-live\`: ${list(schema.options['live-region'])}. Announced politeness: ${list(liveRegions)}.`, '',
    '## Text actions', '', `Selectable text supports ${list(actions)}.`, ''].join('\n');
}

// ---- error codes ------------------------------------------------------------------------

// Codes no host test can trigger, and why; every other code needs a test that names it.
const untestable = {
  UI_SURFACE: 'needs a device without a compatible GPU or surface format',
  UI_WORKER: 'needs the OS to refuse a thread',
  UI_FONT: "needs the platform's system font files",
};

// Native modules keep their tests in a trailing `#[cfg(test)] mod … {`; only shipped code raises
// codes. A `#[cfg(test)]` on a module declaration hides nothing.
const shipped = (path, text) => {
  if (path.endsWith('.rs')) return text.split(/^#\[cfg\(test\)\]\s*(?:pub(?:\(crate\))? )?mod \w+ \{/m)[0];
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
};

// Only actual raises in JS: a code starting a string/template before `:` (or after `${...}: `),
// or a standalone code literal passed to a helper. Prose mentioning a code does not count.
export function raisedCodesInJs(text) {
  const stripped = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
  const codes = new Set();
  for (const match of stripped.matchAll(/(?:["'`]|:\s+)(UI_[A-Z][A-Z0-9_]*[A-Z0-9])(?=:)|["'`](UI_[A-Z][A-Z0-9_]*[A-Z0-9])["'`]/g)) {
    codes.add(match[1] ?? match[2]);
  }
  return codes;
}

async function files(dir, test) {
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) { if (!['tests', 'generated', 'node_modules'].includes(entry.name)) out.push(...await files(path, test)); }
    else if (test(entry.name)) out.push(path);
  }
  return out.sort();
}

// Test sources: Rust test modules and files, JS tests and the compiled docs examples.
async function testText() {
  const all = async (dir, test) => (await readdir(dir, { withFileTypes: true, recursive: true }))
    .filter(entry => entry.isFile() && test(entry.name) && !join(entry.parentPath, entry.name).includes('node_modules'))
    .map(entry => join(entry.parentPath, entry.name));
  let text = '';
  for (const path of await all(join(repositoryRoot, 'crates/lingxia-view'), name => name.endsWith('.rs'))) {
    const source = await readFile(path, 'utf8');
    if (/[/\\]tests?[/\\]|tests?\.rs$|_tests\.rs$/.test(path)) text += source;
    else if (source.includes('#[cfg(test)]')) text += source.slice(source.indexOf('#[cfg(test)]'));
  }
  for (const dir of [join(packageRoot, 'tests'), join(repositoryRoot, 'docs/view/guide')])
    for (const path of await all(dir, name => /\.(mjs|md)$/.test(name))) text += await readFile(path, 'utf8');
  return text;
}

const VALID_STAGES = new Set(['build', 'runtime', 'native']);
const VALID_SEVERITIES = new Set(['error', 'warning', 'recovery']);

export async function errors(schema) {
  const tableEntries = schema.errors;
  if (!tableEntries || typeof tableEntries !== 'object') throw Error('docs.mjs: schema.json is missing the errors map');
  for (const [key, entry] of Object.entries(tableEntries)) {
    if (!Array.isArray(entry.stages) || !entry.stages.length || entry.stages.some(stage => !VALID_STAGES.has(stage)))
      throw Error(`docs.mjs: ${key} has invalid stages in schema.json`);
    if (!VALID_SEVERITIES.has(entry.severity))
      throw Error(`docs.mjs: ${key} has invalid severity in schema.json`);
    if (typeof entry.when !== 'string' || !entry.when.trim() || typeof entry.fix !== 'string' || !entry.fix.trim())
      throw Error(`docs.mjs: ${key} must have non-empty when and fix strings in schema.json`);
  }

  const sources = [
    // The compatibility banner is emitted by the compiler but runs when the page loads.
    ['build', join(packageRoot, 'compiler'), name => name.endsWith('.mjs') && name !== 'compatibility.mjs'],
    ['runtime', join(packageRoot, 'compiler'), name => name === 'compatibility.mjs'],
    ['runtime', join(packageRoot, 'src'), name => name.endsWith('.js')],
    ['native', join(repositoryRoot, 'crates/lingxia-view/src'), name => name.endsWith('.rs') && !/tests?\.rs$|_tests\.rs$/.test(name)],
    ['native', join(repositoryRoot, 'crates/lingxia/src/gpu_page'), name => name.endsWith('.rs') && !/tests?\.rs$|_tests\.rs$/.test(name)],
  ];
  const found = new Map();
  const scanFile = async (where, path) => {
    const raw = await readFile(path, 'utf8');
    const text = shipped(path, raw);
    const codes = where === 'native'
      ? [...text.matchAll(/["'`](UI_[A-Z][A-Z0-9_]*[A-Z0-9])(?=[:"'`])/g)].map(m => m[1])
      : [...raisedCodesInJs(raw)];
    for (const key of codes) {
      const entry = found.get(key) ?? { where: new Set() };
      entry.where.add(where);
      found.set(key, entry);
    }
    return text;
  };
  const sharedBuildSrc = new Set();
  for (const [where, dir, test] of sources) for (const path of await files(dir, test)) {
    const text = await scanFile(where, path);
    if (where === 'build') {
      for (const imp of text.matchAll(/from\s+['"](\.\.\/src\/[^'"]+\.js)['"]/g)) {
        if (!imp[1].includes('/generated/')) sharedBuildSrc.add(join(packageRoot, imp[1].slice(3)));
      }
    }
  }
  for (const path of sharedBuildSrc) await scanFile('build', path);

  const missingInSchema = [...found.keys()].filter(key => !tableEntries[key]);
  if (missingInSchema.length) throw Error(`docs.mjs: error codes raised in source but missing from schema.json: ${missingInSchema.join(', ')}`);

  for (const [key, entry] of found) {
    const declared = new Set(tableEntries[key].stages);
    for (const stage of entry.where) {
      if (stage !== 'native' && !declared.has(stage)) {
        throw Error(`docs.mjs: ${key} is raised at ${stage} in JS but schema.json lists stages [${tableEntries[key].stages.join(', ')}]`);
      }
    }
  }

  const staleJsStages = [];
  for (const [key, entry] of Object.entries(tableEntries)) {
    const stages = found.get(key)?.where;
    for (const stage of entry.stages) {
      if (stage !== 'native' && !stages?.has(stage)) staleJsStages.push(`${key} (${stage})`);
    }
  }
  if (staleJsStages.length) throw Error(`docs.mjs: schema.json lists JS error stages that no JS source raises: ${staleJsStages.join(', ')}`);

  const unraised = Object.keys(tableEntries).filter(key => !found.has(key));
  if (unraised.length) throw Error(`docs.mjs: schema.json lists error codes that no source raises: ${unraised.join(', ')}`);

  const tested = await testText();
  const untested = [...found.keys()].filter(key => !tested.includes(key) && !untestable[key]);
  if (untested.length) throw Error(`docs.mjs: every error code needs a test that names it: ${untested.join(', ')}`);

  const order = ['build', 'runtime', 'native'];
  const rows = Object.keys(tableEntries).sort((a, b) => a.localeCompare(b))
    .map(key => {
      const entry = tableEntries[key];
      const stages = order.filter(stage => entry.stages.includes(stage)).join(', ');
      return [code(key), stages, entry.severity, entry.when, entry.fix];
    });
  return [header, '# Error codes', '',
    'Every message starts with its code: `CODE: detail`. **build** codes stop `lingxia build` with',
    '`file:line:col`; **runtime** codes come from the View runtime with the JSX trace (`at <tag> (file:line:col)`);',
    '**native** codes come from the host decoder, layout or renderer and keep the last good frame.', '',
    table(['Code', 'Raised at', 'Severity', 'When', 'Fix'], rows), ''].join('\n');
}

/**
 * Map of repository-relative path to generated reference page. `generated` is the
 * contract generation the pages describe; it defaults to a fresh one from `schema`.
 */
export async function docs(source, generated) {
  generated ??= await generate(source);
  const schema = documented(source);
  const [system, { compileUtilities }] = await Promise.all([designSystem(), lowering(generated)]);
  // One lowering pass covers every utility and every variant probe.
  const names = system.getClassList().map(([name]) => name);
  const rules = await compileUtilities(new Set([...names, ...variantCandidates(system).map(([, candidate]) => candidate), 'placeholder:text-muted', ...['track', 'range', 'secondary', 'thumb'].map(part => `${part}:bg-accent`)]));
  const base = 'docs/view/reference';
  return {
    [`${base}/elements.md`]: await elements(schema),
    [`${base}/styles.md`]: styles(schema),
    [`${base}/utilities.md`]: utilities(system, rules),
    [`${base}/variants.md`]: variants(schema, system, rules),
    [`${base}/limits.md`]: limits(schema),
    'docs/internal/view-engine-limits.md': engineLimitsPage(schema),
    [`${base}/errors.md`]: await errors(schema),
    [`${base}/environment.md`]: await environment(schema),
    [`${base}/semantics.md`]: semantics(schema),
  };
}

// Not top-level await: generate() imports this module back, and would wait on its evaluation.
const main = async () => {
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
};
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1])
  main().catch(error => { console.error(error); process.exit(1); });
