import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildPages } from '../compiler/index.mjs';

const packageRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const repoRoot = resolve(packageRoot, '../..');
const snapshotPath = join(packageRoot, 'tests/fixtures/real-app-base-snapshot.json.gz');

const snapshot = JSON.parse(gunzipSync(readFileSync(snapshotPath)).toString('utf8'));

function extractPageTables(jsText) {
  const clsMatch = jsText.match(/\n(?:__lxView\.)?configureClasses\((.*)\);\n/);
  assert.ok(clsMatch, 'built page must emit configureClasses(...)');
  const classes = JSON.parse(clsMatch[1]);
  const fontMatch = jsText.match(/\n(?:__lxView\.)?configureFonts\((.*)\);\n/);
  const fonts = fontMatch ? JSON.parse(`[${fontMatch[1]}]`) : null;
  return { classes, fonts };
}

function discoverPages(appRoot) {
  const pagesDir = join(appRoot, 'pages');
  return readdirSync(pagesDir)
    .filter(name => existsSync(join(pagesDir, name, 'index.tsx')))
    .map(name => `pages/${name}/index.tsx`)
    .sort();
}

const APPS = [
  {
    name: 'showcase',
    root: join(repoRoot, 'examples/lingxia-showcase/lxapp'),
    required: ['node_modules', '.lingxia/i18n'],
  },
  {
    name: 'gallery',
    root: join(repoRoot, 'packages/liuli/gallery'),
    required: ['node_modules'],
  },
];

// A shared-chunk build must give every page the class table and fonts a single-file build gives it.
for (const app of APPS) for (const chunks of [false, true]) {
  test(`real-app snapshot: ${app.name} ${chunks ? 'shared-chunk' : 'single-file'} class tables and theme values match base (${snapshot.baseCommit})`, async t => {
    const missing = app.required.filter(rel => !existsSync(join(app.root, rel)));
    if (missing.length) {
      t.skip(`${app.name} prerequisites missing under ${app.root}: ${missing.join(', ')}`);
      return;
    }

    const pages = discoverPages(app.root);
    const expectedApp = snapshot.apps[app.name];
    assert.ok(expectedApp, `missing ${app.name} in base snapshot`);
    assert.deepEqual(pages, Object.keys(expectedApp.pages).sort(), `${app.name} page list matches snapshot`);

    const outDir = mkdtempSync(join(tmpdir(), `lx-real-app-${app.name}-`));
    try {
      const result = await buildPages(app.root, pages, outDir, {}, { dev: false, chunks });
      const uiErrors = result.warnings.filter(w => /UI_(?!PERF\b|INERT\b|STYLE_STATIC\b|BUDGET\b|POPOVER\b)/.test(w));
      assert.deepEqual(uiErrors, [], `${app.name} emitted unexpected UI_* diagnostics`);
      // Every className is followed to literals, so strictClasses would warn nowhere either.
      assert.deepEqual(result.untracedClasses, [], `${app.name}: some className is not followed to literals`);

      for (const page of pages) {
        const jsPath = join(outDir, page.replace(/\.tsx$/, '.js'));
        assert.ok(existsSync(jsPath), `${app.name} ${page} output exists`);
        if (chunks) assert.match(readFileSync(jsPath, 'utf8'), /from\s*"lx:view\//, `${app.name} ${page} imports the shared chunk`);
        const actual = extractPageTables(readFileSync(jsPath, 'utf8'));
        const expected = expectedApp.pages[page];
        assert.deepEqual(actual.fonts, expected.fonts, `${app.name} ${page} theme/fonts differ from base`);
        assert.deepEqual(actual.classes, expected.classes, `${app.name} ${page} class table differs from base`);
      }
    } finally {
      rmSync(outDir, { recursive: true, force: true });
    }
  });
}
