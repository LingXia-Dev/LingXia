// Every TSX block in the View guide is a page: it must type-check, compile and render.
// A block fenced as ```tsx error CODE must fail at build or render with that code;
// ```tsx data={…} renders the block with that `view.data` (fence info is not displayed);
// a ```json i18n/<tag>.json block is the catalogue the chapter's later blocks speak from.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { build } from './support/build.mjs';
import { maxField } from '../src/generated/wire.js';

const packageRoot = fileURLToPath(new URL('..', import.meta.url));
const docs = join(packageRoot, '../../docs/view');
globalThis.__lingxiaViewHost = { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs'] };

const env = {
  theme: 'light', viewport: { width: 390, height: 844 }, insets: { top: 47, right: 0, bottom: 34, left: 0 },
  keyboard: { height: 0 }, pointer: 'coarse', os: 'android', sizeClass: 'compact',
  language: 'en-US', reducedMotion: false, direction: 'ltr', canDock: false, runner: false, renderer: { fallback: null },
};

async function chapters() {
  const files = [join(docs, 'README.md'), ...(await readdir(join(docs, 'guide'))).filter(name => name.endsWith('.md')).sort().map(name => join(docs, 'guide', name))];
  const blocks = [];
  for (const file of files) {
    const text = await readFile(file, 'utf8');
    let theme, messages;
    const fenced = [...text.matchAll(/^```(\w+)([^\n]*)\n([\s\S]*?)^```$/gm)];
    // A View importing `./$types` is typed from the Logic block right after it, else right before it.
    const logicAt = index => fenced[index]?.[1] === 'ts' && fenced[index][3].includes('definePage(') ? fenced[index][3] : undefined;
    for (const [index, match] of fenced.entries()) {
      const [, lang, info, source] = match;
      const line = text.slice(0, match.index).split('\n').length;
      if (lang === 'css') theme = source;
      if (lang === 'json' && /i18n\//.test(info)) messages = JSON.parse(source);
      if (lang !== 'tsx') continue;
      const expected = info.match(/\berror\s+(UI_[A-Z0-9_]+)/)?.[1];
      const data = JSON.parse(info.match(/\bdata=(\{.*\})/)?.[1] ?? '{}');
      const name = `${relative(docs, file)}:${line}`;
      const logic = source.includes("from './$types'") ? logicAt(index + 1) ?? logicAt(index - 1) : undefined;
      if (source.includes("from './$types'") && !logic) throw Error(`${name}: imports ./$types without a definePage block beside it`);
      blocks.push({ name, source, theme, messages, expected, data, logic });
    }
  }
  return blocks;
}

// What `lingxia build` generates beside each page, with the Logic as `page.ts`.
const pageTypes = `import type { View as PageView } from '@lingxia/view';
import type page from './page';
export type View = PageView<typeof page>;
`;

async function writePage(dir, block) {
  await writeFile(join(dir, 'page.tsx'), block.source);
  if (!block.logic) return;
  await writeFile(join(dir, 'page.ts'), block.logic);
  await writeFile(join(dir, '$types.d.ts'), pageTypes);
}

const blocks = await chapters();

async function project(block, run) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-doc-'));
  try {
    await writePage(root, block);
    if (block.theme) await writeFile(join(root, 'theme.css'), block.theme);
    if (block.messages) {
      // The build compiles ICU; these examples use plain text and `{name}` only.
      const said = Object.entries(block.messages).map(([key, { defaultMessage }]) =>
        `${JSON.stringify(key)}: (a, f) => f.j(${JSON.stringify(defaultMessage.split(/(\{\w+\})/))}.map(part => /^\{\w+\}$/.test(part) ? String(a[part.slice(1, -1)]) : part))`);
      await mkdir(join(root, '.lingxia', 'i18n'), { recursive: true });
      await writeFile(join(root, '.lingxia', 'i18n', 'index.js'),
        `export const source = 'en-US';
export const languages = { 'en-US': { ${said.join(', ')} } };
export const formatter = () => ({ j: parts => parts.join('') });
`);
    }
    return await run(root);
  } finally { await rm(root, { recursive: true, force: true }); }
}

async function renderCompiled(root, data) {
  const source = await readFile(join(root, 'dist/page.js'), 'utf8');
  await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  const view = globalThis.__lingxiaCreateView(data, env);
  return globalThis.__lingxiaDescribe(globalThis.__lingxiaRender, view);
}

test('the guide has examples', () => assert.ok(blocks.length > 0));

for (const block of blocks) test(`${block.name} ${block.expected ? `fails with ${block.expected}` : 'compiles and renders'}`, () => project(block, async root => {
  const output = join(root, 'dist');
  const options = { messages: block.messages ? join(root, '.lingxia', 'i18n', 'index.js') : null };
  if (!block.expected) {
    await build(root, ['page.tsx'], output, {}, options);
    let records;
    try { records = await renderCompiled(root, block.data); }
    catch (error) { assert.fail(error.message.split('\n').slice(0, 6).join('\n')); }
    assert.ok(records.length > 0);
    return;
  }
  let failure;
  try { await build(root, ['page.tsx'], output, {}, options); await renderCompiled(root, block.data); }
  catch (error) { failure = error; }
  assert.ok(failure, `expected ${block.expected}, but the example compiled and rendered`);
  assert.match(String(failure.message), new RegExp(`\\b${block.expected}\\b`));
}));

test('guide examples type-check against the authoring API', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-types-'));
  try {
    const files = [];
    for (const [index, block] of blocks.entries()) {
      if (block.expected) continue;
      const dir = join(root, String(index));
      await mkdir(dir);
      await writePage(dir, block);
      files.push([join(dir, 'page.tsx'), block.name]);
      if (block.logic) files.push([join(dir, 'page.ts'), `${block.name} (Logic)`]);
    }
    const options = {
      target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
      jsx: ts.JsxEmit.Preserve, strict: true, noEmit: true, types: [], lib: ['lib.es2020.d.ts'],
      paths: { '@lingxia/view': [join(packageRoot, 'src/index.d.ts')], '@lingxia/types/page': [join(packageRoot, '../lingxia-types/src/page.ts')], '@lingxia/logic': [join(packageRoot, '../lingxia-logic/index.d.ts')] },
    };
    const program = ts.createProgram([...files.map(([file]) => file), join(packageRoot, 'src/index.d.ts')], options);
    const names = new Map(files);
    // The examples are under test, not the packages they import.
    const problems = ts.getPreEmitDiagnostics(program).filter(diagnostic => !diagnostic.file || names.has(diagnostic.file.fileName)).map(diagnostic => {
      const where = diagnostic.file ? names.get(diagnostic.file.fileName) ?? diagnostic.file.fileName : '';
      const line = diagnostic.file ? diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start).line + 1 : 0;
      return `${where} (+${line}): ${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')}`;
    });
    assert.deepEqual(problems, []);
  } finally { await rm(root, { recursive: true, force: true }); }
});
