// Writes everything derived from the doc files: the reference pages (docs/<Name>.md), catalog.json
// for agents, and the gallery's component pages. `--check` writes nothing and fails on drift.
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { transform } from 'esbuild';
import ts from 'typescript';

const core = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const src = join(core, 'src');
const gallery = resolve(core, '../gallery');
const check = process.argv.includes('--check');
const generated = 'Generated from the doc file by scripts/generate.mjs; do not edit.';

const names = readdirSync(src).filter(name => /^[A-Z]/.test(name) && statSync(join(src, name)).isDirectory()).sort();
const problems = [];

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
  const file = program.getSourceFile(join(src, name, `${name}.tsx`));
  const declaration = file.statements.find(node => (ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node)) && node.name.text === typeName);
  if (!declaration) { problems.push(`src/${name}/${name}.doc.ts: propsType ${typeName} is not declared in ${name}.tsx`); return []; }
  const type = checker.getTypeAtLocation(declaration.name);
  const defaults = destructuredDefaults(file, name);
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
  lines.push('## Props', '', '| Prop | Type | Default | Description |', '| --- | --- | --- | --- |');
  for (const prop of entry.props)
    lines.push(`| \`${prop.name}\`${prop.required ? ' (required)' : ''} | \`${cell(prop.type)}\` | ${prop.default ? `\`${cell(prop.default)}\`` : ''} | ${cell(prop.description)} |`);
  lines.push('', '## Anatomy', '', '| Part | Element | Required | Description |', '| --- | --- | --- | --- |');
  for (const part of doc.anatomy) lines.push(`| ${part.part} | \`<${entry.parts[part.part] ?? '?'}>\` | ${part.required ? 'yes' : 'no'} | ${cell(part.description)} |`);
  lines.push('', '## Accessibility', '');
  lines.push(`- Pattern: ${a.pattern ?? 'none (holds a control)'}.`, `- Name: ${a.name}.`);
  lines.push(`- Description: ${a.description.length ? a.description.join(', then ') : 'none'}.`);
  if (a.states.length) lines.push(`- States: ${a.states.map(state => `${state.state} (from ${state.from})`).join('; ')}.`);
  for (const key of a.keyboard) lines.push(`- ${key.keys[0].toUpperCase()}${key.keys.slice(1)}: ${key.effect}.`);
  if (a.target) lines.push(`- Press target: ${Object.entries(a.target).map(([size, px]) => `${size} ${px} px`).join(', ')}.`);
  for (const note of a.notes ?? []) lines.push(`- ${note}`);
  lines.push('', '| Contrast | Pair | Minimum | Proven by the theme pack |', '| --- | --- | --- | --- |');
  for (const pair of a.contrast) lines.push(`| ${pair.what} | \`${pair.pair[0]}\` on \`${pair.pair[1]}\` | ${pair.minimum} | ${pair.proven ? 'yes' : 'no'} |`);
  lines.push('', '## Guidance', '', ...doc.guidance.do.map(item => `- Do: ${item}`), ...doc.guidance.dont.map(item => `- Don't: ${item}`));
  if (doc.gaps.length) {
    lines.push('', '## Not yet', '', '| Missing | Needs | Meanwhile |', '| --- | --- | --- |');
    for (const gap of doc.gaps) lines.push(`| ${cell(gap.missing)} | ${cell(gap.needs)} | ${cell(gap.today)} |`);
  }
  if (doc.related?.length) lines.push('', `Related: ${doc.related.join(', ')}.`);
  return `${lines.join('\n')}\n`;
}

function galleryPage(doc) {
  const examples = doc.examples.map(example => example.name);
  return [
    `// ${generated}`,
    `import { ${examples.join(', ')} } from '../../../core/src/${doc.name}/${doc.name}.examples';`,
    "import { GalleryPage, Section } from '../../shared/Section';",
    '',
    'export function render() {',
    '  return (',
    `    <GalleryPage>`,
    ...doc.examples.flatMap(example => [`      <Section title={${JSON.stringify(example.title)}}>`, `        <${example.name} />`, '      </Section>']),
    '    </GalleryPage>',
    '  );',
    '}',
    '',
  ].join('\n');
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
    parts,
    accessibility: doc.accessibility,
    examples: doc.examples.map(example => ({ ...example, source: sources.get(example.name) ?? '' })),
    gaps: doc.gaps,
    related: doc.related ?? [],
  };
  for (const example of entry.examples) if (!example.source) problems.push(`src/${name}/${name}.doc.ts: example ${example.name} has no source`);
  catalog.push(entry);
  outputs.set(join(core, 'docs', `${name}.md`), reference(doc, entry));
  const page = join(gallery, 'pages', kebab(name));
  outputs.set(join(page, 'index.tsx'), galleryPage(doc));
  outputs.set(join(page, 'index.json'), `{ "navigationBar": { "title": ${JSON.stringify(name)} } }\n`);
}
outputs.set(join(core, 'catalog.json'), `${JSON.stringify({ generated, components: catalog }, null, 2)}\n`);
outputs.set(join(gallery, 'shared', 'components.ts'), [
  `// ${generated}`,
  'export const components = [',
  ...catalog.map(entry => `  { page: ${JSON.stringify(kebab(entry.name))}, name: ${JSON.stringify(entry.name)}, summary: ${JSON.stringify(entry.summary)} },`),
  '] as const;',
  '',
].join('\n'));
const manifestPath = join(gallery, 'lxapp.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
manifest.pages = [{ name: 'home', path: 'pages/home/index' }, ...catalog.map(entry => ({ name: kebab(entry.name), path: `pages/${kebab(entry.name)}/index` }))];
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
