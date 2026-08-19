// Build-time Logic JS API reference generator.
//
// Reads the pinned `@lingxia/types` declarations and emits one Starlight page
// per capability group into src/content/docs/reference/api/. Run before
// `astro build`/`astro dev` (wired in package.json). Output is a build artifact
// (gitignored) so the reference always tracks the published package version —
// never hand-edited.
//
// GROUPS is the only curated part: it decides which capability a member belongs
// to. A member the runtime adds or drops fails this script instead of silently
// appearing at the end of a page or vanishing from the site.

import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { Project } from 'ts-morph';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, '..');
const require = createRequire(import.meta.url);

const typesEntry = require.resolve('@lingxia/types');
const typesDts = typesEntry.replace(/\.js$/, '.d.ts');
const typesVersion = JSON.parse(
  readFileSync(join(dirname(typesEntry), '../package.json'), 'utf8'),
).version;

const outDir = join(projectRoot, 'src/content/docs/reference/api');

// Capability groups, in sidebar order. `members` lists every `lx.*` member the
// group owns; the script fails on any member missing from, or unknown to, this
// map.
const GROUPS = [
  {
    slug: 'navigation',
    title: 'Navigation',
    summary: 'Move between pages of this lxapp, and between lxapps.',
    members: [
      'navigateTo', 'redirectTo', 'reLaunch', 'navigateBack', 'switchTab',
      'navigateToApp', 'navigateBackApp',
    ],
  },
  {
    slug: 'surfaces',
    title: 'Surfaces and host chrome',
    summary:
      'Open host surfaces and drive the chrome around the page — navigation bar, tab bar, tray, shell, and pull-to-refresh.',
    members: [
      'surface', 'navigationBar', 'tabBar', 'setMoreActions', 'appearance',
      'startPullDownRefresh', 'stopPullDownRefresh',
      'tray', 'shell',
    ],
  },
  {
    slug: 'feedback',
    title: 'Dialogs and feedback',
    summary: 'Ask the user something, or report what just happened.',
    members: ['showModal', 'showActionSheet', 'showToast', 'hideToast', 'share'],
  },
  {
    slug: 'files',
    title: 'Files and storage',
    summary: 'Managed files, the system picker, transfers, and key-value storage.',
    members: [
      'fs', 'openFile', 'chooseFile', 'chooseDirectory',
      'uploadFile', 'downloadFile', 'getStorage',
    ],
  },
  {
    slug: 'media',
    title: 'Media',
    summary: 'Pick, preview, transcode, and save images and video; read a code.',
    members: [
      'chooseMedia', 'previewMedia', 'getImageInfo', 'compressImage',
      'saveImageToPhotosAlbum', 'saveVideoToPhotosAlbum',
      'createVideoContext', 'getVideoInfo', 'extractVideoThumbnail', 'compressVideo',
      'scanCode',
    ],
  },
  {
    slug: 'device',
    title: 'Device and input',
    summary: 'Device, screen, and system facts, plus orientation and key events.',
    members: [
      'getDeviceInfo', 'getScreenInfo', 'getSystemSetting',
      'vibrateShort', 'vibrateLong', 'makePhoneCall', 'getLocation',
      'setDeviceOrientation', 'onDeviceOrientationChange',
      'onKeyDown', 'onKeyUp',
    ],
  },
  {
    slug: 'network',
    title: 'Network and WiFi',
    summary: 'Connectivity facts and events, and the WiFi module.',
    members: [
      'getNetworkInfo', 'onNetworkChange',
      'startWifi', 'stopWifi', 'connectWifi', 'getWifiList', 'getConnectedWifi',
      'onWifiConnected',
    ],
  },
  {
    slug: 'host',
    title: 'Host app and runtime',
    summary: 'The host app around this lxapp, its environment, updates, and automation.',
    members: [
      'app', 'env', 'getLxAppInfo', 'getUpdateManager', 'openExternal',
      'automation', 'supports', 'terminal',
    ],
  },
];

const project = new Project({ compilerOptions: { allowJs: true, skipLibCheck: true } });
project.addSourceFileAtPath(typesDts);
project.resolveSourceFileDependencies();
const checker = project.getTypeChecker();

/** First sentence of a symbol's JSDoc, flattened to one line. */
function summarize(symbol) {
  const parts = symbol?.compilerSymbol?.getDocumentationComment(checker.compilerObject);
  const text = (parts?.map((p) => p.text).join(' ') ?? '').replace(/\s+/g, ' ').trim();
  if (!text) return '';
  const [first] = text.split(/(?<=[.。])\s/);
  return first.trim();
}

/** Declaration source without its doc comment, normalized for a code block. */
function signature(node) {
  return node
    .getText(false)
    .replace(/\r/g, '')
    .replace(/;$/, '')
    .replace(/^\s*\/\/.*$/gm, '')
    .trim();
}

// Collect the merged `Lx` interface. It is declared across several module
// blocks, so members are gathered by name in first-seen order.
const members = new Map();
for (const sourceFile of project.getSourceFiles()) {
  for (const mod of sourceFile.getModules()) {
    for (const iface of mod.getInterfaces()) {
      if (iface.getName() !== 'Lx') continue;
      for (const member of iface.getMembers()) {
        const symbol = member.getSymbol?.();
        const name = symbol?.getName();
        if (!name) continue;
        const entry = members.get(name) ?? {
          name,
          kind: member.getKindName() === 'PropertySignature' ? 'namespace' : 'method',
          declarations: [],
          summary: '',
          node: member,
        };
        entry.declarations.push(member);
        entry.summary ||= summarize(symbol);
        members.set(name, entry);
      }
    }
  }
}

// Drift guards: the curated grouping must describe the published surface
// exactly, in both directions.
const grouped = new Set(GROUPS.flatMap((g) => g.members));
const ungrouped = [...members.keys()].filter((name) => !grouped.has(name)).sort();
const missing = [...grouped].filter((name) => !members.has(name)).sort();
if (ungrouped.length || missing.length) {
  const lines = ['[gen-logic-api] GROUPS no longer matches @lingxia/types@' + typesVersion];
  if (ungrouped.length) lines.push(`  new members with no group: ${ungrouped.join(', ')}`);
  if (missing.length) lines.push(`  grouped members no longer published: ${missing.join(', ')}`);
  lines.push('  Update GROUPS in scripts/gen-logic-api.mjs.');
  throw new Error(lines.join('\n'));
}

function frontmatter(title, order, description) {
  return [
    '---',
    `title: ${title}`,
    `description: ${JSON.stringify(description)}`,
    'sidebar:',
    `  order: ${order}`,
    '---',
    '',
  ].join('\n');
}

/** Parameter names only — the full types live in the code block below it. */
function callForm(entry) {
  if (entry.kind === 'namespace') return `lx.${entry.name}`;
  const params = entry.declarations[0]
    .getParameters()
    .map((p) => p.getName() + (p.hasQuestionToken() ? '?' : ''))
    .join(', ');
  return `lx.${entry.name}(${params})`;
}

/**
 * The option bag a method takes, expanded one level. Overloaded members keep
 * their signatures only — there is no single option shape to show.
 */
function optionsTable(entry) {
  if (entry.declarations.length !== 1) return '';
  const [parameter] = entry.declarations[0].getParameters();
  if (!parameter) return '';
  const type = parameter.getType().getNonNullableType();
  // An either/or option bag (`{ page }` vs `{ path }`) gets one table per
  // variant; more than a few variants is a signature to read, not a table.
  const variants = (type.isUnion() ? type.getUnionTypes() : [type])
    .map((variant) => variant.getNonNullableType())
    .filter((variant) => variant.isObject() && !variant.getCallSignatures().length);
  if (!variants.length || variants.length > 3) return '';

  const blocks = [];
  for (const variant of variants) {
    const rows = [];
    for (const property of variant.getProperties()) {
      const declaration = property.getDeclarations()[0];
      if (!declaration) continue;
      const propertyType = checker
        .getTypeOfSymbolAtLocation(property, declaration)
        .getNonNullableType();
      // `path?: never` only exists to make the variants exclusive.
      if (propertyType.isNever()) continue;
      const typeText = propertyType.getText(declaration).replace(/\s+/g, ' ').replace(/\|/g, '\\|');
      rows.push(
        `| \`${property.getName()}\` | \`${typeText}\` | ` +
          `${property.isOptional?.() ? 'no' : 'yes'} | ` +
          `${summarize(property).replace(/\|/g, '\\|') || '—'} |`,
      );
    }
    if (!rows.length || rows.length > 25) continue;
    blocks.push(
      ['| Field | Type | Required | Description |', '| --- | --- | --- | --- |', ...rows].join('\n'),
    );
  }
  if (!blocks.length) return '';

  const lead = blocks.length > 1
    ? `\`${parameter.getName()}\` — one of:`
    : `\`${parameter.getName()}\`:`;
  return [lead, '', blocks.join('\n\nor\n\n'), ''].join('\n');
}

/** One level of a namespace's own members, as a table. */
function namespaceTable(entry) {
  const type = entry.declarations[0].getType().getNonNullableType();
  const rows = [];
  for (const prop of type.getProperties()) {
    const decl = prop.getDeclarations()[0];
    if (!decl) continue;
    const text = signature(decl).replace(/\s+/g, ' ').replace(/\|/g, '\\|');
    rows.push(`| \`${text}\` | ${summarize(prop).replace(/\|/g, '\\|') || '—'} |`);
  }
  if (!rows.length) return '';
  return ['| Member | Description |', '| --- | --- |', ...rows, ''].join('\n');
}

mkdirSync(outDir, { recursive: true });

const written = [];
GROUPS.forEach((group, index) => {
  let body = frontmatter(group.title, index + 1, group.summary);
  body += `${group.summary}\n\n`;

  for (const name of group.members) {
    const entry = members.get(name);
    body += `## \`${callForm(entry)}\`\n\n`;
    if (entry.summary) body += `${entry.summary}\n\n`;
    body += '```ts\n' + entry.declarations.map(signature).join('\n') + '\n```\n\n';
    const table = entry.kind === 'namespace' ? namespaceTable(entry) : optionsTable(entry);
    if (table) body += `${table}\n`;
  }

  writeFileSync(join(outDir, `${group.slug}.md`), body, 'utf8');
  written.push(group);
});

let index = frontmatter(
  'Logic JS API',
  0,
  `The lx.* surface published by @lingxia/types ${typesVersion}, grouped by capability.`,
);
index += `Everything an lxapp's Logic context can call on the global \`lx\`, grouped by capability. Generated from \`@lingxia/types\` **${typesVersion}** — the version a project on the current CLI installs.\n\n`;
index += `| Capability | What it covers |\n| --- | --- |\n`;
for (const group of written) {
  index += `| [${group.title}](./${group.slug}/) | ${group.summary} |\n`;
}
index += `\n:::note\nSignatures are reproduced from the published declarations; option and result shapes are types in the same package. In an editor, type \`lx.\` and hover a member to read the same information against the exact version your project installs.\n:::\n`;
index += `\nThe \`Page({})\` and \`App({})\` contracts, error codes, and handle types ship in the same package — see [About the Logic JS API](../logic-api/).\n`;
writeFileSync(join(outDir, 'index.md'), index, 'utf8');

const expected = new Set(['index.md', ...written.map((g) => `${g.slug}.md`)]);
for (const file of readdirSync(outDir)) {
  if (file.endsWith('.md') && !expected.has(file)) unlinkSync(join(outDir, file));
}

const count = written.reduce((total, g) => total + g.members.length, 0);
console.log(
  `[gen-logic-api] wrote ${written.length} capability page(s) + index for ` +
    `${count} lx.* members from @lingxia/types@${typesVersion}`,
);
