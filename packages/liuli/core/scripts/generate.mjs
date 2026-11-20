// Writes everything derived from the doc files: the reference pages (docs/<Name>.md), catalog.json
// for agents, and the gallery's component content and pages. `--check` writes nothing and fails on drift.
// A package folder as the argument generates that Liuli package instead; only core's run writes
// the gallery's index (catalog module, category pages, manifest), with every sibling package that
// has a catalog.json.
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { transform } from 'esbuild';
import ts from 'typescript';
import { appearances, format, needed, palette, ratio } from './contrast.mjs';

const packageArg = process.argv.slice(2).find(arg => !arg.startsWith('--'));
const core = resolve(packageArg ?? resolve(dirname(fileURLToPath(import.meta.url)), '..'));
const folder = basename(core);
const packageName = JSON.parse(readFileSync(join(core, 'package.json'), 'utf8')).name;
const isCore = folder === 'core';
const src = join(core, 'src');
const gallery = resolve(core, '../gallery');
const check = process.argv.includes('--check');
const generated = 'Generated from the doc file by scripts/generate.mjs; do not edit.';

const names = readdirSync(src).filter(name => /^[A-Z]/.test(name) && statSync(join(src, name)).isDirectory()).sort();
const problems = [];
const theme = await palette();

async function loadDoc(name) {
  const file = join(src, name, `${name}.doc.ts`);
  const { code } = await transform(readFileSync(file, 'utf8'), { loader: 'ts', format: 'esm' });
  return (await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`)).doc;
}

// One program over the package, for props and example sources.
const config = ts.parseJsonConfigFileContent(JSON.parse(readFileSync(join(core, 'tsconfig.json'), 'utf8')), ts.sys, core);
const program = ts.createProgram(config.fileNames, config.options);
const checker = program.getTypeChecker();

// A prop's values when its type is a set of string literals, however the type is named; the
// playground offers them as a choice.
const choiceValues = new Map();
const sizeOrder = ['xs', 'sm', 'md', 'lg', 'xl'];
function literalChoices(type) {
  const members = (type.isUnion() ? type.types : [type]).filter(member => !(member.flags & ts.TypeFlags.Undefined));
  if (members.length < 2 || !members.every(member => member.isStringLiteral())) return null;
  const values = members.map(member => member.value).sort();
  return values.every(value => sizeOrder.includes(value)) ? values.sort((a, b) => sizeOrder.indexOf(a) - sizeOrder.indexOf(b)) : values;
}

/** The exported props type's properties: name, type, required, description, default. */
function props(name, typeName) {
  const component = typeName.replace(/Props$/, '');
  const file = program.getSourceFile(join(src, name, `${name}.tsx`));
  const declaration = file.statements.find(node => (ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) && node.name.text === typeName);
  if (!declaration) { problems.push(`src/${name}/${name}.doc.ts: propsType ${typeName} is not declared in ${name}.tsx`); return []; }
  const type = checker.getTypeAtLocation(declaration.name);
  const defaults = destructuredDefaults(file, component);
  return checker.getPropertiesOfType(type).map(property => {
    const declaration = property.valueDeclaration ?? property.declarations?.[0];
    const propertyType = checker.getTypeOfSymbolAtLocation(property, declaration ?? file);
    const literals = literalChoices(propertyType);
    if (literals) choiceValues.set(`${typeName}.${property.name}`, literals);
    const tags = property.getJsDocTags(checker);
    const tagged = tags.find(tag => tag.name === 'default');
    const fallback = tagged ? ts.displayPartsToString(tagged.text).trim() : undefined;
    const written = defaults.get(property.name);
    if (fallback !== undefined && written !== undefined && fallback !== written)
      problems.push(`src/${name}/${name}.tsx: ${property.name}: @default ${fallback} but the code defaults to ${written}`);
    if (fallback === undefined && written !== undefined && !/^!/.test(written))
      problems.push(`src/${name}/${name}.tsx: ${property.name}: defaults to ${written} without an @default tag`);
    return {
      name: property.name,
      type: typeText(propertyType),
      required: !(property.flags & ts.SymbolFlags.Optional),
      description: ts.displayPartsToString(property.getDocumentationComment(checker)).replace(/\s+/g, ' ').trim(),
      ...(fallback !== undefined ? { default: fallback } : {}),
    };
  });
}

/**
 * A type as the reference prints it. TypeScript orders a union's members by when it first met
 * each, so an unrelated file can reorder them; a union of string literals is printed sorted.
 */
function typeText(type) {
  const text = checker.typeToString(type, undefined, ts.TypeFormatFlags.NoTruncation | ts.TypeFormatFlags.UseAliasDefinedOutsideCurrentScope).replace(/ \| undefined$/, '');
  return /^"[^"|]*"( \| "[^"|]*")+$/.test(text) ? text.split(' | ').sort().join(' | ') : text;
}

/** Defaults written in the component's destructuring: `{ size = 'md' }`. */
function destructuredDefaults(file, name) {
  const found = new Map();
  const component = file.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === name);
  if (!component) return found;
  const visit = node => {
    if (ts.isBindingElement(node) && node.initializer && ts.isObjectBindingPattern(node.parent)) {
      const key = (node.propertyName ?? node.name).getText(file).replace(/^['"]|['"]$/g, '');
      found.set(key, node.initializer.getText(file));
    }
    ts.forEachChild(node, visit);
  };
  visit(component);
  return found;
}

/** Each example's source, with relative imports turned into the package's own paths. */
function exampleSources(name) {
  const path = join(src, name, `${name}.examples.tsx`);
  const file = program.getSourceFile(path);
  const text = file.text;
  const imports = file.statements.filter(ts.isImportDeclaration).map(node => node.getText(file)
    .replace(/from '\.\/(\w+)'/, (_, module) => `from '${packageName}/${module}'`)
    .replace(/from '\.\.\/(\w+)\/\w+'/, (_, module) => `from '${packageName}/${module}'`));
  const sources = new Map();
  const shared = file.statements.filter(node => ts.isVariableStatement(node) || ts.isTypeAliasDeclaration(node)).map(node => node.getText(file));
  for (const statement of file.statements) {
    if (!ts.isFunctionDeclaration(statement) || !statement.name) continue;
    const body = text.slice(statement.getFullStart(), statement.end).trim();
    const used = shared.filter(declaration => {
      const id = /^(?:const|type)\s+(\w+)/.exec(declaration)?.[1];
      return id && new RegExp(`\\b${id}\\b`).test(body);
    });
    const needed = imports.map(line => {
      const specifiers = /\{([^}]*)\}/.exec(line)?.[1].split(',').map(each => each.trim()).filter(Boolean) ?? [];
      const kept = specifiers.filter(each => new RegExp(`\\b${each.replace(/^type\s+/, '').split(/\s+as\s+/).pop()}\\b`).test([body, ...used].join('\n')));
      return kept.length ? line.replace(/\{[^}]*\}/, `{ ${kept.join(', ')} }`) : null;
    }).filter(Boolean);
    sources.set(statement.name.text, [...needed, '', ...used, ...(used.length ? [''] : []), body].join('\n'));
  }
  return sources;
}

// What an app can change about a component without forking it, read from the recipe so it cannot drift.
const scaleTokens = [...readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '..', 'tokens.css'), 'utf8').matchAll(/^\s*--(spacing|text|shadow|radius|animate)-([a-z0-9-]+):/gm)]
  .filter(([, , name]) => !name.includes('--'))
  .map(([, kind, name]) => ({ token: `--${kind}-${name}`, pattern: new RegExp(`(?:^|[\\s:])${{ spacing: '(?:min-h|h|w|size|min-w)', text: 'text', shadow: 'shadow', radius: 'rounded(?:-[a-z]{1,2})?', animate: 'animate' }[kind]}-${name}(?![a-z0-9-])`) }));
const roleNames = Object.keys((await palette()).roles.light).sort((a, b) => b.length - a.length);
const rolePattern = new RegExp(`(?:bg|text|border(?:-[a-z])?|ring|inset-ring|divide|decoration|fill|stroke)-(${roleNames.map(name => name.replace(/-/g, '\\-')).join('|')})(?![a-z-])`, 'g');

const textRolePattern = new RegExp(`(?:^|[\\s:])text-(${roleNames.filter(name => name.startsWith('text-')).map(name => name.slice(5)).join('|')})(?![a-z-])`, 'g');

// The gallery's playground: each component that opts in, its starting props, and a control for
// every prop that is a plain value.
const playgroundChoiceLimit = 12;
function playgroundControl(entry, prop) {
  if (/^(className|data-testid)$|ClassName$|^aria-/.test(prop.name)) return null;
  const base = { name: prop.name, required: prop.required };
  if (prop.type === 'boolean') return { ...base, kind: 'boolean' };
  if (prop.type === 'string') return { ...base, kind: 'text' };
  if (prop.type === 'number') return { ...base, kind: 'number' };
  const options = choiceValues.get(`${entry.name}Props.${prop.name}`);
  return options && options.length <= playgroundChoiceLimit ? { ...base, kind: 'choice', options } : null;
}

function playgroundModule(entries) {
  const playable = entries.filter(entry => entry.playground);
  const data = playable.map(entry => {
    const controls = entry.props.map(prop => playgroundControl(entry, prop)).filter(Boolean);
    for (const [name, value] of Object.entries(entry.playground.props)) {
      const control = controls.find(each => each.name === name);
      const fits = control && (control.kind === 'choice' ? control.options.includes(value) : typeof value === { boolean: 'boolean', text: 'string', number: 'number' }[control.kind]);
      if (!fits) problems.push(`src/${entry.name}/${entry.name}.doc.ts: playground: ${name} is not a plain-value prop of ${entry.name}, or ${JSON.stringify(value)} is not one of its values`);
    }
    for (const control of controls)
      if (control.required && !(control.name in entry.playground.props)) problems.push(`src/${entry.name}/${entry.name}.doc.ts: playground: ${control.name} is required and has no starting value`);
    return { name: entry.name, defaults: entry.playground.props, controls };
  });
  return [
    `// ${generated}`,
    ...playable.map(entry => `import { ${entry.name} } from '${entry.import}';`),
    '',
    "export interface PlaygroundControl { name: string; required: boolean; kind: 'boolean' | 'text' | 'number' | 'choice'; options?: string[] }",
    'export type PlaygroundProps = Record<string, string | number | boolean>;',
    'export interface PlaygroundEntry { name: string; defaults: PlaygroundProps; controls: PlaygroundControl[] }',
    '',
    `export const playgroundEntries: PlaygroundEntry[] = ${JSON.stringify(data, null, 2)};`,
    '',
    '/** The chosen component, drawn from the props in hand. */',
    'export function PlaygroundSubject({ name, props }: { name: string; props: PlaygroundProps }) {',
    ...playable.map(entry => `  if (name === ${JSON.stringify(entry.name)}) return <${entry.name} {...(props as any)} />;`),
    '  return null;',
    '}',
    '',
  ].join('\n');
}

function classesOf(value) {
  return typeof value === 'string' ? [value] : value && typeof value === 'object' ? Object.values(value).flatMap(classesOf) : [];
}

function theming(recipeMap, entry) {
  const classes = classesOf(recipeMap).join(' ');
  const classProps = [...entry.props.map(prop => ({ ...prop, on: '' })), ...Object.entries(entry.otherProps).flatMap(([component, rows]) => rows.map(prop => ({ ...prop, on: component })))]
    .filter(prop => /^class(Name)?$|ClassName$/.test(prop.name))
    .map(prop => ({ name: prop.name, on: prop.on, description: prop.description }));
  return {
    classProps,
    tokens: scaleTokens.filter(each => each.pattern.test(classes)).map(each => each.token),
    // `text-muted` is the role `text-muted` as a text colour, not the role `muted`.
    roles: [...new Set([...[...classes.matchAll(rolePattern)].map(match => match[1]), ...[...classes.matchAll(textRolePattern)].map(match => `text-${match[1]}`)])].sort(),
  };
}

const kebab = name => name.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
const cell = text => String(text).replace(/\|/g, '\\|').replace(/\n/g, ' ');

function reference(doc, entry) {
  const a = doc.accessibility;
  const lines = [
    `<!-- ${generated} -->`, '', `# ${doc.name}`, '', `${doc.summary}. Status: ${doc.status}.`, '',
    '```ts', `import { ${entry.exports.join(', ')} } from '${entry.import}';`, '```', '', doc.description, '',
    '## Examples', '',
  ];
  for (const example of entry.examples) {
    lines.push(`### ${example.title}`, '');
    if (example.description) lines.push(example.description, '');
    lines.push('```tsx', example.source, '```', '');
  }
  const table = (heading, rows) => {
    lines.push(heading, '', '| Prop | Type | Default | Description |', '| --- | --- | --- | --- |');
    for (const prop of rows)
      lines.push(`| \`${prop.name}\`${prop.required ? ' (required)' : ''} | \`${cell(prop.type)}\` | ${prop.default ? `\`${cell(prop.default)}\`` : ''} | ${cell(prop.description)} |`);
  };
  table('## Props', entry.props);
  for (const [component, rows] of Object.entries(entry.otherProps)) {
    lines.push('');
    table(`## ${component} props`, rows);
  }
  lines.push('', '## Anatomy', '', '| Part | Element | Required | Description |', '| --- | --- | --- | --- |');
  for (const part of doc.anatomy) lines.push(`| ${part.part} | \`<${entry.parts[part.part] ?? '?'}>\` | ${part.required ? 'yes' : 'no'} | ${cell(part.description)} |`);
  const t = entry.theming;
  lines.push('', '## What an app may override', '');
  if (t.classProps.length) for (const prop of t.classProps) lines.push(`- \`${prop.name}\`${prop.on ? ` on ${prop.on}` : ''}: ${prop.description || 'classes merged after the recipe\'s, so they win.'}`);
  else lines.push('- No class props: it takes its look from its props, the tokens and the roles below.');
  lines.push(`- The recipe, \`${doc.recipe.map}\` from \`${entry.import}/recipe\`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.`);
  lines.push(t.tokens.length ? `- Scale tokens it reads, set in the app's stylesheet under \`@theme\`: ${t.tokens.map(token => `\`${token}\``).join(', ')}.` : '- It reads no Liuli scale token.');
  lines.push(t.roles.length ? `- Colour roles it paints with, set by the theme pack: ${t.roles.map(role => `\`${role}\``).join(', ')}.` : '- It paints with no colour role of its own.');
  lines.push('', '## Accessibility', '');
  lines.push(`- Pattern: ${a.pattern ?? 'none'}.`, `- Name: ${a.name ?? 'none; its content is read'}.`);
  lines.push(`- Description: ${a.description.length ? a.description.join(', then ') : 'none'}.`);
  if (a.states.length) lines.push(`- States: ${a.states.map(state => `${state.state} (from ${state.from})`).join('; ')}.`);
  for (const key of a.keyboard) lines.push(`- ${key.keys[0].toUpperCase()}${key.keys.slice(1)}: ${key.effect}.`);
  if (a.target) lines.push(`- Press target: ${Object.entries(a.target).map(([size, px]) => `${size} ${px} px`).join(', ')}.`);
  for (const note of a.notes ?? []) lines.push(`- ${note}`);
  lines.push('', '| Contrast | Pair | Minimum | Light | Dark |', '| --- | --- | --- | --- | --- |');
  for (const pair of a.contrast) {
    const cells = appearances.map(appearance => {
      const result = ratio(theme.roles[appearance], pair);
      if (result.missing) return 'needs the resolved palette';
      return result.value >= needed(pair) ? format(result.value) : `${format(result.value)}, below`;
    });
    const roles = `\`${pair.pair[0]}\` on \`${pair.pair[1]}\`${pair.over ? ` over \`${pair.over}\`` : ''}`;
    lines.push(`| ${pair.what} | ${roles} | ${pair.minimum} | ${cells.join(' | ')} |`);
  }
  lines.push('', `Ratios are computed from ${theme.source} and rounded down.`);
  lines.push('', '## Guidance', '', ...doc.guidance.do.map(item => `- Do: ${item}`), ...doc.guidance.dont.map(item => `- Don't: ${item}`));
  if (doc.gaps.length) {
    lines.push('', '## Not yet', '', '| Missing | Needs | Meanwhile |', '| --- | --- | --- |');
    for (const gap of doc.gaps) lines.push(`| ${cell(gap.missing)} | ${cell(gap.needs)} | ${cell(gap.today)} |`);
  }
  if (doc.limits?.length) lines.push('', '## Limits', '', ...doc.limits.map(limit => `- ${limit}`));
  if (doc.related?.length) lines.push('', `Related: ${doc.related.join(', ')}.`);
  return `${lines.join('\n')}\n`;
}

const jsx = text => `{${JSON.stringify(text)}}`;

/** The names a component's gallery shows, in page order, each once. */
function galleryNames(doc) {
  const page = doc.gallery.page;
  if (!page) return doc.examples.map(example => example.name);
  return [...new Set([page.hero, ...(page.parts ? [page.parts.name] : []), ...['variants', 'sizes', 'states', 'inContext'].flatMap(key => (page[key] ?? []).map(ref => ref.name))])];
}

/** A component's gallery content: its examples in page order, without the page around them. */
function galleryContent(doc) {
  const page = doc.gallery.page;
  const lines = [
    `// ${generated}`,
    `import { ${galleryNames(doc).join(', ')} } from '../../${folder}/src/${doc.name}/${doc.name}.examples';`,
    `import { ${page ? 'Lead, ' : ''}PageSection, Stage${page?.states ? ', StageGrid' : ''} } from '../shared/Layout';`,
    '',
    'export function Content() {',
    '  return (',
    '    <>',
  ];
  const bare = page?.stage === 'bare' ? ' bare' : '';
  const stage = (ref, indent) => [`${indent}<Stage caption=${jsx(ref.title)}${bare}>`, `${indent}  <${ref.name} />`, `${indent}</Stage>`];
  if (!page) {
    for (const example of doc.examples)
      lines.push(`      <PageSection title=${jsx(example.title)}>`, '        <Stage>', `          <${example.name} />`, '        </Stage>', '      </PageSection>');
  } else {
    lines.push(`      <Lead text=${jsx(page.lead)}${bare}>`, `        <${page.hero} />`, '      </Lead>');
    if (page.parts)
      lines.push('      <PageSection title="Parts">', `        <Stage caption=${jsx(page.parts.title)} bare>`, `          <${page.parts.name} />`, '        </Stage>', '      </PageSection>');
    const sections = [['variants', 'Variants'], ['sizes', 'Sizes'], ['states', 'States'], ['inContext', 'In context']];
    for (const [key, title] of sections) {
      const refs = page[key];
      if (!refs?.length) continue;
      // A staged state can pass for a fault, so its section says first that it is staged.
      const note = key === 'states' ? page.statesNote ?? 'Each state is set by its props so you can see it. Nothing here is waiting, missing or broken.' : undefined;
      lines.push(`      <PageSection title=${jsx(key === 'states' ? page.statesTitle ?? title : title)}${note ? ` note=${jsx(note)}` : ''}>`);
      if (key === 'states') lines.push(page.statesLayout === 'stack' ? '        <StageGrid stack>' : '        <StageGrid>', ...refs.flatMap(ref => stage(ref, '          ')), '        </StageGrid>');
      else lines.push(...refs.flatMap(ref => stage(ref, '        ')));
      lines.push('      </PageSection>');
    }
  }
  lines.push('    </>', '  );', '}', '');
  return lines.join('\n');
}

/**
 * A phone page: its content in the gallery's scrolling page. The desktop workspace shows the same
 * content beside the sidebar, so a page's body lives in content/, never here.
 */
/** A pushed page's Logic: the gallery's shared page Logic, named so it can hand off to the workspace. */
const pageLogic = page => [
  `// ${generated}`,
  "import { definePage } from '@lingxia/types/page';",
  "import { publish, returnToWorkspaceWhenWide, signIn } from '../../shared/pages';",
  '',
  'export default definePage({',
  '  setup() {',
  '    let unwatch: (() => void) | undefined;',
  '    return {',
  '      onLoad() {',
  `        unwatch = returnToWorkspaceWhenWide(${JSON.stringify(page)});`,
  '      },',
  '      onUnload() {',
  '        unwatch?.();',
  '      },',
  '      actions: {',
  '        async open({ page }: { page: string }) {',
  '          await lx.nav.go({ page }).opened;',
  '        },',
  '        publish,',
  '        signIn,',
  '      },',
  '    };',
  '  },',
  '});',
  '',
].join('\n');

function pageWrapper(page, { opens = false, isFilled = false } = {}) {
  return [
    `// ${generated}`,
    ...(opens ? ["import type { View } from '@lingxia/view';"] : []),
    `import { Content } from '../../content/${page}';`,
    "import { GalleryPage } from '../../shared/Layout';",
    '',
    ...(opens ? ['type Actions = { open(target: { page: string }): Promise<void> };', ''] : []),
    `export function render(${opens ? '{ actions }: View<Actions>' : ''}) {`,
    '  return (',
    isFilled ? '    <GalleryPage isFilled>' : '    <GalleryPage>',
    opens ? '      <Content onOpen={page => actions.open({ page })} />' : '      <Content />',
    '    </GalleryPage>',
    '  );',
    '}',
    '',
  ].join('\n');
}

// The package's own status, shown once on the home page.
const packageStatus = 'preview';
const statusTitles = { preview: 'Preview', stable: 'Stable' };
// A tagline fits one line of a half-width home tile on a 360 pt screen at footnote size, in the
// widest system face (HarmonyOS Sans).
const taglineLimit = 19;

// The gallery's categories, in home order: Foundations and Patterns hold the guide pages, the rest
// hold components by their doc's `category`.
const categoriesPath = join(gallery, 'categories.json');
const categories = existsSync(categoriesPath) ? JSON.parse(readFileSync(categoriesPath, 'utf8')) : [];
// The names a category's icon may take: Liuli's own set.
const iconNames = new Set(Object.keys(JSON.parse(readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../icons/icons.json'), 'utf8')).icons));
const guideCategories = new Set(['foundations', 'patterns']);
const categoryTitles = Object.fromEntries(categories.filter(category => !guideCategories.has(category.id)).map(category => [category.id, category.title]));
const categoryPage = id => `category-${id}`;

// Foundations and Patterns are hand-written gallery pages listed in gallery/guides.json; each has a
// tile specimen in gallery/shared/Specimens.tsx named after its page.
const inCategory = (entries, category) => entries.filter(entry => entry.category === category).sort((a, b) => a.gallery.order - b.gallery.order);
const pascal = page => page.replace(/(^|-)([a-z])/g, (_, __, letter) => letter.toUpperCase());
const guideSpecimen = guide => `${pascal(guide.page)}Specimen`;

/** A tile per member: its specimen, name and line, opening its page. */
function tileLines({ name, tagline, isWide, page, specimen }) {
  return [
    '        <Tile',
    `          name=${jsx(name)}`,
    `          tagline=${jsx(tagline)}`,
    ...(isWide ? ['          isWide'] : []),
    `          onOpen={() => onOpen(${JSON.stringify(page)})}`,
    `          testId=${jsx(`open-${page}`)}`,
    '        >',
    `          <${specimen} />`,
    '        </Tile>',
  ];
}

/** One category's content: its line, then a tile per member. */
function categoryContent(category, tiles, imports) {
  return [
    `// ${generated}`,
    ...imports,
    "import { Category, Tile } from '../shared/Layout';",
    '',
    'export function Content({ onOpen }: { onOpen: (page: string) => void }) {',
    '  return (',
    '    <>',
    `      <text className="text-body text-muted">${'{'}${JSON.stringify(category.lead ?? category.description)}${'}'}</text>`,
    `      <Category title=${jsx(`${tiles.length} ${category.id === 'foundations' ? 'guides' : category.id === 'patterns' ? 'patterns' : tiles.length === 1 ? 'component' : 'components'}`)}>`,
    ...tiles.flatMap((tile, index) => tileLines({ ...tile, isWide: tiles.length % 2 === 1 && index === tiles.length - 1 })),
    '      </Category>',
    '    </>',
    '  );',
    '}',
    '',
  ].join('\n');
}

/** What the home page and its search read: generated, so a new component needs no home edit. */
function indexModule(entries, guides) {
  const component = entry => ({ name: entry.name, page: kebab(entry.name), category: entry.category, summary: entry.summary, keywords: entry.keywords ?? [] });
  const guide = (list, category) => list.map(each => ({ name: each.title, page: each.page, category, summary: each.summary ?? each.tagline, keywords: [] }));
  const all = [...guide(guides.foundations, 'foundations'), ...Object.keys(categoryTitles).flatMap(category => inCategory(entries, category).map(component)), ...guide(guides.patterns, 'patterns')];
  const counted = categories.map(({ lead, ...category }) => ({ ...category, page: categoryPage(category.id), count: all.filter(entry => entry.category === category.id).length })).filter(category => category.count > 0);
  return [
    `// ${generated}`,
    '',
    "import type { IconName } from '@liulidesign/core/Icon';",
    '',
    'export interface GalleryEntry { name: string; page: string; category: string; summary: string; keywords: string[] }',
    'export interface GalleryCategory { id: string; title: string; icon: IconName; description: string; page: string; count: number }',
    '',
    `export const entries: GalleryEntry[] = ${JSON.stringify(all, null, 2)};`,
    '',
    `export const galleryCategories: GalleryCategory[] = ${JSON.stringify(counted, null, 2)};`,
    '',
    `export const version = ${JSON.stringify(JSON.parse(readFileSync(join(core, 'package.json'), 'utf8')).version)};`,
    `export const packageStatus = ${JSON.stringify(statusTitles[packageStatus])};`,
    '',
  ].join('\n');
}

/**
 * Every page's content by page name, for the desktop workspace, which shows them in one page. It
 * imports every example, so only the workspace page imports it.
 */
function contentsModule(entries, guides) {
  const pages = [
    ...categories.filter(category => (guideCategories.has(category.id) ? guides[category.id] : inCategory(entries, category.id)).length).map(category => categoryPage(category.id)),
    ...guides.foundations.map(guide => guide.page),
    ...Object.keys(categoryTitles).flatMap(category => inCategory(entries, category).map(entry => kebab(entry.name))),
    ...guides.patterns.map(guide => guide.page),
  ];
  return [
    `// ${generated}`,
    "import type { Component } from '@lingxia/view';",
    ...pages.map(page => `import { Content as ${pascal(page)} } from '../content/${page}';`),
    '',
    '/** Category contents open a page through `onOpen`; the others ignore it. */',
    'export type GalleryContent = Component<{ onOpen: (page: string) => void }>;',
    '',
    'export const contents: Record<string, GalleryContent> = {',
    ...pages.map(page => `  ${JSON.stringify(page)}: ${pascal(page)},`),
    '};',
    '',
  ].join('\n');
}

const outputs = new Map();
const catalog = [];
for (const name of names) {
  const doc = await loadDoc(name);
  // The doc files are loaded as plain modules here, so their shape is checked here as well as by tsc.
  const docPath = `src/${name}/${name}.doc.ts`;
  for (const state of doc.accessibility.states)
    if (typeof state !== 'object' || !state.state || !state.from) problems.push(`${docPath}: each state names the state and the prop it comes from`);
  for (const gap of doc.gaps)
    if (typeof gap !== 'object' || !gap.missing || !gap.needs || !gap.today) problems.push(`${docPath}: a gap says what is missing, what it needs and what ships today; a plain limitation goes in limits`);
  for (const limit of doc.limits ?? [])
    if (typeof limit !== 'string' || !limit.trim()) problems.push(`${docPath}: a limit is one sentence`);
  const recipe = await import(`data:text/javascript;base64,${Buffer.from((await transform(readFileSync(join(src, name, `${name}.recipe.ts`), 'utf8'), { loader: 'ts', format: 'esm' })).code).toString('base64')}`);
  const parts = recipe[doc.recipe.parts] ?? {};
  const component = program.getSourceFile(join(src, name, `${name}.tsx`));
  const exports = component.statements.filter(node => ts.isFunctionDeclaration(node) && node.name && node.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword)).map(node => node.name.text);
  const sources = exampleSources(name);
  const entry = {
    name: doc.name,
    import: `${packageName}/${name}`,
    exports,
    category: doc.category,
    status: doc.status,
    since: doc.since,
    summary: doc.summary,
    description: doc.description,
    keywords: doc.keywords,
    props: props(name, doc.propsType),
    otherProps: Object.fromEntries((doc.otherPropsTypes ?? []).map(type => [type.replace(/Props$/, ''), props(name, type)])),
    parts,
    accessibility: doc.accessibility,
    examples: doc.examples.map(example => ({ ...example, source: sources.get(example.name) ?? '' })),
    gaps: doc.gaps,
    limits: doc.limits,
    playground: doc.playground,
    theming: undefined,
    related: doc.related ?? [],
    gallery: doc.gallery,
  };
  if (!(doc.category in categoryTitles)) problems.push(`src/${name}/${name}.doc.ts: category ${doc.category} is not in gallery/categories.json`);
  const named = doc.accessibility.name;
  if (named !== null && ![entry.props, ...Object.values(entry.otherProps)].flat().some(prop => prop.name === named)) problems.push(`${docPath}: accessibility.name is ${JSON.stringify(named)}, which is not a prop of ${doc.propsType} or of the components beside it`);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(doc.since ?? '')) problems.push(`src/${name}/${name}.doc.ts: since must be YYYY-MM-DD`);
  const tagline = doc.gallery.tagline ?? '';
  if (!tagline || tagline.length > taglineLimit || tagline.endsWith('.')) problems.push(`src/${name}/${name}.doc.ts: gallery.tagline must be 1 to ${taglineLimit} characters without a trailing period`);
  if (!Number.isInteger(doc.gallery.order) || doc.gallery.order < 1) problems.push(`src/${name}/${name}.doc.ts: gallery.order must be a whole number from 1`);
  for (const shown of [doc.gallery.specimen, ...galleryNames(doc)])
    if (!sources.has(shown)) problems.push(`src/${name}/${name}.doc.ts: gallery shows ${shown}, which ${name}.examples.tsx does not export`);
  for (const example of entry.examples) if (!example.source) problems.push(`src/${name}/${name}.doc.ts: example ${example.name} has no source`);
  entry.theming = theming(recipe[doc.recipe.map] ?? {}, entry);
  catalog.push(entry);
  outputs.set(join(core, 'docs', `${name}.md`), reference(doc, entry));
  const page = join(gallery, 'pages', kebab(name));
  outputs.set(join(gallery, 'content', `${kebab(name)}.tsx`), galleryContent(doc));
  outputs.set(join(page, 'index.tsx'), pageWrapper(kebab(name)));
  outputs.set(join(page, 'index.ts'), pageLogic(kebab(name)));
  outputs.set(join(page, 'index.json'), `{ "navigationBar": { "title": ${JSON.stringify(name)} } }\n`);
}
outputs.set(join(core, 'catalog.json'), `${JSON.stringify({ generated, components: catalog }, null, 2)}\n`);
// The package's root entry: every component module, so `@liulidesign/core` gives what each subpath gives.
if (isCore) {
  const modules = [...catalog].sort((x, y) => x.name.localeCompare(y.name)).map(entry => `export * from './${entry.name}/${entry.name}';`);
  outputs.set(join(core, 'src', 'index.ts'), `// Generated by scripts/generate.mjs from the catalog; do not edit.\n${modules.join('\n')}\n`);
}

// The README's component list: how many there are and what each import gives, between two markers.
const readmePath = join(core, 'README.md');
if (existsSync(readmePath)) {
  const readme = readFileSync(readmePath, 'utf8');
  const [start, end] = ['<!-- components:start -->', '<!-- components:end -->'];
  const [from, to] = [readme.indexOf(start), readme.indexOf(end)];
  if (from < 0 || to < from) problems.push('README.md: the component list needs its <!-- components:start --> and <!-- components:end --> markers');
  else {
    const inOrder = Object.keys(categoryTitles).flatMap(category => inCategory(catalog, category));
    const listed = [...inOrder, ...catalog.filter(entry => !inOrder.includes(entry))];
    const rows = listed.map(entry => `| \`${entry.import}\` | ${entry.exports.join(', ')} | ${cell(entry.summary)} |`);
    const list = [
      start,
      isCore
        ? `${catalog.length} components. Import any of them from \`${packageName}\`, or from its own path; a second component in a row (\`AvatarGroup\`, \`Spinner\`) has a path of its own too:`
        : `${catalog.length} components, each at its own import path:`,
      '',
      '| Import | Exports | What it is |',
      '| --- | --- | --- |',
      ...rows,
      '',
    ].join('\n');
    outputs.set(readmePath, `${readme.slice(0, from)}${list}${readme.slice(to)}`);
  }
}
// The gallery's index holds every package's components, so core's run reads its siblings' catalogs.
const folderOf = new Map(catalog.map(entry => [entry, folder]));
const siblings = !isCore ? [] : readdirSync(resolve(core, '..')).filter(name => name !== folder && existsSync(resolve(core, '..', name, 'catalog.json'))).sort()
  .flatMap(name => JSON.parse(readFileSync(resolve(core, '..', name, 'catalog.json'), 'utf8')).components.map(entry => (folderOf.set(entry, name), entry)));
for (const entry of siblings) if (!(entry.category in categoryTitles)) problems.push(`${folderOf.get(entry)}/catalog.json: ${entry.name}: category ${entry.category} is not in gallery/categories.json`);
const shown = [...catalog, ...siblings];
for (const category of Object.keys(categoryTitles)) {
  const orders = inCategory(shown, category).map(entry => entry.gallery.order);
  if (new Set(orders).size !== orders.length) problems.push(`gallery.order repeats within ${category}: ${orders.join(', ')}`);
}
if (isCore) {
  // A gallery without guide pages has neither file.
  const guidesPath = join(gallery, 'guides.json');
  const guides = existsSync(guidesPath) ? JSON.parse(readFileSync(guidesPath, 'utf8')) : { foundations: [], patterns: [] };
  const specimensPath = join(gallery, 'shared', 'Specimens.tsx');
  const specimens = existsSync(specimensPath) ? readFileSync(specimensPath, 'utf8') : '';
  for (const guide of [...guides.foundations, ...guides.patterns]) {
    const where = `gallery/guides.json: ${guide.page}`;
    if (!guide.tagline || guide.tagline.length > taglineLimit || guide.tagline.endsWith('.')) problems.push(`${where}: tagline must be 1 to ${taglineLimit} characters without a trailing period`);
    if (!existsSync(join(gallery, 'pages', guide.page, 'index.json'))) problems.push(`${where}: pages/${guide.page}/index.json is missing`);
    const content = join(gallery, 'content', `${guide.page}.tsx`);
    if (!existsSync(content) || !/export function Content\b/.test(readFileSync(content, 'utf8'))) problems.push(`${where}: content/${guide.page}.tsx does not export Content`);
    outputs.set(join(gallery, 'pages', guide.page, 'index.tsx'), pageWrapper(guide.page, { isFilled: guide.layout === 'fill' }));
    outputs.set(join(gallery, 'pages', guide.page, 'index.ts'), pageLogic(guide.page));
    if (!new RegExp(`export function ${guideSpecimen(guide)}\\b`).test(specimens)) problems.push(`${where}: shared/Specimens.tsx does not export ${guideSpecimen(guide)}`);
  }
  outputs.set(join(gallery, 'shared', 'playground.generated.tsx'), playgroundModule(shown));
  outputs.set(join(gallery, 'shared', 'catalog.generated.ts'), indexModule(shown, guides));
  outputs.set(join(gallery, 'shared', 'contents.generated.ts'), contentsModule(shown, guides));
  for (const category of categories) {
    const isGuide = guideCategories.has(category.id);
    const members = isGuide ? guides[category.id] : inCategory(shown, category.id);
    if (!members.length) continue;
    if (!iconNames.has(category.icon)) problems.push(`gallery/categories.json: ${category.id}: icon ${JSON.stringify(category.icon)} is not in Liuli's icon set`);
    const tiles = isGuide
      ? members.map(guide => ({ name: guide.title, tagline: guide.tagline, page: guide.page, specimen: guideSpecimen(guide) }))
      : members.map(entry => ({ name: entry.name, tagline: entry.gallery.tagline, page: kebab(entry.name), specimen: `${entry.name}Specimen` }));
    const imports = isGuide
      ? [`import { ${members.map(guideSpecimen).join(', ')} } from '../shared/Specimens';`]
      : members.map(entry => `import { ${entry.gallery.specimen} as ${entry.name}Specimen } from '../../${folderOf.get(entry)}/src/${entry.name}/${entry.name}.examples';`);
    const dir = join(gallery, 'pages', categoryPage(category.id));
    outputs.set(join(gallery, 'content', `${categoryPage(category.id)}.tsx`), categoryContent(category, tiles, imports));
    outputs.set(join(dir, 'index.tsx'), pageWrapper(categoryPage(category.id), { opens: true }));
    outputs.set(join(dir, 'index.json'), `{ "navigationBar": { "title": ${JSON.stringify(category.title)} } }\n`);
    outputs.set(join(dir, 'index.ts'), pageLogic(categoryPage(category.id)));
  }
  for (const id of Object.keys(categoryTitles)) if (!categories.some(category => category.id === id)) problems.push(`gallery/categories.json lacks ${id}`);
  const manifestPath = join(gallery, 'lxapp.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  // Home order, so the manifest reads like the gallery.
  const pageEntry = page => ({ name: page, path: `pages/${page}/index` });
  const shownCategories = categories.filter(category => (guideCategories.has(category.id) ? guides[category.id] : inCategory(shown, category.id)).length);
  manifest.pages = [pageEntry('home'), ...shownCategories.map(category => pageEntry(categoryPage(category.id))), ...guides.foundations.map(guide => pageEntry(guide.page)), ...Object.keys(categoryTitles).flatMap(category => inCategory(shown, category).map(entry => pageEntry(kebab(entry.name)))), ...guides.patterns.map(guide => pageEntry(guide.page))];
  // One page per line, as the hand-written manifest had it.
  outputs.set(manifestPath, `${JSON.stringify(manifest, null, 2).replace(/\{\n\s+"name": ("[^"]+"),\n\s+"path": ("[^"]+")\n\s+\}/g, '{ "name": $1, "path": $2 }')}\n`);
}

if (problems.length) {
  for (const problem of problems) console.log(problem);
  process.exit(1);
}
const stale = [];
for (const [path, contents] of outputs) {
  const current = existsSync(path) ? readFileSync(path, 'utf8') : null;
  if (current === contents) continue;
  stale.push(relative(resolve(core, '..'), path));
  if (!check) { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, contents); }
}
if (check && stale.length) {
  console.log(`out of date; run node scripts/generate.mjs:\n${stale.map(path => `  ${path}`).join('\n')}`);
  process.exit(1);
}
console.log(check ? 'liuli generate: up to date' : stale.length ? `wrote ${stale.length} file${stale.length === 1 ? '' : 's'}` : 'liuli generate: up to date');
