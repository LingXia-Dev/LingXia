import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { build } from './support/build.mjs';

async function makeApp(minRuntime) {
  const root = await mkdtemp(join(tmpdir(), 'lx-chunks-'));
  await mkdir(join(root, 'pages', 'a'), { recursive: true });
  await mkdir(join(root, 'pages', 'b'), { recursive: true });
  await writeFile(join(root, 'lxapp.json'), JSON.stringify({ appId: 'demo', minRuntime }));
  await writeFile(
    join(root, 'pages', 'a', 'index.tsx'),
    `import { useLocal } from '@lingxia/view';\nexport function render() { const [n] = useLocal(1); return <text>A {n}</text>; }\n`,
  );
  await writeFile(
    join(root, 'pages', 'b', 'index.tsx'),
    `import { useLocal } from '@lingxia/view';\nexport function render() { const [n] = useLocal(2); return <text>B {n}</text>; }\n`,
  );
  return root;
}

test('two pages share one View chunk and neither page bundles the runtime', async () => {
  const root = await makeApp('0.30.0');
  try {
    const out = join(root, 'dist');
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out);

    const a = await readFile(join(out, 'pages', 'a', 'index.js'), 'utf8');
    const b = await readFile(join(out, 'pages', 'b', 'index.js'), 'utf8');

    const matchA = a.match(/from\s*["'](lx:view\/([0-9a-f]{12}))["']/);
    const matchB = b.match(/from\s*["'](lx:view\/([0-9a-f]{12}))["']/);
    assert.ok(matchA, 'page A imports lx:view/<hash>');
    assert.ok(matchB, 'page B imports lx:view/<hash>');
    assert.equal(matchA[1], matchB[1], 'both pages import the exact same chunk');

    const chunkPath = join(out, '_view', `${matchA[2]}.js`);
    assert.ok(existsSync(chunkPath), 'shared chunk file exists');
    const chunk = await readFile(chunkPath, 'utf8');
    assert.ok(chunk.includes('__lingxiaRender'), 'chunk holds the View runtime');
    assert.ok(!a.includes('__lingxiaPrepareWindows'), 'page A does not bundle runtime internals');
    // Minified, the entry's own imports of these are gone; the page reaches them through the chunk.
    assert.match(a, new RegExp(`import \\* as __lxView from "lx:view/${matchA[2]}";\\n\\n__lxView\\.configureClasses\\(`));
    assert.ok(!b.includes('__lingxiaPrepareWindows'), 'page B does not bundle runtime internals');

    const headerA = JSON.parse(a.split('\n')[0].replace('// LingXia UI module; ', ''));
    assert.ok(headerA.requiredFeatures.includes('view-chunks'), 'page declares view-chunks feature');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('a third page using a @liulidesign part adds it to the shared chunk', async () => {
  const root = await makeApp('0.30.0');
  try {
    await mkdir(join(root, 'node_modules', '@liulidesign', 'ui'), { recursive: true });
    await writeFile(
      join(root, 'node_modules', '@liulidesign', 'ui', 'package.json'),
      JSON.stringify({ name: '@liulidesign/ui', type: 'module', main: './index.tsx' }),
    );
    await writeFile(
      join(root, 'node_modules', '@liulidesign', 'ui', 'index.tsx'),
      `export function Badge({ label }: { label: string }) { return <text>{label}</text>; }\n`,
    );
    await mkdir(join(root, 'pages', 'c'), { recursive: true });
    await writeFile(
      join(root, 'pages', 'c', 'index.tsx'),
      `import { Badge } from '@liulidesign/ui';\nexport function render() { return <Badge label="New" />; }\n`,
    );

    const out = join(root, 'dist');
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx', 'pages/c/index.tsx'], out);

    const c = await readFile(join(out, 'pages', 'c', 'index.js'), 'utf8');
    const match = c.match(/from\s*["'](lx:view\/([0-9a-f]{12}))["']/);
    assert.ok(match, 'page C imports the shared chunk');
    const chunk = await readFile(join(out, '_view', `${match[2]}.js`), 'utf8');
    assert.ok(chunk.includes('__lxPkg_0'), 'chunk exports the Liuli module');
    assert.ok(chunk.includes('Badge'), 'Liuli part lives in the shared chunk');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('per-language catalogues are split out of the shared chunk into _view/<hash>/<lang>.js', async () => {
  const root = await makeApp('0.30.0');
  try {
    await mkdir(join(root, '.lingxia', 'i18n'), { recursive: true });
    await writeFile(join(root, '.lingxia', 'i18n', 'en-US.js'), `export default { "hello": () => "Hello" };\n`);
    await writeFile(join(root, '.lingxia', 'i18n', 'zh-CN.js'), `export default { "hello": () => "你好" };\n`);
    await writeFile(
      join(root, '.lingxia', 'i18n', 'index.js'),
      `import m0 from './en-US.js';\nimport m1 from './zh-CN.js';\nexport const source = "en-US";\nexport const languages = { "en-US": m0, "zh-CN": m1 };\nexport function formatter() { return { j: p => p.join('') }; }\n`,
    );
    const out = join(root, 'dist');
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out, {}, { messages: join(root, '.lingxia', 'i18n', 'index.js') });

    const a = await readFile(join(out, 'pages', 'a', 'index.js'), 'utf8');
    const match = a.match(/from\s*["'](lx:view\/([0-9a-f]{12}))["']/);
    assert.ok(match);
    const hash = match[2];
    const chunk = await readFile(join(out, '_view', `${hash}.js`), 'utf8');
    assert.ok(!chunk.includes('你好'), 'chunk does not inline catalogues');

    const en = await readFile(join(out, '_view', hash, 'en-US.js'), 'utf8');
    const zh = await readFile(join(out, '_view', hash, 'zh-CN.js'), 'utf8');
    assert.ok(en.includes('__lingxiaRegisterCatalogue') && en.includes('Hello') && !en.includes('你好'));
    assert.ok(zh.includes('__lingxiaRegisterCatalogue') && zh.includes('你好') && !zh.includes('Hello'));

    const headerA = JSON.parse(a.split('\n')[0].replace('// LingXia UI module; ', ''));
    assert.equal(headerA.catalogueChunk, `lx:view/${hash}`);
    assert.equal(headerA.catalogueSource, 'en-US');
    assert.deepEqual(headerA.catalogueLanguages, ['en-US', 'zh-CN']);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('incremental single-page dev build preserves sibling Liuli exports and chunkBuildId', async () => {
  const root = await makeApp('0.30.0');
  try {
    await mkdir(join(root, 'node_modules', '@liulidesign', 'ui'), { recursive: true });
    await writeFile(
      join(root, 'node_modules', '@liulidesign', 'ui', 'package.json'),
      JSON.stringify({ name: '@liulidesign/ui', type: 'module', main: './index.tsx' }),
    );
    await writeFile(
      join(root, 'node_modules', '@liulidesign', 'ui', 'index.tsx'),
      `export function Switch({ label }: { label: string }) { return <text>{label}</text>; }\nexport { Switch as switch };\n`,
    );
    await writeFile(
      join(root, 'pages', 'b', 'index.tsx'),
      `import { Switch } from '@liulidesign/ui';\nexport function render() { return <Switch label="On" />; }\n`,
    );
    const out = join(root, 'dist');
    // Full dev build first.
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out, {}, { dev: true });
    const chunkBefore = await readFile(join(out, '_view', 'dev.js'), 'utf8');
    const buildIdBefore = JSON.parse(chunkBefore.split('\n')[0].replace('// LingXia UI module; ', '')).chunkBuildId;

    // Incremental dev rebuild of only page A must keep page B's Liuli export and the same chunkBuildId.
    await writeFile(
      join(root, 'pages', 'a', 'index.tsx'),
      `import { useLocal } from '@lingxia/view';\nexport function render() { const [n] = useLocal(99); return <text>A {n}</text>; }\n`,
    );
    await build(root, ['pages/a/index.tsx'], out, {}, { dev: true });
    const chunkAfter = await readFile(join(out, '_view', 'dev.js'), 'utf8');
    const buildIdAfter = JSON.parse(chunkAfter.split('\n')[0].replace('// LingXia UI module; ', '')).chunkBuildId;
    assert.equal(buildIdAfter, buildIdBefore, 'chunkBuildId stays stable on single-page dev edit');
    assert.ok(chunkAfter.includes('Switch'), 'sibling page Liuli component preserved in dev chunk');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('minRuntime below 0.30.0 stays single-file', async () => {
  const root = await makeApp('0.29.0');
  try {
    const out = join(root, 'dist');
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out);

    const a = await readFile(join(out, 'pages', 'a', 'index.js'), 'utf8');
    assert.ok(!a.includes('lx:view/'), 'old minRuntime does not emit chunk imports');
    assert.ok(!existsSync(join(out, '_view')), 'no _view directory emitted');
    const headerA = JSON.parse(a.split('\n')[0].replace('// LingXia UI module; ', ''));
    assert.ok(!headerA.requiredFeatures.includes('view-chunks'));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

const header = text => JSON.parse(text.split('\n')[0].replace('// LingXia UI module; ', ''));
const classTable = text => JSON.parse(/\n(?:__lxView\.)?configureClasses\((.*)\);\n/.exec(text)[1]).map(rule => rule.name).sort();

async function liuliKit(root) {
  const dir = join(root, 'node_modules', '@liulidesign', 'kit');
  await mkdir(join(dir, 'Button'), { recursive: true });
  await writeFile(join(dir, 'package.json'), JSON.stringify({ name: '@liulidesign/kit', type: 'module', exports: { '.': './index.ts', './store': './store.ts' } }));
  await writeFile(join(dir, 'index.ts'), `export * from './Button/Button';\nexport * from './store';\n`);
  await writeFile(join(dir, 'Button', 'recipe.ts'), `export const recipe = { base: 'px-3 py-2 rounded-md' };\n`);
  await writeFile(join(dir, 'store.ts'), `export const store = { count: 0 };\n`);
  await writeFile(
    join(dir, 'Button', 'Button.tsx'),
    `import { recipe } from './recipe';\nimport { store } from '../store';\nexport function Button({ label }: { label: string }) { store.count++; return <button className={recipe.base}><text>{label}</text></button>; }\n`,
  );
}

test('a page styles and checks the chunk modules it reaches as a single-file build would', async () => {
  const root = await makeApp('0.30.0');
  try {
    await liuliKit(root);
    await writeFile(
      join(root, 'pages', 'a', 'index.tsx'),
      `import { Button, store } from '@liulidesign/kit';\nexport function render() { return <view className="p-4"><Button label={String(store.count)} /></view>; }\n`,
    );
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], join(root, 'chunked'));
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], join(root, 'single'), {}, { chunks: false });
    const chunked = await readFile(join(root, 'chunked', 'pages', 'a', 'index.js'), 'utf8');
    const single = await readFile(join(root, 'single', 'pages', 'a', 'index.js'), 'utf8');
    assert.match(chunked, /lx:view\//);
    assert.deepEqual(classTable(chunked), classTable(single), 'classes of the Liuli recipe reach the page table');
    assert.ok(classTable(chunked).includes('px-3'));
    // The store the page imports through the barrel is the chunk's instance, not a page copy.
    assert.ok(!chunked.includes('count: 0'), 'stateful package module is not bundled into the page');

    await writeFile(join(root, 'node_modules', '@liulidesign', 'kit', 'Button', 'recipe.ts'), `export const recipe = { base: 'px-3 not-a-utility' };\n`);
    await assert.rejects(build(root, ['pages/a/index.tsx'], join(root, 'chunked')), /not-a-utility/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('a dev page edit that moves the chunk rebuilds its siblings against the new chunk', async () => {
  const root = await makeApp('0.30.0');
  try {
    await liuliKit(root);
    const out = join(root, 'dist');
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out, {}, { dev: true });
    await writeFile(
      join(root, 'pages', 'a', 'index.tsx'),
      `import { Button } from '@liulidesign/kit';\nexport function render() { return <Button label="A" />; }\n`,
    );
    await build(root, ['pages/a/index.tsx'], out, {}, { dev: true });
    const chunk = header(await readFile(join(out, '_view', 'dev.js'), 'utf8')).chunkBuildId;
    for (const page of ['a', 'b'])
      assert.equal(header(await readFile(join(out, 'pages', page, 'index.js'), 'utf8')).chunkBuildId, chunk, page);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('a language resolves over the declared catalogues, not the order they loaded in', async () => {
  const { registerLanguageCatalogue, lxappText } = await import('../src/runtime/i18n.js');
  const formatter = () => ({});
  const declared = ['zh-TW', 'zh-CN'];
  registerLanguageCatalogue('zh-CN', { hi: () => '简' }, formatter, 'zh-CN', declared);
  registerLanguageCatalogue('zh-TW', { hi: () => '繁' }, formatter, 'zh-CN', declared);
  assert.equal(lxappText('hi', 'zh-HK'), '繁');
  assert.equal(lxappText('hi', 'fr'), '简');
});

// Links a page to its chunk and catalogues as the native loader does, then renders it once.
const RUN_CHUNKED = `
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import vm from 'node:vm';
import { maxField } from ${JSON.stringify(new URL('../src/generated/wire.js', import.meta.url).href)};
const [dist, page, language] = process.argv.slice(1);
const context = vm.createContext({
  __lingxiaViewHost: { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs', 'view-chunks'] },
  console, setTimeout, clearTimeout, queueMicrotask,
});
const source = readFileSync(join(dist, page), 'utf8');
const manifest = JSON.parse(source.split('\\n', 1)[0].replace('// LingXia UI module; ', ''));
const modules = new Map();
const load = async specifier => {
  if (!modules.has(specifier)) {
    const path = join(dist, '_view', specifier.slice('lx:view/'.length) + '.js');
    const module = new vm.SourceTextModule(readFileSync(path, 'utf8'), { context, identifier: specifier });
    modules.set(specifier, module);
    await module.link(load);
  }
  return modules.get(specifier);
};
const pageModule = new vm.SourceTextModule(source, { context, identifier: 'page.js' });
await pageModule.link(load);
await pageModule.evaluate();
for (const tag of manifest.catalogueChunk ? [manifest.catalogueSource, language] : []) {
  const module = await load(manifest.catalogueChunk + '/' + tag);
  await module.evaluate();
}
const buffer = context.__lingxiaDescribe(context.__lingxiaRender, { data: {}, env: { language } });
process.stdout.write(JSON.stringify(buffer.filter(item => typeof item === 'string')));
`;

function renderChunked(dist, page, language = 'en-US') {
  const run = spawnSync(process.execPath, ['--experimental-vm-modules', '--no-warnings', '--input-type=module', '-e', RUN_CHUNKED, dist, page, language], { encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test('a release chunked page renders through its chunk with its classes and catalogue', async () => {
  const root = await makeApp('0.30.0');
  try {
    await liuliKit(root);
    await mkdir(join(root, '.lingxia', 'i18n'), { recursive: true });
    await writeFile(join(root, '.lingxia', 'i18n', 'en-US.js'), `export default { hi: () => 'Hello' };\n`);
    await writeFile(join(root, '.lingxia', 'i18n', 'zh-CN.js'), `export default { hi: () => '你好' };\n`);
    await writeFile(
      join(root, '.lingxia', 'i18n', 'index.js'),
      `import m0 from './en-US.js';\nimport m1 from './zh-CN.js';\nexport const source = "en-US";\nexport const languages = { "en-US": m0, "zh-CN": m1 };\nexport function formatter() { return {}; }\n`,
    );
    await writeFile(
      join(root, 'pages', 'a', 'index.tsx'),
      `import { t } from '@lingxia/view';\nimport { Button } from '@liulidesign/kit';\nexport function render() { return <view className="p-4"><text>{t('hi')}</text><Button label="Go" /></view>; }\n`,
    );
    await writeFile(join(root, 'pages', 'b', 'index.tsx'), `import { Button } from '@liulidesign/kit';\nexport function render() { return <Button label="B" />; }\n`);
    const out = join(root, 'dist');
    await build(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out, {}, { messages: join(root, '.lingxia', 'i18n', 'index.js') });
    assert.deepEqual(renderChunked(out, 'pages/a/index.js').filter(text => text), ['Hello', 'Go']);
    assert.deepEqual(renderChunked(out, 'pages/a/index.js', 'zh-CN').filter(text => text), ['你好', 'Go']);
    assert.deepEqual(renderChunked(out, 'pages/b/index.js').filter(text => text), ['B']);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
