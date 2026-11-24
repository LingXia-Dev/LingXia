import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { buildPages } from '../compiler/index.mjs';

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
    await buildPages(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out);

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
    await buildPages(root, ['pages/a/index.tsx', 'pages/b/index.tsx', 'pages/c/index.tsx'], out);

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
    await buildPages(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out);

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
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('minRuntime below 0.30.0 stays single-file', async () => {
  const root = await makeApp('0.29.0');
  try {
    const out = join(root, 'dist');
    await buildPages(root, ['pages/a/index.tsx', 'pages/b/index.tsx'], out);

    const a = await readFile(join(out, 'pages', 'a', 'index.js'), 'utf8');
    assert.ok(!a.includes('lx:view/'), 'old minRuntime does not emit chunk imports');
    assert.ok(!existsSync(join(out, '_view')), 'no _view directory emitted');
    const headerA = JSON.parse(a.split('\n')[0].replace('// LingXia UI module; ', ''));
    assert.ok(!headerA.requiredFeatures.includes('view-chunks'));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
