import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { audit, buildPages } from '../compiler/index.mjs';
import { moduleBanner } from '../compiler/compatibility.mjs';
import { runInNewContext } from 'node:vm';
import { maxField } from '../src/generated/wire.js';
import { tree, decode } from './support/wire.mjs';

globalThis.__lingxiaUiHost = { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs'] };
const clean = source => assert.deepEqual(audit(source, 'page.tsx'), []);
const flags = (source, pattern) => assert.match(audit(source, 'page.tsx').join('\n'), pattern);

async function project(files, run) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-compiler-'));
  try {
    await writeFile(join(root, 'page.json'), '{"renderer":"gpu"}');
    for (const [name, contents] of Object.entries(files)) await writeFile(join(root, name), contents);
    await run(root, page => buildPages(root, [page ?? 'page.tsx'], join(root, 'dist')));
  } finally { await rm(root, { recursive: true, force: true }); }
}
async function load(root, name = 'page') {
  const source = await readFile(join(root, `dist/${name}.js`), 'utf8');
  await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return source;
}
const describePage = (view = { data: {}, env: {} }) => globalThis.__lingxiaDescribe(globalThis.__lingxiaRender, view);

test('compiled entry refuses old native and missing capabilities before author code runs', () => {
  const source = moduleBanner('0.25.1') + '\nglobalThis.didRender = true;';
  for (const host of [undefined, { uiAbi: 2, maxField, features: [] }, { uiAbi: 1, features: ['candidate-transactions', 'bounded-jobs'] },
    { uiAbi: 1, maxField: maxField - 1, features: ['candidate-transactions', 'bounded-jobs'] }, { uiAbi: 1, maxField, features: ['candidate-transactions'] }]) {
    const context = { __lingxiaUiHost: host };
    assert.throws(() => runInNewContext(source, context), /UI_COMPAT/);
    assert.equal(context.didRender, undefined);
  }
  const context = { __lingxiaUiHost: globalThis.__lingxiaUiHost };
  runInNewContext(source, context);
  assert.equal(context.didRender, true);
});

test('audit collects every problem with its source position', () => {
  const problems = audit('const a = <div/>;\nconst b = <text onClick={x} style={{alignItems:"center", widthPercent:1}}/>;\ndocument.title;', 'page.tsx');
  assert.deepEqual(problems, [
    'page.tsx:1:11: UI_TAG: unsupported tag <div>',
    'page.tsx:2:17: UI_PROP: onClick is not a property of <text>',
    'page.tsx:2:37: UI_STYLE: alignItems is not supported on <text>',
    'page.tsx:2:58: UI_STYLE: unknown style property widthPercent',
    'page.tsx:3:1: UI_DOM: document is unavailable in GPU Views',
  ]);
});

test('component member expressions are not intrinsic tags; fragments and DOM globals are diagnosed', () => {
  clean("import * as components from './components'; export function render(){return <components.Card custom={true}/>}");
  flags('export function render(){return <unsupported/>}', /UI_TAG/);
  for (const source of ['const metadata = {window: 1}; metadata.window;', 'function read(document) { return document.title; }', 'const window = 1; const metadata = {window};'])
    clean(source);
  for (const source of ['window.location;', 'const metadata = {window};', 'function read(document) { return document; } document.title;']) flags(source, /UI_DOM/);
  flags('export function render(){return <><text>Hello</text></>}', /UI_FRAGMENT/);
});

test('spread and explicit props obey one element contract', () => {
  for (const source of ['const view = <text {...{typo:true}}>Hello</text>;', 'const props = {typo:true}; const view = <text {...props}/>;',
    'const view = <search {...{secure:true}}/>;']) flags(source, /UI_PROP/);
  clean('const props = {children:"Hello",className:"text-sm"}; const view = <text {...props}/>;');
  clean('const view = <list count={keys.length} itemKeys={keys} row={renderRow}/>;');
  flags('const view = <scroll itemKeys={keys}/>;', /UI_PROP: itemKeys is not a property of <scroll>/);
  clean('const x = <input bind={draft} placeholder="Write" keyboard="text" autoFocus onSubmit={save} disabled/>;');
  clean('const x = <textarea value="" onInput={change} disabled={false}/>;');
  flags('const x = <textarea enabled={false}/>;', /UI_PROP: enabled/);
  clean('const x = <checkbox value={true}><row><text>Task</text></row></checkbox>;');
  clean('const x = <switch accessibilityLabel="Task" bind={on}/>;');
  for (const prop of ['secure={false}', 'keyboard="text"', 'onChange={find}']) flags(`const x=<search ${prop}/>;`, /UI_PROP/);
  for (const name of ['onClick', 'onInput', 'onSubmit']) flags(`const x=<slider accessibilityLabel="Range" ${name}={update}/>;`, /UI_PROP/);
  flags('const x=<radio-option/>;', /UI_TAG/);
  flags('const x=<radio onClick={change}/>;', /UI_PROP/);
  flags('const icon=<vector paths={paths} onClick={click}/>;', /UI_PROP/);
  for (const [tag, prop] of [['text', 'lines={2}'], ['grid', 'columns={3}'], ['image', 'fit="cover"']]) flags(`const x=<${tag} ${prop}/>;`, /UI_PROP/);
});

test('inline styles are CSS-named and checked per element', () => {
  clean('const x=<grid style={{gridTemplateColumns:3}}><text style={{lineClamp:2}}>Card</text></grid>;');
  clean('const x=<stack><text style={{position:"absolute",top:-5,left:10}}>Top</text></stack>;');
  flags('const x=<column style={{gridTemplateColumns:3}}/>;', /UI_STYLE: gridTemplateColumns is not supported on <column>/);
  flags('const x=<text style={{offset:{x:1}}}/>;', /UI_STYLE: unknown style property offset/);
  flags('const view = <column style={{animation:null}}/>;', /UI_ANIMATION/);
});

test('class diagnostics separate unknown, unsupported and wrong-element utilities and report all at once', async () => {
  await project({ 'page.tsx': `export function render(view) {
  return <column className="grid-cols-2 not-a-utility transition">
    <text className={view.data.on ? "text-sm" : "items-center"}>A</text>
    <column className="active:bg-red-500 hover:p-4"/>
  </column>;
}` }, async (root, build) => {
    const error = await build().catch(error => error);
    const lines = error.message.split('\n');
    assert.match(lines[0], /page.tsx: 6 authoring problems/);
    for (const pattern of [
      /^page.tsx:2:\d+: UI_CLASS_ELEMENT: grid-cols-2 sets gridTemplateColumns, which <column> does not support$/,
      /^page.tsx:2:\d+: UI_CLASS_UNKNOWN: not-a-utility is not a Tailwind utility$/,
      /^page.tsx:2:\d+: UI_CLASS_UNSUPPORTED: transition: the native renderer does not support transition-property/,
      /^page.tsx:3:\d+: UI_CLASS_ELEMENT: items-center sets alignItems, which <text> does not support$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: active:bg-red-500 needs a control; <column> is never pressed$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: hover:p-4: hover styles change paint only, not paddingTop, paddingRight, paddingBottom, paddingLeft$/,
    ]) assert.ok(lines.some(line => pattern.test(line)), `${pattern} in\n${error.message}`);
    await assert.rejects(readFile(join(root, 'dist/page.js')), { code: 'ENOENT' });
  });
});

test('class usage follows lexical constants, hidden branches and imported maps to their defining module', async () => {
  await project({ 'styles.ts': 'export const bad = "nope-utility"; export const variants = {good:"text-sm", bad};' }, async (root, build) => {
    for (const expression of ['"nope-utility"', 'view.data.active ? "text-sm" : "nope-utility"', 'view.data.active && "nope-utility"', 'local', '(local as string)']) {
      await writeFile(join(root, 'page.tsx'), `const local = 'nope-utility'; export function render(view){return <column className={${expression}}/>}`);
      await assert.rejects(build(), /page.tsx:1:\d+: UI_CLASS_UNKNOWN: nope-utility/);
    }
    for (const expression of ['bad', 'variants[view.data.mode]']) {
      await writeFile(join(root, 'page.tsx'), `import {bad, variants} from './styles'; export function render(view){return <text className={${expression}}>Hello</text>}`);
      await assert.rejects(build(), /styles.ts:1:\d+: UI_CLASS_UNKNOWN: nope-utility/);
    }
    await writeFile(join(root, 'page.tsx'), 'export function render(){return <text {...{className:"nope-utility"}} className="text-sm">Hello</text>}');
    await build();
    await writeFile(join(root, 'page.tsx'), "const style = 'nope-utility';\nexport function render(){const style = 'text-sm'; return <text className={style}>nope-utility</text>}");
    await build();
  });
});

test('runtime errors in compiled pages name the JSX site and component chain', async () => {
  await project({ 'page.tsx': `function Card({ children }) {
  return <column>{children}</column>;
}
export function render() {
  return <Card><text onClick={() => {}}>x</text></Card>;
}` }, async (root, build) => {
    // A spread the checker cannot see past reaches the runtime, which reports the JSX chain.
    await writeFile(join(root, 'page.tsx'), `function Card({ children }) {
  return <column>{children}</column>;
}
const extra = JSON.parse('{"onClick":1}');
export function render() {
  return <Card><text {...extra}>x</text></Card>;
}`);
    await build();
    await load(root);
    assert.throws(() => describePage(), /UI_PROP: onClick is not a property of <text>\n  at <text> \(page.tsx:6:16\)\n  at <column> \(page.tsx:2:10\)\n  at Card \(page.tsx:6:10\)/);
  });
});

test('wrapper components forward children; renderer config is the only page switch', async () => {
  await project({ 'page.tsx': `
      function Card(props){return <column {...props}/>}
      export function render(){return <Card><text children="Replaced">Visible{true}{false}{null}{''}</text></Card>}
    ` }, async (root, build) => {
    await build(); await load(root);
    const card = tree(describePage());
    assert.equal(card.children[0].text, 'Visible');
    await writeFile(join(root, 'components.tsx'), 'export function Card(props){return <view {...props}/>}');
    await writeFile(join(root, 'page.tsx'), "import * as components from './components';export function render(){return <components.Card className='flex flex-col'><text>Member</text></components.Card>}");
    await build(); await load(root);
    assert.equal(tree(describePage()).props.flexDirection, 'column');
    await writeFile(join(root, 'page.json'), '{"renderer":"webview"}');
    await assert.rejects(build(), /page.json: expected "renderer": "gpu"/);
    await assert.rejects(build('../outside.tsx'), /Invalid GPU page path/);
  });
});

test('compiled transparent and empty components follow the public children contract', async () => {
  await project({ 'page.tsx': `
      function Forward({children}: {children: JSX.Element}) { return children; }
      function Optional(): JSX.Element | null { return null; }
      export function render() { return <column><Forward><text>One</text></Forward><Optional/></column>; }
    ` }, async (root, build) => {
    await build(); await load(root);
    const column = tree(describePage());
    assert.deepEqual(column.children.map(child => child.text), ['One']);
  });
});

test('page builds reject animation utilities before installing a module', async () => {
  await project({ 'page.tsx': 'export function render(){return <column className="animate-pulse"><text>Pending</text></column>;}' }, async (root, build) => {
    await assert.rejects(build(), /UI_ANIMATION: native presentation scheduling is not connected/);
    await assert.rejects(readFile(join(root, 'dist/page.js')), { code: 'ENOENT' });
    await writeFile(join(root, 'tokens.css'), '@theme {--animate-enter:enter 1s;@keyframes enter{from{opacity:0}to{opacity:1}}}');
    await writeFile(join(root, 'page.tsx'), 'import "./tokens.css";export function render(){return <column className="animate-enter"/>;}');
    await assert.rejects(build(), /UI_ANIMATION/);
  });
});

test('compiler emits executable ESM UI, carries the action catalog and preserves page config', async () => {
  await project({
    'page.json': '{"renderer":"gpu","title":"Hello"}',
    'page.tsx': `export function render() { return <column>
      <stack><text>Back</text><text style={{position:"absolute",left:12,top:-4}}>Front</text></stack>
      <grid style={{gridTemplateColumns:2}}><text style={{lineClamp:2}}>Hello, world!</text><text>Second card</text></grid>
      <search placeholder="Find a moment"/><slider accessibilityLabel="Volume" value={40}/>
      <radio value="daily" options={[{value:"daily",label:"Daily"},{value:"weekly",label:"Weekly"}]} />
    </column>; }`,
  }, async root => {
    const catalog = { __names: ['increment'], __modes: { increment: 'call' } };
    await buildPages(root, ['page.tsx'], join(root, 'dist'), { 'page.tsx': catalog });
    const module = await load(root);
    assert.match(module, /^\/\/ LingXia UI module/);
    assert.deepEqual(globalThis.__lingxiaActionCatalog, catalog);
    const records = decode(describePage({ data: {}, actions: {}, env: {} }));
    const named = text => records.find(record => record.text === text);
    assert.equal(named('Hello, world!').props.textLines, 2);
    assert.deepEqual([named('Front').props.insetLeft, named('Front').props.insetTop], [12, -4]);
    assert.equal(records.find(record => record.tag === 'grid').props.gridColumns, 2);
    assert.equal(records.find(record => record.tag === 'search').props.placeholder, 'Find a moment');
    assert.equal(records.find(record => record.tag === 'slider').props.sliderValue, 40);
    assert.deepEqual(records.filter(record => record.tag === 'radio-option').map(record => record.text), ['Daily', 'Weekly']);
    assert.equal(JSON.parse(await readFile(join(root, 'dist/page.json'), 'utf8')).title, 'Hello');
    await assert.rejects(buildPages(root, ['page.tsx'], join(root, 'dist'), { 'page.tsx': { __names: ['watch'], __modes: { watch: 'channel' } } }), /action metadata/);
  });
});

test('bundled TSX classes use theme tokens, conditional strings and explicit style overrides', async () => {
  await project({
    'theme.css': '@import "./palette.css"; @theme inline { --color-primary: var(--brand); }',
    'palette.css': ':root { --brand: #007aff; } [data-theme="dark"] { --brand: #0a84ff; }',
    'page.tsx': `
      import './theme.css';
      function Label({ className, children }) { return <text className={className} style={{ fontSize: 17 }}>{children}</text>; }
      export function render(view) { return <Label className={view.data.active ? 'text-sm text-primary dark:text-white' : 'text-sm text-black'}>Hello</Label>; }
    `,
  }, async (root, build) => {
    await build();
    const source = await load(root);
    const label = (active, theme) => tree(describePage({ data: { active }, env: { theme } })).props;
    const light = label(true, 'light');
    assert.deepEqual([light.color, light.fontSize, light.lineHeight], ['#007affff', 17, 17 * (1.25 / .875)]);
    assert.equal(label(true, 'dark').color, '#ffffffff');
    assert.equal(label(false, 'dark').color, '#000000ff');
    assert.doesNotMatch(source, /function compileUtilities|function declarations\(/);
    await writeFile(join(root, 'page.tsx'), 'export function render() { return <text className="backdrop-invert">Hello</text>; }');
    await assert.rejects(build(), /page.tsx:1:\d+: UI_CLASS_UNSUPPORTED: backdrop-invert/);
  });
});
