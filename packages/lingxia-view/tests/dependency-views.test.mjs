// View components that another package ships as TSX source compile like the app's own.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, cp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { buildPages } from '../compiler/index.mjs';
import { maxField } from '../src/generated/wire.js';
import { decode } from './support/wire.mjs';

globalThis.__lingxiaViewHost = { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs'] };

// A workspace package linked into the lxapp's node_modules (esbuild reads tsconfigs of linked
// packages, not of installed ones), with its own tsconfig asking for the automatic JSX runtime,
// which the View does not have.
const kit = {
  '../kit/package.json': JSON.stringify({ name: 'view-kit', type: 'module', exports: { './card': './src/card.tsx' } }),
  '../kit/tsconfig.json': JSON.stringify({ compilerOptions: { jsx: 'react-jsx', jsxImportSource: 'react' } }),
  '../kit/src/card.tsx': 'export function Card({ title }: { title: string }) { return <text className="text-sm">{title}</text>; }\n',
};

// The lxapp sits in `app/`; `../kit/…` files form a package beside it, linked into its
// node_modules as a workspace does, or copied there as an install does.
async function project(files, run, { install = 'link' } = {}) {
  const base = await mkdtemp(join(tmpdir(), 'lingxia-view-deps-'));
  const root = join(base, 'app');
  try {
    await mkdir(root);
    for (const [name, contents] of Object.entries(files)) {
      await mkdir(dirname(join(root, name)), { recursive: true });
      await writeFile(join(root, name), contents);
    }
    if (files['../kit/package.json']) {
      await mkdir(join(root, 'node_modules'), { recursive: true });
      if (install === 'link') await symlink(join(base, 'kit'), join(root, 'node_modules/view-kit'), 'dir');
      else await cp(join(base, 'kit'), join(root, 'node_modules/view-kit'), { recursive: true });
    }
    await run(root, (options = {}) => buildPages(root, ['page.tsx'], join(root, 'dist'), {}, options));
  } finally { await rm(base, { recursive: true, force: true }); }
}
async function load(root) {
  const source = await readFile(join(root, 'dist/page.js'), 'utf8');
  await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return source;
}
const records = () => decode(globalThis.__lingxiaDescribe(globalThis.__lingxiaRender, { data: {}, env: {} }));

test("a dependency's TSX compiles with the View's JSX whatever its own tsconfig says; the lxapp's paths still resolve", async () => {
  await project({
    ...kit,
    'tsconfig.json': JSON.stringify({ compilerOptions: { jsx: 'preserve', paths: { '@app/*': ['./src/*'] } } }),
    'src/label.ts': "export const label = 'From the app';\n",
    'page.tsx': "import { Card } from 'view-kit/card';\nimport { label } from '@app/label';\nexport function render() { return <view><Card title={label} /></view>; }\n",
  }, async (root, build) => {
    await build();
    const source = await load(root);
    assert.doesNotMatch(source, /jsx-runtime/);
    assert.ok(records().some(record => record.text === 'From the app'));
  });
});

test("a dependency's components are identified by package name and path inside it, linked or installed", async () => {
  const files = { ...kit, 'page.tsx': "import { Card } from 'view-kit/card';\nexport function render() { return <view><Card title=\"Hi\" /></view>; }\n" };
  for (const install of ['link', 'copy']) {
    await project(files, async (root, build) => {
      await build();
      const source = await readFile(join(root, 'dist/page.js'), 'utf8');
      assert.match(source, /Card\.__lxid = "view-kit\/src\/card#Card"/, install);
    }, { install });
  }
});

test("a dev build names a dependency's lines by package name and path, linked or installed", async () => {
  const files = {
    ...kit,
    '../kit/src/total.tsx': 'export function Total({ items }) {\n  const first = items[0];\n  return <view><text>{first.price.toFixed(2)}</text></view>;\n}\n',
    '../kit/package.json': JSON.stringify({ name: 'view-kit', type: 'module', exports: { './card': './src/card.tsx', './total': './src/total.tsx' } }),
    'page.tsx': "import { Total } from 'view-kit/total';\nexport function render() {\n  return <view><Total items={[{}]} /></view>;\n}\n",
  };
  for (const install of ['link', 'copy']) {
    await project(files, async (root, build) => {
      await build({ dev: true });
      // The kit's own JSX sites carry the same names.
      assert.match(await load(root), /__loc: "view-kit\/src\/total\.tsx:3:10"/, install);
      const error = (() => { try { records(); } catch (error) { return error; } })();
      assert.match(error.message, /\n  at view-kit\/src\/total\.tsx:3:\d+\n  at Total \(page\.tsx:3:16\)/, install);
      assert.doesNotMatch(error.message, /node_modules|\.\.\//, install);
    }, { install });
  }
});
