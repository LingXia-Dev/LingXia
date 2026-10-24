import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { audit, buildPages } from '../compiler.mjs';
import { moduleBanner } from '../compiler/compatibility.mjs';
import { runInNewContext } from 'node:vm';

globalThis.__lingxiaUiHost = {uiAbi:1,maxField:108,features:['candidate-transactions','bounded-jobs']};

test('compiled entry refuses old native and missing capabilities before author code runs', () => {
  const source = moduleBanner('0.25.1') + '\nglobalThis.didRender = true;';
  for (const host of [undefined, {uiAbi:2,maxField:104,features:[]},
    {uiAbi:1,features:['candidate-transactions','bounded-jobs']},
    {uiAbi:1,maxField:104,features:['candidate-transactions','bounded-jobs']},
    {uiAbi:1,maxField:105,features:['candidate-transactions']}]) {
    const context = {__lingxiaUiHost:host};
    assert.throws(() => runInNewContext(source, context), /UI_COMPAT/);
    assert.equal(context.didRender, undefined);
  }
  const context = {__lingxiaUiHost:globalThis.__lingxiaUiHost};
  runInNewContext(source, context);
  assert.equal(context.didRender, true);
});

test('component member expressions are not intrinsic tags', () => {
  assert.doesNotThrow(() => audit("import * as components from './components'; export function render(){return <components.Card custom={true}/>}", 'member.ui.tsx'));
  assert.throws(() => audit('export function render(){return <unsupported/>}', 'member.ui.tsx'), /UI_TAG/);
});

test('DOM diagnostics distinguish free globals from local bindings and property names', () => {
  for (const source of [
    'const metadata = {window: 1}; metadata.window;',
    'function read(document) { return document.title; }',
    'const window = 1; const metadata = {window};',
  ]) assert.doesNotThrow(() => audit(source, 'scope.ui.tsx'));
  for (const source of ['window.location;', 'const metadata = {window};',
    'function read(document) { return document; } document.title;'])
    assert.throws(() => audit(source, 'scope.ui.tsx'), /UI_DOM/);
  assert.throws(() => audit('export function render(){return <><text>Hello</text></>}', 'fragment.ui.tsx'), /UI_FRAGMENT/);
});

test('known spread properties obey the same intrinsic contract as explicit props', () => {
  for (const source of [
    'const view = <text {...{typo:true}}>Hello</text>;',
    'const props = {typo:true}; const view = <text {...props}/>;',
    'const view = <search {...{secure:true}}/>;',
  ]) assert.throws(() => audit(source, 'spread.ui.tsx'), /UI_PROP/);
  assert.doesNotThrow(() => audit('const props = {children:"Hello",className:"text-sm"}; const view = <text {...props}/>;', 'spread.ui.tsx'));
});

test('known classes fail at build time in expressions, hidden branches and lexical constants', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-class-usage-'));
  try {
    await writeFile(join(root, 'page.json'), '{"renderer":"gpu"}');
    for (const expression of ['"grid-cols-2"', 'view.data.active ? "text-sm" : "grid-cols-2"',
      'view.data.active && "grid-cols-2"', 'bad', '(bad as string)']) {
      await writeFile(join(root, 'page.ui.tsx'), `const bad = 'grid-cols-2'; export function render(view){return <column className={${expression}}/>}`);
      await assert.rejects(buildPages(root, ['page.ui.tsx'], join(root, 'dist')), /page.ui.tsx:1:\d+: UI_CLASS: grid-cols-2/);
      await assert.rejects(readFile(join(root, 'dist/page.ui.js')), {code:'ENOENT'});
    }
    await writeFile(join(root, 'page.ui.tsx'), `
      const style = 'grid-cols-2';
      export function render(){const style = 'text-sm'; return <text className={style}>grid-cols-2</text>}
    `);
    await buildPages(root, ['page.ui.tsx'], join(root, 'dist'));
  } finally { await rm(root, { recursive:true, force:true }); }
});

test('compiled wrapper components forward children with explicit JSX precedence', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-children-'));
  try {
    await writeFile(join(root, 'page.json'), '{"renderer":"gpu"}');
    await writeFile(join(root, 'page.ui.tsx'), `
      function Card(props){return <column {...props}/>}
      export function render(){return <Card><text children="Replaced">Visible{true}{false}{null}</text></Card>}
    `);
    await buildPages(root, ['page.ui.tsx'], join(root, 'dist'));
    const source = await readFile(join(root, 'dist/page.ui.js'), 'utf8');
    const entry = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
    const records = entry.describe(entry.render, {data:{},env:{}});
    assert.ok(records.includes('Visible'));
    assert.ok(!records.includes('Replaced'));
    await writeFile(join(root, 'components.tsx'), 'export function Card(props){return <view {...props}/>}');
    await writeFile(join(root, 'page.ui.tsx'), "import * as components from './components';export function render(){return <components.Card className='flex flex-col'><text>Member</text></components.Card>}");
    await buildPages(root, ['page.ui.tsx'], join(root, 'dist'));
    const memberSource=await readFile(join(root,'dist/page.ui.js'),'utf8');
    const member=await import('data:text/javascript;base64,'+Buffer.from(memberSource).toString('base64'));
    const memberRecords=member.describe(member.render,{data:{},env:{}});
    assert.ok(memberRecords.includes('Member'));
    assert.ok(memberRecords.includes(103),'forwarded flex direction reaches native records');
    await writeFile(join(root, 'page.ui.tsx'), 'export function render(){return <view className="flex flex-col md:flex-row"><text>Hello</text></view>}');
    await buildPages(root, ['page.ui.tsx'], join(root, 'dist'));
    await writeFile(join(root, 'page.ui.tsx'), 'export function render(){return <text className="flex-col">Hello</text>}');
    await assert.rejects(buildPages(root, ['page.ui.tsx'], join(root, 'dist')), /UI_LAYOUT: flex-col requires a flex container/);
  } finally { await rm(root, { recursive:true, force:true }); }
});

test('compiled transparent and empty components follow the public children contract', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-composition-'));
  try {
    await writeFile(join(root, 'page.json'), '{"renderer":"gpu"}');
    await writeFile(join(root, 'page.ui.tsx'), `
      function Forward({children}: {children: JSX.Element}) { return children; }
      function Optional(): JSX.Element | null { return null; }
      export function render() { return <column><Forward><text>One</text></Forward><Optional/></column>; }
    `);
    await buildPages(root, ['page.ui.tsx'], join(root, 'dist'));
    const source = await readFile(join(root, 'dist/page.ui.js'), 'utf8');
    const entry = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
    const records = entry.describe(entry.render, {data:{},env:{}});
    assert.ok(records.includes('One'));
    assert.equal(records[1], -1);
  } finally { await rm(root, {recursive:true,force:true}); }
});

test('imported class maps diagnose every selectable branch at its defining module', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-imported-classes-'));
  try {
    await writeFile(join(root, 'page.json'), '{"renderer":"gpu"}');
    await writeFile(join(root, 'styles.ts'), 'export const bad = "grid-cols-2"; export const variants = {good:"text-sm", bad};');
    for (const expression of ['bad', 'variants[view.data.mode]']) {
      await writeFile(join(root, 'page.ui.tsx'), `import {bad, variants} from './styles'; export function render(view){return <text className={${expression}}>Hello</text>}`);
      await assert.rejects(buildPages(root, ['page.ui.tsx'], join(root, 'dist')), /styles.ts:1:\d+: UI_CLASS: grid-cols-2/);
    }
    await writeFile(join(root, 'page.ui.tsx'), 'export function render(){return <text {...{className:"grid-cols-2"}}>Hello</text>}');
    await assert.rejects(buildPages(root, ['page.ui.tsx'], join(root, 'dist')), /UI_CLASS: grid-cols-2/);
    await writeFile(join(root, 'page.ui.tsx'), 'export function render(){return <text {...{className:"grid-cols-2"}} className="text-sm">Hello</text>}');
    await buildPages(root, ['page.ui.tsx'], join(root, 'dist'));
  } finally { await rm(root, { recursive:true, force:true }); }
});

test('virtual list identity inventories are accepted only on lists', () => {
  assert.doesNotThrow(() => audit('const view = <list count={keys.length} itemKeys={keys} row={renderRow}/>;', 'list.ui.tsx'));
  assert.throws(() => audit('const view = <scroll itemKeys={keys}/>;', 'list.ui.tsx'), /UI_PROP: unsupported itemKeys/);
});

test('compiler diagnoses unsupported UI constructs at their source', () => {
  assert.throws(() => audit('const x = <div/>;', 'bad.ui.tsx'), /bad.ui.tsx:1:11: UI_TAG/);
  assert.doesNotThrow(() => audit('const x = <text className="text-sm"/>;', 'good.ui.tsx'));
  assert.throws(() => audit('const x = <text style={{position:"fixed"}}/>;', 'bad.ui.tsx'), /UI_STYLE/);
  assert.throws(() => audit('document.title = "x";', 'bad.ui.tsx'), /UI_DOM/);
  assert.doesNotThrow(() => audit('const x = <input bind={draft} placeholder="Write" keyboard="text" autoFocus onSubmit={save}/>;', 'input.ui.tsx'));
  assert.doesNotThrow(() => audit('const x = <textarea value="" onInput={change} enabled={false}/>;', 'input.ui.tsx'));
  assert.throws(() => audit('const x = <text value="invalid"/>;', 'bad.ui.tsx'), /UI_PROP/);
});

test('compiler admits checkbox composition without widening switch props', () => {
  assert.doesNotThrow(() => audit('const x = <checkbox accessibilityLabel="Task" value={true}><row><text>Task</text></row></checkbox>;', 'check.ui.tsx'));
  assert.throws(() => audit('const x = <switch accessibilityLabel="Task"/>;', 'check.ui.tsx'), /UI_PROP/);
});

test('page builds reject animation utilities before installing a module', async () => {
  assert.throws(()=>audit('const view = <column style={{animation:null}}/>;', 'animation.ui.tsx'),/UI_ANIMATION/);
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-animation-'));
  try {
    await writeFile(join(root, 'animated.json'), '{"renderer":"gpu"}');
    await writeFile(join(root, 'animated.ui.tsx'), 'export function render(){return <column className="animate-pulse"><text>Pending</text></column>;}');
    await assert.rejects(buildPages(root, ['animated.ui.tsx'], join(root,'dist')), /UI_ANIMATION: native presentation scheduling is not connected/);
    await assert.rejects(readFile(join(root,'dist/animated.ui.js')), {code:'ENOENT'});
    await writeFile(join(root, 'tokens.css'), '@theme {--animate-enter:enter 1s;@keyframes enter{from{opacity:0}to{opacity:1}}}');
    await writeFile(join(root, 'animated.ui.tsx'), 'import "./tokens.css";export function render(){return <column className="animate-enter"/>;}');
    await assert.rejects(buildPages(root, ['animated.ui.tsx'], join(root,'dist')), /UI_ANIMATION/);
    await assert.rejects(readFile(join(root,'dist/animated.ui.js')), {code:'ENOENT'});
  } finally { await rm(root,{recursive:true,force:true}); }
});

test('compiler emits executable ESM UI and preserves page config', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-'));
  try {
    await writeFile(join(root, 'hello.json'), '{"renderer":"gpu","title":"Hello"}');
    await writeFile(join(root, 'hello.ui.tsx'), 'export function render() { return <column><stack><text>Back</text><text style={{offset:{x:12,y:-4}}}>Front</text></stack><grid columns={2}><text lines={2}>Hello, world!</text><text>Second card</text></grid><search placeholder="Find a moment"/><slider accessibilityLabel="Volume" value={40}/><radio value="daily" options={[{value:"daily",label:"Daily"},{value:"weekly",label:"Weekly"}]} /></column>; }');
    const catalog = { __names: ['increment'], __modes: { increment: 'call' } };
    await buildPages(root, ['hello.ui.tsx'], join(root, 'dist'), { 'hello.ui.tsx': catalog });
    const module = await readFile(join(root, 'dist/hello.ui.js'), 'utf8');
    assert.match(module, /^\/\/ LingXia UI module/);
    assert.match(module, /export\s*\{/);
    const entry = await import(`data:text/javascript;base64,${Buffer.from(module).toString('base64')}`);
    assert.deepEqual(globalThis.__lingxiaActionCatalog, catalog);
    const records = entry.describe(entry.render, { data: {}, actions: {}, env: {} });
    assert.ok(records.includes('Hello, world!') && records.includes(50));
    assert.ok(records.includes('Front') && records.includes(48) && records.includes(49));
    assert.ok(records.includes(16) && records.includes(47) && records.includes('Second card'));
    assert.ok(records.includes('Find a moment') && records.includes('search'));
    assert.ok(records.includes('Volume') && records.includes(15));
    assert.ok(records.includes('Daily') && records.includes('Weekly'));
    assert.ok(records.includes(12) && records.includes(13));
    assert.equal(JSON.parse(await readFile(join(root, 'dist/hello.json'), 'utf8')).title, 'Hello');
    await assert.rejects(buildPages(root, ['../outside.ui.tsx'], join(root, 'dist')), /Invalid GPU page path/);
    await assert.rejects(buildPages(root, ['hello.ui.tsx'], join(root, 'dist'), {
      'hello.ui.tsx': { __names: ['watch'], __modes: { watch: 'channel' } },
    }), /action metadata/);
    await buildPages(root, ['hello.ui.tsx'], join(root, 'stream'), {
      'hello.ui.tsx': { __names: ['watch'], __modes: { watch: 'stream' } },
    });
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('compiler admits vector paths and rejects interactive vector props', () => {
  assert.doesNotThrow(()=>audit('const icon=<vector viewBox={[0,0,24,24]} paths={[{d:"M0 0h24"}]} className="w-6 text-blue-500"/>;','vector.ui.tsx'));
  assert.throws(()=>audit('const icon=<vector paths={paths} onClick={click}/>;','vector.ui.tsx'),/UI_PROP/);
});

test('compiler accepts boolean bindings and rejects unrelated control events', () => {
  for (const tag of ['checkbox','switch']) {
    assert.doesNotThrow(()=>audit(`const x=<${tag} bind={preference} enabled>Label</${tag}>;`,'check.ui.tsx'));
    assert.doesNotThrow(()=>audit(`const x=<${tag} value={true} onChange={change}/>;`,'check.ui.tsx'));
    assert.throws(()=>audit(`const x=<${tag} onInput={change}/>;`,'check.ui.tsx'),/UI_PROP/);
  }
});

test('bundled TSX classes use theme tokens, conditional strings and explicit style overrides', async () => {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-ui-classes-'));
  try {
    await writeFile(join(root, 'hello.json'), '{"renderer":"gpu"}');
    await writeFile(join(root, 'theme.css'), '@import "./palette.css"; @theme inline { --color-primary: var(--brand); }');
    await writeFile(join(root, 'palette.css'), ':root { --brand: #007aff; } [data-theme="dark"] { --brand: #0a84ff; }');
    await writeFile(join(root, 'hello.ui.tsx'), `
      import './theme.css';
      function Label({ className, children }) { return <text className={className} style={{ fontSize: 17 }}>{children}</text>; }
      export function render(view) { return <Label className={view.data.active ? 'text-sm text-primary dark:text-white' : 'text-sm text-black'}>Hello</Label>; }
    `);
    await buildPages(root, ['hello.ui.tsx'], join(root, 'dist'));
    const source = await readFile(join(root, 'dist/hello.ui.js'), 'utf8');
    const entry = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
    const describe = (active, theme) => entry.describe(entry.render, { data: { active }, env: { theme } });
    const light = describe(true, 'light');
    assert.ok(light.includes('#007affff'));
    assert.ok(light.includes(17));
    assert.ok(!light.includes(14));
    assert.ok(light.includes(17 * (1.25 / .875)));
    assert.ok(describe(true, 'dark').includes('#ffffffff'));
    assert.ok(describe(false, 'dark').includes('#000000ff'));
    assert.doesNotMatch(source, /function compileUtilities|function declarations\(/);
    await writeFile(join(root, 'hello.ui.tsx'), 'export function render() { return <text className="backdrop-invert">Hello</text>; }');
    await assert.rejects(buildPages(root, ['hello.ui.tsx'], join(root, 'dist')), /hello.ui.tsx:1:\d+: UI_CLASS: backdrop-invert/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('compiler admits radio groups but keeps internal options private',()=>{
  assert.doesNotThrow(()=>audit('const x=<radio options={choices} bind={choice} accessibilityLabel="Pace"/>;','radio.ui.tsx'));
  assert.throws(()=>audit('const x=<radio-option/>;','radio.ui.tsx'),/UI_TAG/);
  assert.throws(()=>audit('const x=<radio onClick={change}/>;','radio.ui.tsx'),/UI_PROP/);
});

test('search accepts string binding and reserves its keyboard configuration',()=>{
  assert.doesNotThrow(()=>audit('const x=<search bind={query} onSubmit={find} placeholder="Search"/>;','search.ui.tsx'));
  for(const prop of ['secure={false}','keyboard="text"','onChange={find}'])
    assert.throws(()=>audit(`const x=<search ${prop}/>;`,'search.ui.tsx'),/UI_PROP/);
});

test('slider exposes typed numeric changes rather than click or input events',()=>{
 assert.doesNotThrow(()=>audit('const x=<slider accessibilityLabel="Volume" bind={volume} min={0} max={1} step={0.1}/>;','slider.ui.tsx'));
 for(const name of ['onClick','onInput','onSubmit'])assert.throws(()=>audit(`const x=<slider accessibilityLabel="Range" ${name}={update}/>;`,'slider.ui.tsx'),/UI_PROP/);
});


test('grid columns belong to the layout primitive',()=>{
 assert.doesNotThrow(()=>audit('const x=<grid columns={3}><text>Card</text></grid>;','grid.ui.tsx'));
 assert.throws(()=>audit('const x=<column columns={3}/>;','grid.ui.tsx'),/UI_PROP/);
});


test('compiler keeps stack offsets structured and wire field names private',()=>{
 assert.doesNotThrow(()=>audit('const x=<stack><text style={{offset:{x:10,y:-5}}}>Top</text></stack>;','stack.ui.tsx'));
 for(const field of ['offsetX','offsetY'])assert.throws(()=>audit(`const x=<text style={{${field}:1}}/>;`,'stack.ui.tsx'),/UI_STYLE/);
});


test('compiler scopes lines to text props',()=>{
 assert.doesNotThrow(()=>audit('const x=<text lines={2}>Summary</text>;','text.ui.tsx'));
 for(const tag of ['button','column'])assert.throws(()=>audit(`const x=<${tag} lines={2}/>;`,'text.ui.tsx'),/UI_PROP/);
 assert.throws(()=>audit('const x=<text style={{lines:2}}/>;','text.ui.tsx'),/UI_STYLE/);
});
