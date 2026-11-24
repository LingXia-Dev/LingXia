// View components that another package ships as TSX source compile like the app's own.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, cp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from './support/build.mjs';
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
      if (install === 'link') await symlink(join(base, 'kit'), join(root, 'node_modules/view-kit'), 'junction');
      else await cp(join(base, 'kit'), join(root, 'node_modules/view-kit'), { recursive: true });
    }
    await run(root, (options = {}) => build(root, ['page.tsx'], join(root, 'dist'), {}, options));
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

test("the View's paths come from tsconfig.view.json when the root tsconfig only lists projects", async () => {
  await project({
    'tsconfig.json': JSON.stringify({ files: [], references: [{ path: './tsconfig.view.json' }] }),
    'tsconfig.view.json': JSON.stringify({ compilerOptions: { jsx: 'preserve', paths: { '@app/*': ['./src/*'] } } }),
    'src/label.ts': "export const label = 'From the View config';\n",
    'page.tsx': "import { label } from '@app/label';\nexport function render() { return <view><text>{label}</text></view>; }\n",
  }, async (root, build) => {
    await build();
    await load(root);
    assert.ok(records().some(record => record.text === 'From the View config'));
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

// A package that declares tokens: its pages may use its scale names; the app overrides them.
const tokenKit = {
  ...kit,
  '../kit/package.json': JSON.stringify({ name: 'view-kit', type: 'module', exports: { './card': './src/card.tsx', './chip': './src/chip.tsx' }, lingxia: { tokens: './tokens.css' } }),
  '../kit/tokens.css': '@theme {\n  --radius-control: 0.75rem;\n  --spacing-control: 2.5rem;\n}\n',
  '../kit/src/chip.tsx': 'export function Chip() { return <view className="rounded-control min-h-control" />; }\n',
};
const rule = (source, name) => JSON.parse(/\nconfigureClasses\((.*)\);\n/.exec(source)[1]).find(rule => rule.name === name)?.style;

test("a dependency's declared tokens reach the pages that bundle it, below the app's own", async () => {
  for (const install of ['link', 'copy']) {
    await project({ ...tokenKit, 'page.tsx': "import { Chip } from 'view-kit/chip';\nexport function render() { return <view><Chip /></view>; }\n" }, async (root, build) => {
      await build();
      let source = await readFile(join(root, 'dist/page.js'), 'utf8');
      assert.deepEqual(rule(source, 'rounded-control'), { borderRadius: 12 }, install);
      assert.deepEqual(rule(source, 'min-h-control'), { minHeight: 40 }, install);
      await writeFile(join(root, 'theme.css'), '@theme { --radius-control: 1rem; }\n');
      await writeFile(join(root, 'page.tsx'), "import './theme.css';\nimport { Chip } from 'view-kit/chip';\nexport function render() { return <view className=\"rounded-control\"><Chip /></view>; }\n");
      await build();
      source = await readFile(join(root, 'dist/page.js'), 'utf8');
      assert.deepEqual(rule(source, 'rounded-control'), { borderRadius: 16 }, `${install}: the app's token wins`);
    }, { install });
  }
});

test("a page that does not bundle the package does not get its tokens", async () => {
  await project({ ...tokenKit, 'page.tsx': 'export function render() { return <view className="rounded-control" />; }\n' }, async (root, build) => {
    await assert.rejects(build(), /page.tsx:1:\d+: UI_CLASS_UNKNOWN: rounded-control/);
  });
});

test("a dependency's token file holds @theme and @utility only, and two packages may not disagree", async () => {
  const page = "import { Chip } from 'view-kit/chip';\nexport function render() { return <view><Chip /></view>; }\n";
  await project({ ...tokenKit, '../kit/tokens.css': ':root { --color-accent: #ff0000; }\n', 'page.tsx': page }, async (root, build) => {
    await assert.rejects(build(), /view-kit\/tokens\.css:1: UI_CSS: a package's tokens are @theme and @utility only/);
  });
  await project({ ...tokenKit, '../kit/package.json': JSON.stringify({ name: 'view-kit', exports: { './chip': './src/chip.tsx' }, lingxia: { tokens: '../outside.css' } }), 'page.tsx': page }, async (root, build) => {
    await assert.rejects(build(), /view-kit\/package\.json: UI_CSS: lingxia\.tokens names a stylesheet inside the package/);
  });
  const other = {
    'node_modules/other-kit/package.json': JSON.stringify({ name: 'other-kit', exports: { './dot': './dot.tsx' }, lingxia: { tokens: './tokens.css' } }),
    'node_modules/other-kit/tokens.css': '@theme { --radius-control: 0.25rem; }\n',
    'node_modules/other-kit/dot.tsx': 'export function Dot() { return <view className="rounded-control" />; }\n',
  };
  await project({ ...tokenKit, ...other, 'page.tsx': "import { Chip } from 'view-kit/chip';\nimport { Dot } from 'other-kit/dot';\nexport function render() { return <view><Chip /><Dot /></view>; }\n" }, async (root, build) => {
    await assert.rejects(build(), /view-kit\/tokens\.css:2: UI_CSS: --radius-control is 0\.75rem here and 0\.25rem in other-kit; set it once in the app's stylesheet/);
  });
});

test("a dev build lists a linked package's sources and token file, and a linked View runtime, as page inputs, not an installed copy's", async () => {
  const files = { ...tokenKit, 'page.tsx': "import { Chip } from 'view-kit/chip';\nexport function render() { return <view><Chip /></view>; }\n" };
  for (const [install, kitInputs] of [['link', ['../kit/src/chip.tsx', '../kit/tokens.css']], ['copy', []]]) {
    await project(files, async (root, build) => {
      await build({ dev: true });
      const inputs = JSON.parse(await readFile(join(root, '.lingxia/gpu-inputs.json'), 'utf8'));
      const runtime = inputs['page.tsx'].filter(path => path.endsWith('/lingxia-view/src/runtime/tree.js'));
      assert.deepEqual(inputs['page.tsx'].filter(path => !path.includes('/lingxia-view/src/')).sort(), ['page.tsx', ...kitInputs].sort(), install);
      // This View is a workspace copy, not an installed one, so its runtime is an input too.
      assert.equal(runtime.length, 1, install);
    }, { install });
  }
});

test("a dependency's build problems name its file by package, linked or installed", async () => {
  const files = { ...kit, '../kit/src/card.tsx': 'export function Card() { return <text className="text-sm bg-acent">x</text>; }\n',
    'page.tsx': "import { Card } from 'view-kit/card';\nexport function render() { return <view><Card /></view>; }\n" };
  for (const install of ['link', 'copy']) {
    await project(files, async (root, build) => {
      const message = (await build().catch(error => error)).message;
      assert.match(message, /^view-kit\/src\/card\.tsx:1:\d+: UI_CLASS_UNKNOWN: bg-acent/m, install);
      assert.doesNotMatch(message, /\/(tmp|var|private)\//, install);
    }, { install });
  }
});

test("tokens of a bundled package's peer dependencies load transitively, below the app's own, and a missing peer fails the build", async () => {
  const fixture = fileURLToPath(new URL('./fixtures/peer-tokens', import.meta.url));
  const base = await mkdtemp(join(tmpdir(), 'lingxia-view-peers-'));
  try {
    await cp(fixture, base, { recursive: true });
    const app = join(base, 'app');
    await mkdir(join(app, 'node_modules'), { recursive: true });
    await symlink(join(base, 'pkg-b'), join(app, 'node_modules/pkg-b'), 'junction');
    // Missing peer: pkg-b peers on pkg-a, which is not installed yet.
    await assert.rejects(
      build(app, ['page.tsx'], join(app, 'dist')),
      /pkg-b\/package\.json: UI_MODULE:.*pkg-a.*pkg-b/,
    );
    // Install pkg-a: its tokens load for pkg-b.
    await symlink(join(base, 'pkg-a'), join(app, 'node_modules/pkg-a'), 'junction');
    await build(app, ['page.tsx'], join(app, 'dist'));
    let source = await readFile(join(app, 'dist/page.js'), 'utf8');
    assert.deepEqual(rule(source, 'rounded-peer-card'), { borderRadius: 16 });
    // App's own tokens still win over peer tokens.
    await writeFile(join(app, 'theme.css'), '@theme {\n  --radius-peer-card: 2rem;\n}\n');
    await writeFile(join(app, 'page.tsx'), "import './theme.css';\nimport { Widget } from 'pkg-b';\nexport function render() { return <view><Widget /></view>; }\n");
    await build(app, ['page.tsx'], join(app, 'dist'));
    source = await readFile(join(app, 'dist/page.js'), 'utf8');
    assert.deepEqual(rule(source, 'rounded-peer-card'), { borderRadius: 32 });
    // Transitive peer: pkg-c -> pkg-b -> pkg-a (and cycle pkg-a -> pkg-b does not loop).
    await mkdir(join(base, 'pkg-c'), { recursive: true });
    await writeFile(join(base, 'pkg-c/package.json'), JSON.stringify({ name: 'pkg-c', type: 'module', exports: { '.': './index.tsx' }, peerDependencies: { 'pkg-b': '*' } }));
    await writeFile(join(base, 'pkg-c/index.tsx'), 'export function Outer() { return <view className="rounded-peer-card" />; }\n');
    await writeFile(join(base, 'pkg-a/package.json'), JSON.stringify({ name: 'pkg-a', type: 'module', lingxia: { tokens: './tokens.css' }, peerDependencies: { 'pkg-b': '*' } }));
    await symlink(join(base, 'pkg-c'), join(app, 'node_modules/pkg-c'), 'junction');
    await writeFile(join(app, 'page.tsx'), "import { Outer } from 'pkg-c';\nexport function render() { return <view><Outer /></view>; }\n");
    await build(app, ['page.tsx'], join(app, 'dist'));
    source = await readFile(join(app, 'dist/page.js'), 'utf8');
    assert.deepEqual(rule(source, 'rounded-peer-card'), { borderRadius: 16 });
    // Peer ordering: pkg-alpha peers on pkg-zeta (alphabetically later); pkg-alpha's @utility comes after pkg-zeta's and wins.
    await mkdir(join(base, 'pkg-zeta'), { recursive: true });
    await writeFile(join(base, 'pkg-zeta/package.json'), JSON.stringify({ name: 'pkg-zeta', type: 'module', lingxia: { tokens: './tokens.css' } }));
    await writeFile(join(base, 'pkg-zeta/tokens.css'), '@utility peer-pad { padding: 4px; }\n');
    await mkdir(join(base, 'pkg-alpha'), { recursive: true });
    await writeFile(join(base, 'pkg-alpha/package.json'), JSON.stringify({ name: 'pkg-alpha', type: 'module', exports: { '.': './index.tsx' }, lingxia: { tokens: './tokens.css' }, peerDependencies: { 'pkg-zeta': '*' } }));
    await writeFile(join(base, 'pkg-alpha/tokens.css'), '@utility peer-pad { padding: 20px; }\n');
    await writeFile(join(base, 'pkg-alpha/index.tsx'), 'export function Alpha() { return <view className="peer-pad" />; }\n');
    await symlink(join(base, 'pkg-zeta'), join(app, 'node_modules/pkg-zeta'), 'junction');
    await symlink(join(base, 'pkg-alpha'), join(app, 'node_modules/pkg-alpha'), 'junction');
    await writeFile(join(app, 'page.tsx'), "import { Alpha } from 'pkg-alpha';\nexport function render() { return <view><Alpha /></view>; }\n");
    await build(app, ['page.tsx'], join(app, 'dist'));
    source = await readFile(join(app, 'dist/page.js'), 'utf8');
    assert.equal(rule(source, 'peer-pad').paddingTop, 20);
  } finally { await rm(base, { recursive: true, force: true }); }
});
