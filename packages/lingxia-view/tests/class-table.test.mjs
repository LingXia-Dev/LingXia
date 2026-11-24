import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildPages } from '../compiler/index.mjs';
import { maxField } from '../src/generated/wire.js';

globalThis.__lingxiaViewHost = { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs'] };

// Builds `files`, renders the page with `data.on` set, and returns the page's class names.
async function built(files) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-class-table-'));
  try {
    for (const [name, contents] of Object.entries(files)) await writeFile(join(root, name), contents);
    await buildPages(root, ['page.tsx'], join(root, 'dist'));
    const source = await readFile(join(root, 'dist/page.js'), 'utf8');
    const table = JSON.parse(source.match(/\nconfigureClasses\((.*)\);\n/)[1]).map(rule => rule.name);
    await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
    globalThis.__lingxiaDescribe(globalThis.__lingxiaRender, { data: { on: true }, env: {} });
    return table;
  } finally { await rm(root, { recursive: true, force: true }); }
}

const cxPage = (imports, body, expression) =>
  `${imports}\nexport function render(view){ ${body} return <text className={${expression}}>x</text> }`;

for (const [name, imports, body, expression] of [
  ['identifier', `import { cx } from '@lingxia/view';`, '', 'cx({ italic: view.data.on })'],
  ['shorthand', `import { cx } from '@lingxia/view';`, 'const italic = view.data.on;', 'cx({ italic })'],
  ['cx imported under another name', `import { cx as join } from '@lingxia/view';`, '', 'join({ italic: view.data.on })'],
  ['cx through a namespace import', `import * as v from '@lingxia/view';`, '', 'v.cx({ italic: view.data.on })'],
  ['in a nested array', `import { cx } from '@lingxia/view';`, '', `cx('p-1', [[{ italic: view.data.on }]])`],
  ['in a conditional argument', `import { cx } from '@lingxia/view';`, '', `cx(view.data.on ? { italic: true } : 'p-1')`],
  ['behind a logical argument', `import { cx } from '@lingxia/view';`, '', 'cx(view.data.on && { italic: true })'],
])
  test(`class table holds a cx map key: ${name}`, async () => {
    assert.ok((await built({ 'page.tsx': cxPage(imports, body, expression) })).includes('italic'));
  });

test('class table leaves out a computed cx map key, which is known only at run time', async () => {
  const page = cxPage(`import { cx } from '@lingxia/view';`, 'const italic = view.data.on ? "p-2" : "p-3";', 'cx({ [italic]: true }, "p-1")');
  assert.ok(!(await built({ 'page.tsx': page })).includes('italic'));
});

test('class table does not take identifier keys of maps outside cx', async () => {
  const page = cxPage('', 'const flags = { italic: view.data.on };', `flags.italic ? 'p-1' : 'p-2'`);
  assert.ok(!(await built({ 'page.tsx': page })).includes('italic'));
});

test('class table holds the classes of a template over known values', async () => {
  const page = cxPage('', 'const n = 4; const tone = view.data.on ? "accent" : "muted";', '`p-${n} text-${tone}`');
  const table = await built({ 'page.tsx': page });
  for (const name of ['p-4', 'text-accent', 'text-muted']) assert.ok(table.includes(name), name);
});

test('class table adds nothing from a className with a part known only at run time', async () => {
  const page = cxPage(`import { cx } from '@lingxia/view';`, 'const n = 4;', '`p-${n} ${view.data.more}`');
  const error = await built({ 'page.tsx': page }).catch(error => error);
  assert.match(String(error?.message ?? ''), /UI_CLASS_UNKNOWN: p-4 is a utility, but the build found it in no string/);
});

test('class table holds the keys of a cx map built elsewhere and passed by name', async () => {
  const page = cxPage(`import { cx } from '@lingxia/view';`, 'const map = { italic: view.data.on };', 'cx(map)');
  assert.ok((await built({ 'page.tsx': page })).includes('italic'));
});

const usesK = specifier => `import { k } from '${specifier}';\nexport function render(){ return <text className={k}>x</text> }`;
for (const [file, contents] of [
  ['h.mjs', `export const k = 'italic';`],
  ['h.cjs', `exports.k = 'italic';`],
  ['h.mts', `export const k: string = 'italic';`],
  ['h.cts', `export const k: string = 'italic';`],
])
  test(`class table holds a class written in a ${file.slice(1)} module`, async () => {
    assert.ok((await built({ 'page.tsx': usesK(`./${file}`), [file]: contents })).includes('italic'));
  });

test('class table holds the string values of an imported JSON file, not its keys', async () => {
  const table = await built({
    'page.tsx': `import t from './t.json';\nexport function render(){ return <text className={t.k}>x</text> }`,
    't.json': '{"k":"italic","underline":1}',
  });
  assert.ok(table.includes('italic'));
  assert.ok(!table.includes('underline'));
});

test('class table ignores a JSON file the page does not import', async () => {
  const table = await built({ 'page.tsx': `export function render(){ return <text className="p-1">x</text> }`, 'other.json': '{"k":"italic"}' });
  assert.ok(!table.includes('italic'));
});

test('UI_CLASS_UNKNOWN says a valid utility was not collected instead of offering it back', async () => {
  // The declaration file names the class, but the className also holds the page's data, so it is not
  // followed in full and adds nothing to the table; the module that runs builds the name, so no string spells it.
  const error = await built({
    'page.tsx': `import { cx } from '@lingxia/view'; import { k } from './h';\nexport function render(view){ return <text className={cx(k, view.data.more)}>x</text> }`,
    'h.js': `export const k = ['ital', 'ic'].join('');`,
    'h.d.ts': `export declare const k = 'italic';`,
  }).catch(error => error);
  assert.match(error.message, /UI_CLASS_UNKNOWN: italic is a utility, but the build found it in no string of this page's sources, so it was not compiled; write 'italic' as one whole word in a string literal/);
  assert.doesNotMatch(error.message, /nearest supported: italic/);
});

// Builds `files` and returns the build's UI_CLASS warnings.
async function classWarnings(files, options = {}) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-class-warnings-'));
  try {
    for (const [name, contents] of Object.entries(files)) await writeFile(join(root, name), contents);
    const { warnings } = await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, options);
    return warnings.filter(line => line.includes('UI_CLASS:'));
  } finally { await rm(root, { recursive: true, force: true }); }
}

test('UI_CLASS warns where a className is built at run time, in any mode', async () => {
  const page = `import { cx } from '@lingxia/view';
function Badge({ tone }: { tone: string }) { return <text className={cx('p-1', tone)}>x</text>; }
export function render(view){ return <view className={'gap-' + view.data.n}><Badge tone={view.data.tone}/><text className={\`text-\${view.data.size}\`}>y</text></view> }`;
  const warnings = await classWarnings({ 'page.tsx': page });
  assert.equal(warnings.length, 3, warnings.join('\n'));
  assert.match(warnings.join('\n'), /page\.tsx:3:\d+: UI_CLASS: `'gap-' \+ view\.data\.n` is computed with `\+` at run time; its classes exist only at run time/);
  assert.match(warnings.join('\n'), /page\.tsx:\d+:\d+: UI_CLASS: `view` comes from the page's View, known only at run time; its classes exist only at run time/);
  assert.match(warnings.join('\n'), /UI_CLASS: `view\.data\.size` is joined into a class name at run time, and its values are not known to the build/);
  assert.deepEqual(await classWarnings({ 'page.tsx': page }, { strictClasses: true }), warnings);
});

test('UI_CLASS names a className the build cannot follow only under strictClasses', async () => {
  const page = `let tone = 'p-1';\nexport function render(view){ return <text className={tone}>x</text> }`;
  assert.deepEqual(await classWarnings({ 'page.tsx': page }), []);
  const strict = await classWarnings({ 'page.tsx': page }, { strictClasses: true });
  assert.equal(strict.length, 1, strict.join('\n'));
  assert.match(strict[0], /^page\.tsx:2:\d+: UI_CLASS: `tone` is a `let` binding, which can be reassigned; write it where the build can follow it to literals$/);
});
