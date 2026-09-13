// Build-time native-components reference generator.
//
// Reads the published `@lingxia/elements` package type definitions and emits a
// Starlight markdown page per `Lx<Name>Attributes` interface into
// src/content/docs/reference/components/. Run before `astro build`/`astro dev`
// (wired in package.json). Output is a build artifact (gitignored) so the docs
// always track the pinned package version — never hand-edited.

import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { existsSync, mkdirSync, readdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { Project } from 'ts-morph';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, '..');
const require = createRequire(import.meta.url);

function resolveElementsEntry() {
  const sibling = join(projectRoot, '../packages/lingxia-elements/src/index.ts');
  if (existsSync(sibling)) return sibling;
  const elementsEntry = require.resolve('@lingxia/elements');
  return elementsEntry.replace(/\.js$/, '.d.ts');
}

const elementsDts = resolveElementsEntry();

const outDir = join(projectRoot, 'src/content/docs/reference/components');

const project = new Project({
  compilerOptions: { allowJs: true, skipLibCheck: true },
});
project.addSourceFileAtPath(elementsDts);
// ts-morph pulls in the referenced per-component d.ts files transitively.
project.resolveSourceFileDependencies();

const checker = project.getTypeChecker();

/** Kebab-case a camelCase identifier (objectFit -> object-fit). */
function kebab(name) {
  if (name.includes('-')) return name; // already kebab (e.g. open-type)
  return name.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
}

/** Render a TS type string as an inline code span safe for a table cell. */
function mdType(text) {
  // Pipes (union types) must be escaped or they split the table column.
  return '`' + text.replace(/\s+/g, ' ').trim().replace(/\|/g, '\\|') + '`';
}

function escapeCell(text) {
  return (text || '').replace(/\r?\n/g, ' ').replace(/\|/g, '\\|').trim();
}

/** First JSDoc description for a property symbol, or ''. */
function describe(prop) {
  const parts = prop.compilerSymbol.getDocumentationComment(checker.compilerObject);
  if (!parts || parts.length === 0) return '';
  return parts.map((p) => p.text).join(' ').trim();
}

// Friendly title + ordering per known component; unknown ones sort last.
const META = {
  LxVideoAttributes: { title: 'Video', tag: 'lx-video', order: 1, summary: 'Native video player. Must be a direct child of LxNativeRoot.' },
  LxMediaSwiperAttributes: { title: 'Media Swiper', tag: 'lx-media-swiper', order: 6, summary: 'Paged image/video carousel.' },
  LxPickerAttributes: { title: 'Picker', tag: 'lx-picker', order: 7, summary: 'Native selector / date / time picker.' },
  LxNavigatorAttributes: { title: 'Navigator', tag: 'lx-navigator', order: 8, summary: 'Declarative navigation element.' },
};

// Island nodes have no `Lx*Attributes` export. Keep a curated page so the
// generated reference cannot silently omit the tree that LxVideo lives in.
const ISLAND = [
  {
    slug: 'native-root',
    title: 'Native Root',
    tag: 'lx-native-root',
    order: 2,
    summary: 'Island root. Every LxVideo must be a direct child of an explicit Root.',
    body: `There is no implicit root. Nested Root, DOM inside Root, a bare \`<lx-video>\`, or Video inside View/Cover is \`NATIVE_ROOT_INVALID_STRUCTURE\`.

React/Vue: import \`LxNativeRoot\` from \`@lingxia/react\` or \`@lingxia/vue\`. HTML: \`<lx-native-root>\` after \`registerInlineNativeComponents()\`.
`,
  },
  {
    slug: 'native-cover',
    title: 'Native Cover',
    tag: 'lx-native-cover',
    order: 3,
    summary: 'Absolutely positioned overlay inside an island (scrim, chrome).',
    body: `Sits inside \`LxNativeRoot\`. Cannot wrap \`LxVideo\`. Use \`scrim\` / \`scrim-opacity\` for a dimmed overlay. Wrap copy in \`LxNativeText\`.
`,
  },
  {
    slug: 'native-view',
    title: 'Native View',
    tag: 'lx-native-view',
    order: 4,
    summary: 'Layout box inside an island.',
    body: `Sits inside \`LxNativeRoot\`. Cannot wrap \`LxVideo\`. Wrap copy in \`LxNativeText\`; do not put bare text or DOM children in the island.
`,
  },
  {
    slug: 'native-text',
    title: 'Native Text',
    tag: 'lx-native-text',
    order: 5,
    summary: 'Native label inside an island.',
    body: `The only way to put copy on the island. Attributes include \`max-lines\`, \`font-size\`, \`font-weight\`, \`line-height\`, \`text-align\`, and \`color\`.
`,
  },
  {
    slug: 'native-button',
    title: 'Native Button',
    tag: 'lx-native-button',
    order: 5.5,
    summary: 'Native tappable on the island. React/Vue \`onPress\` is payload-first.',
    body: `Cannot wrap interactive or native children. Use \`label\` / \`icon\`; an icon-only button needs a non-empty \`aria-label\`. Seek stays on \`LxVideo\` controls — do not invent an island slider.
`,
  },
];

function frontmatter(title, order, description) {
  const lines = ['---', `title: ${title}`];
  if (description) lines.push(`description: ${JSON.stringify(description)}`);
  lines.push('sidebar:', `  order: ${order}`, '---', '');
  return lines.join('\n');
}

// Collect every exported `Lx*Attributes` type alias across resolved sources.
const found = [];
for (const sf of project.getSourceFiles()) {
  for (const alias of sf.getTypeAliases()) {
    const name = alias.getName();
    if (!/^Lx.*Attributes$/.test(name) || !alias.isExported()) continue;
    if (found.some((f) => f.name === name)) continue;
    found.push({ name, alias });
  }
}

found.sort((a, b) => {
  const oa = META[a.name]?.order ?? 99;
  const ob = META[b.name]?.order ?? 99;
  return oa - ob || a.name.localeCompare(b.name);
});

mkdirSync(outDir, { recursive: true });

const overviewRows = [];

for (const { name, alias } of found) {
  const meta = META[name] ?? {
    title: name.replace(/^Lx/, '').replace(/Attributes$/, ''),
    tag: 'lx-' + kebab(name.replace(/^Lx/, '').replace(/Attributes$/, '')),
    order: 99,
    summary: '',
  };

  const type = alias.getType();
  const props = [];
  const events = [];

  for (const prop of type.getProperties()) {
    const propName = prop.getName();
    const decl = prop.getDeclarations()[0];
    const optional = prop.isOptional?.() ?? true;
    // Drop the `| undefined` the optional marker adds — the Required column says it.
    const typeText = decl
      ? checker.getTypeOfSymbolAtLocation(prop, decl).getNonNullableType().getText(decl)
      : 'unknown';
    const row = {
      name: propName,
      attr: kebab(propName),
      type: typeText,
      required: !optional,
      desc: describe(prop),
    };
    if (/^on[A-Z]/.test(propName)) events.push(row);
    else props.push(row);
  }

  let body = frontmatter(meta.title, meta.order, meta.summary);
  if (meta.summary) body += `${meta.summary}\n\n`;
  body += `Custom element: \`<${meta.tag}>\`\n\n`;
  body += `Attributes interface: \`${name}\` (from \`@lingxia/elements\`).\n\n`;
  body += `React and Vue apps should import the framework wrapper from \`@lingxia/react\` or \`@lingxia/vue\`; HTML views use the custom element directly. See [LxApp pages](../../../guide/lxapp-pages/) for framework and callback guidance.\n\n`;

  body += `## Properties\n\n`;
  if (props.length) {
    body += `| Property | Attribute | Type | Required | Description |\n`;
    body += `| --- | --- | --- | --- | --- |\n`;
    for (const p of props) {
      body += `| \`${p.name}\` | \`${p.attr}\` | ${mdType(p.type)} | ${p.required ? 'yes' : 'no'} | ${escapeCell(p.desc)} |\n`;
    }
    body += '\n';
  } else {
    body += `_No properties._\n\n`;
  }

  if (events.length) {
    body += `## Events\n\n`;
    body += `| Handler | Type | Description |\n`;
    body += `| --- | --- | --- |\n`;
    for (const e of events) {
      body += `| \`${e.name}\` | ${mdType(e.type)} | ${escapeCell(e.desc)} |\n`;
    }
    body += '\n';
  }

  const slug = name.replace(/^Lx/, '').replace(/Attributes$/, '').replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
  writeFileSync(join(outDir, `${slug}.md`), body, 'utf8');
  overviewRows.push({ title: meta.title, tag: meta.tag, slug, summary: meta.summary, order: meta.order });
}

for (const island of ISLAND) {
  let body = frontmatter(island.title, island.order, island.summary);
  body += `${island.summary}\n\n`;
  body += `Custom element: \`<${island.tag}>\`\n\n`;
  body += island.body;
  body += `\nSee [LxApp pages](../../../guide/lxapp-pages/) for the island tree and callback shapes.\n`;
  writeFileSync(join(outDir, `${island.slug}.md`), body, 'utf8');
  overviewRows.push({
    title: island.title,
    tag: island.tag,
    slug: island.slug,
    summary: island.summary,
    order: island.order,
  });
}

overviewRows.sort((a, b) => (a.order ?? 99) - (b.order ?? 99) || a.title.localeCompare(b.title));

// Overview index page.
let index = frontmatter('Components', 0, 'Native-backed custom elements provided by @lingxia/elements.');
index += `Native-backed custom elements provided by the \`@lingxia/elements\` package. React and Vue apps normally import their wrappers from \`@lingxia/react\` or \`@lingxia/vue\`; HTML views use these custom elements directly.\n\n`;
index += `Two families: the **inline native island** (\`LxNativeRoot\` wraps \`LxVideo\`, plus Cover / View / Text / Button) and **presenters** (\`LxPicker\`, \`LxMediaSwiper\`, \`LxNavigator\`). Text entry stays on the web platform with \`<input>\` and \`<textarea>\`—there is no \`LxInput\`.\n\n`;
index += `| Component | Element | Description |\n`;
index += `| --- | --- | --- |\n`;
for (const r of overviewRows) {
  index += `| [${r.title}](./${r.slug}/) | \`<${r.tag}>\` | ${escapeCell(r.summary)} |\n`;
}
index += `\n:::note\nThis reference is generated at build time from \`@lingxia/elements\` in this repository when present, otherwise the pinned install. It lists the low-level attribute surface; wrapper callbacks can reshape events. Read [LxApp pages](../../guide/lxapp-pages/) before wiring handlers.\n:::\n`;
writeFileSync(join(outDir, 'index.md'), index, 'utf8');

// Keep existing generated pages in place so Astro's incremental content loader
// observes updates rather than a delete/recreate cycle (which can report
// transient duplicate ids). Remove only pages no longer exported upstream.
const expected = new Set(['index.md', ...overviewRows.map((row) => `${row.slug}.md`)]);
for (const file of readdirSync(outDir)) {
  if (file.endsWith('.md') && !expected.has(file)) unlinkSync(join(outDir, file));
}

console.log(`[gen-components] wrote ${overviewRows.length} component page(s) + index to ${outDir}`);
