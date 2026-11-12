// Writes everything derived from the doc files: the reference pages (docs/<Name>.md), catalog.json
// for agents, and the gallery's component pages. `--check` writes nothing and fails on drift.
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { transform } from 'esbuild';
import ts from 'typescript';
import { appearances, format, needed, palette, ratio } from './contrast.mjs';

const core = resolve(dirname(fileURLToPath(import.meta.url)), '..');
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
      type: checker.typeToString(propertyType, undefined, ts.TypeFormatFlags.NoTruncation | ts.TypeFormatFlags.UseAliasDefinedOutsideCurrentScope).replace(/ \| undefined$/, ''),
      required: !(property.flags & ts.SymbolFlags.Optional),
      description: ts.displayPartsToString(property.getDocumentationComment(checker)).replace(/\s+/g, ' ').trim(),
      ...(fallback !== undefined ? { default: fallback } : {}),
    };
  });
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
    .replace(/from '\.\/(\w+)'/, (_, module) => `from '@liulidesign/core/${module}'`)
    .replace(/from '\.\.\/(\w+)\/\w+'/, (_, module) => `from '@liulidesign/core/${module}'`));
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

function galleryPage(doc) {
  const page = doc.gallery.page;
  const lines = [
    `// ${generated}`,
    `import { ${galleryNames(doc).join(', ')} } from '../../../core/src/${doc.name}/${doc.name}.examples';`,
    `import { GalleryPage, ${page ? 'Lead, ' : ''}PageSection, Stage${page?.states ? ', StageGrid' : ''} } from '../../shared/Layout';`,
    '',
    'export function render() {',
    '  return (',
    '    <GalleryPage>',
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
      lines.push(`      <PageSection title=${jsx(title)}>`);
      if (key === 'states') lines.push('        <StageGrid>', ...refs.flatMap(ref => stage(ref, '          ')), '        </StageGrid>');
      else lines.push(...refs.flatMap(ref => stage(ref, '        ')));
      lines.push('      </PageSection>');
    }
  }
  lines.push('    </GalleryPage>', '  );', '}', '');
  return lines.join('\n');
}

// The package's own status, shown once on the home page; a tile shows only a status that differs.
const packageStatus = 'preview';
const statusTitles = { preview: 'Preview', stable: 'Stable' };
// A tagline fits one line of a half-width home tile on a 360 pt screen at footnote size.
const taglineLimit = 22;

const categoryTitles = { actions: 'Actions', navigation: 'Navigation', inputs: 'Inputs', feedback: 'Feedback', overlays: 'Overlays', content: 'Content' };

// Foundations and Patterns are hand-written gallery pages listed in gallery/guides.json; each has a
// tile specimen in gallery/shared/Specimens.tsx named after its page.
const inCategory = (entries, category) => entries.filter(entry => entry.category === category).sort((a, b) => a.gallery.order - b.gallery.order);
const pascal = page => page.replace(/(^|-)([a-z])/g, (_, __, letter) => letter.toUpperCase());
const guideSpecimen = guide => `${pascal(guide.page)}Specimen`;

function homePage(entries, guides, version) {
  const tile = ({ name, tagline, status, isWide, page, specimen }) => [
    '        <Tile',
    `          name=${jsx(name)}`,
    `          tagline=${jsx(tagline)}`,
    ...(status ? [`          status=${jsx(status)}`] : []),
    ...(isWide ? ['          isWide'] : []),
    `          onOpen={() => actions.open({ page: ${JSON.stringify(page)} })}`,
    `          testId=${jsx(`open-${page}`)}`,
    '        >',
    `          <${specimen} />`,
    '        </Tile>',
  ];
  const guideSection = (title, list) => !list.length ? [] : [
    `      <Category title=${jsx(title)}>`,
    ...list.flatMap((guide, index) => tile({ name: guide.title, tagline: guide.tagline, isWide: list.length % 2 === 1 && index === list.length - 1, page: guide.page, specimen: guideSpecimen(guide) })),
    '      </Category>',
  ];
  const lines = [
    `// ${generated}`,
    "import type { View } from '@lingxia/view';",
    ...entries.map(entry => `import { ${entry.gallery.specimen} as ${entry.name}Specimen } from '../../../core/src/${entry.name}/${entry.name}.examples';`),
    "import { Category, GalleryPage, Masthead, Tile } from '../../shared/Layout';",
    ...(guides.foundations.length + guides.patterns.length ? [`import { ${[...guides.foundations, ...guides.patterns].map(guideSpecimen).join(', ')} } from '../../shared/Specimens';`] : []),
    '',
    'type Actions = { open(target: { page: string }): Promise<void> };',
    '',
    'export function render({ actions }: View<Actions>) {',
    '  return (',
    '    <GalleryPage>',
    `      <Masthead version=${jsx(version)} status=${jsx(statusTitles[packageStatus])} />`,
    ...guideSection('Foundations', guides.foundations),
  ];
  for (const [category, title] of Object.entries(categoryTitles)) {
    const members = inCategory(entries, category);
    if (!members.length) continue;
    lines.push(`      <Category title=${jsx(title)}>`);
    for (const [index, entry] of members.entries())
      lines.push(...tile({
        name: entry.name,
        tagline: entry.gallery.tagline,
        status: entry.status === packageStatus ? undefined : statusTitles[entry.status],
        isWide: members.length % 2 === 1 && index === members.length - 1,
        page: kebab(entry.name),
        specimen: `${entry.name}Specimen`,
      }));
    lines.push('      </Category>');
  }
  lines.push(...guideSection('Patterns', guides.patterns), '    </GalleryPage>', '  );', '}', '');
  return lines.join('\n');
}

const outputs = new Map();
const catalog = [];
for (const name of names) {
  const doc = await loadDoc(name);
  const recipe = await import(`data:text/javascript;base64,${Buffer.from((await transform(readFileSync(join(src, name, `${name}.recipe.ts`), 'utf8'), { loader: 'ts', format: 'esm' })).code).toString('base64')}`);
  const parts = recipe[doc.recipe.parts] ?? {};
  const component = program.getSourceFile(join(src, name, `${name}.tsx`));
  const exports = component.statements.filter(node => ts.isFunctionDeclaration(node) && node.name && node.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword)).map(node => node.name.text);
  const sources = exampleSources(name);
  const entry = {
    name: doc.name,
    import: `@liulidesign/core/${name}`,
    exports,
    category: doc.category,
    status: doc.status,
    summary: doc.summary,
    description: doc.description,
    keywords: doc.keywords,
    props: props(name, doc.propsType),
    otherProps: Object.fromEntries((doc.otherPropsTypes ?? []).map(type => [type.replace(/Props$/, ''), props(name, type)])),
    parts,
    accessibility: doc.accessibility,
    examples: doc.examples.map(example => ({ ...example, source: sources.get(example.name) ?? '' })),
    gaps: doc.gaps,
    related: doc.related ?? [],
    gallery: doc.gallery,
  };
  if (!(doc.category in categoryTitles)) problems.push(`src/${name}/${name}.doc.ts: category ${doc.category} is not a gallery category`);
  const tagline = doc.gallery.tagline ?? '';
  if (!tagline || tagline.length > taglineLimit || tagline.endsWith('.')) problems.push(`src/${name}/${name}.doc.ts: gallery.tagline must be 1 to ${taglineLimit} characters without a trailing period`);
  if (!Number.isInteger(doc.gallery.order) || doc.gallery.order < 1) problems.push(`src/${name}/${name}.doc.ts: gallery.order must be a whole number from 1`);
  for (const shown of [doc.gallery.specimen, ...galleryNames(doc)])
    if (!sources.has(shown)) problems.push(`src/${name}/${name}.doc.ts: gallery shows ${shown}, which ${name}.examples.tsx does not export`);
  for (const example of entry.examples) if (!example.source) problems.push(`src/${name}/${name}.doc.ts: example ${example.name} has no source`);
  catalog.push(entry);
  outputs.set(join(core, 'docs', `${name}.md`), reference(doc, entry));
  const page = join(gallery, 'pages', kebab(name));
  outputs.set(join(page, 'index.tsx'), galleryPage(doc));
  outputs.set(join(page, 'index.json'), `{ "navigationBar": { "title": ${JSON.stringify(name)} } }\n`);
}
for (const category of Object.keys(categoryTitles)) {
  const orders = inCategory(catalog, category).map(entry => entry.gallery.order);
  if (new Set(orders).size !== orders.length) problems.push(`gallery.order repeats within ${category}: ${orders.join(', ')}`);
}
outputs.set(join(core, 'catalog.json'), `${JSON.stringify({ generated, components: catalog }, null, 2)}\n`);
const version = JSON.parse(readFileSync(join(core, 'package.json'), 'utf8')).version;
// A gallery without guide pages has neither file.
const guidesPath = join(gallery, 'guides.json');
const guides = existsSync(guidesPath) ? JSON.parse(readFileSync(guidesPath, 'utf8')) : { foundations: [], patterns: [] };
const specimensPath = join(gallery, 'shared', 'Specimens.tsx');
const specimens = existsSync(specimensPath) ? readFileSync(specimensPath, 'utf8') : '';
for (const guide of [...guides.foundations, ...guides.patterns]) {
  const where = `gallery/guides.json: ${guide.page}`;
  if (!guide.tagline || guide.tagline.length > taglineLimit || guide.tagline.endsWith('.')) problems.push(`${where}: tagline must be 1 to ${taglineLimit} characters without a trailing period`);
  for (const file of ['index.tsx', 'index.ts', 'index.json'])
    if (!existsSync(join(gallery, 'pages', guide.page, file))) problems.push(`${where}: pages/${guide.page}/${file} is missing`);
  if (!new RegExp(`export function ${guideSpecimen(guide)}\\b`).test(specimens)) problems.push(`${where}: shared/Specimens.tsx does not export ${guideSpecimen(guide)}`);
}
outputs.set(join(gallery, 'pages', 'home', 'index.tsx'), homePage(catalog, guides, version));
const manifestPath = join(gallery, 'lxapp.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
// Home order, so the manifest reads like the gallery.
const pageEntry = page => ({ name: page, path: `pages/${page}/index` });
manifest.pages = [pageEntry('home'), ...guides.foundations.map(guide => pageEntry(guide.page)), ...Object.keys(categoryTitles).flatMap(category => inCategory(catalog, category).map(entry => pageEntry(kebab(entry.name)))), ...guides.patterns.map(guide => pageEntry(guide.page))];
// One page per line, as the hand-written manifest had it.
outputs.set(manifestPath, `${JSON.stringify(manifest, null, 2).replace(/\{\n\s+"name": ("[^"]+"),\n\s+"path": ("[^"]+")\n\s+\}/g, '{ "name": $1, "path": $2 }')}\n`);

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
// A page's Logic is hand-written; a component without one gets an empty Page.
for (const entry of catalog) {
  const logic = join(gallery, 'pages', kebab(entry.name), 'index.ts');
  if (existsSync(logic)) continue;
  stale.push(relative(resolve(core, '..'), logic));
  if (!check) writeFileSync(logic, 'Page({});\n');
}
if (check && stale.length) {
  console.log(`out of date; run node scripts/generate.mjs:\n${stale.map(path => `  ${path}`).join('\n')}`);
  process.exit(1);
}
console.log(check ? 'liuli generate: up to date' : stale.length ? `wrote ${stale.length} file${stale.length === 1 ? '' : 's'}` : 'liuli generate: up to date');
