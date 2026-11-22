import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { audit, buildPages, identify } from '../compiler/index.mjs';
import { moduleBanner } from '../compiler/compatibility.mjs';
import { stabilizeClosures } from '../compiler/stable-closures.mjs';
import { runInNewContext } from 'node:vm';
import { maxField, minField } from '../src/generated/wire.js';
import { tree, decode, points } from './support/wire.mjs';

globalThis.__lingxiaViewHost = { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs'] };
const clean = source => assert.deepEqual(audit(source, 'page.tsx'), []);
const flags = (source, pattern) => assert.match(audit(source, 'page.tsx').join('\n'), pattern);

async function project(files, run) {
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-compiler-'));
  try {
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
    { uiAbi: 1, maxField: minField - 1, features: ['candidate-transactions', 'bounded-jobs'] }, { uiAbi: 1, maxField, features: ['candidate-transactions'] }]) {
    const context = { __lingxiaViewHost: host };
    assert.throws(() => runInNewContext(source, context), /UI_COMPAT/);
    assert.equal(context.didRender, undefined);
  }
  // A host below only ignorable fields runs the page; the runtime leaves those fields out.
  for (const host of [globalThis.__lingxiaViewHost, { ...globalThis.__lingxiaViewHost, maxField: minField }]) {
    const context = { __lingxiaViewHost: host };
    runInNewContext(source, context);
    assert.equal(context.didRender, true);
  }
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
  flags('export function render(){ return <button onClick={() => lx.ui.toast("hi")}>Hi</button> }', /UI_LX: lx is Logic-only; do the call in a Logic action/);
  flags('import { lx } from "@lingxia/logic";\nexport function render(){ return <button onClick={() => lx.ui.toast("hi")}>Hi</button> }', /UI_LX: @lingxia\/logic is Logic-only/);
  for (const source of ['const lx = { n: 1 }; export const n = lx.n;', 'const metadata = { lx: 1 }; metadata.lx;', 'import type { NavTarget } from "@lingxia/logic"; export type T = NavTarget;']) clean(source);
  flags('export function render(){return <><text>Hello</text></>}', /UI_FRAGMENT/);
  flags('export const render = () => (<><text>Hello</text></>);', /UI_FRAGMENT/);
  clean('export function render(){return <page data-state="open" className="data-[state=open]:p-4"><text>x</text></page>}');
  flags('export function render(){return <page dataState="open"><text>x</text></page>}', /UI_PROP: dataState is not a property of <page>; data attributes are written data-state/);
  for (const name of ['testId', 'testID', 'dataTestId'])
    flags(`export function render(){return <text ${name}="t">x</text>}`, /; a test id is data-testid \(`id` names popover and aria targets\)/);
  clean('function Rows(){return <><text>a</text><text>b</text></>} export function render(){return <page><Rows/><>{[1].map(n => <text key={String(n)}>x</text>)}</></page>}');
});

test('spread and explicit props obey one element contract', () => {
  for (const source of ['const view = <text {...{typo:true}}>Hello</text>;', 'const props = {typo:true}; const view = <text {...props}/>;',
    'const view = <input type="search" {...{secure:true}}/>;']) flags(source, /UI_PROP/);
  clean('const props = {children:"Hello",className:"text-sm"}; const view = <text {...props}/>;');
  flags('const view = <list count={keys.length} itemKeys={keys} row={renderRow}/>;', /UI_TAG: unsupported tag <list>; use <List> from @lingxia\/view/);
  flags('const view = <scroll itemKeys={keys}/>;', /UI_PROP: itemKeys is not a property of <scroll>/);
  clean('const x = <input bind={draft} aria-label="Note" placeholder="Write" type="text" inputMode="numeric" autoFocus onSubmit={save} disabled/>;');
  clean('const x = <textarea aria-label="Note" value="" onInput={change} disabled={false}/>;');
  flags('const x = <textarea enabled={false}/>;', /UI_PROP: enabled/);
  clean('const x = <checkbox value={true}><view className="flex-row"><text>Task</text></view></checkbox>;');
  clean('const x = <switch aria-label="Task" bind={on}/>;');
  for (const prop of ['secure', 'keyboard="text"', 'onChange={find}']) flags(`const x=<input type="search" ${prop}/>;`, /UI_PROP/);
  for (const name of ['onClick', 'onInput', 'onSubmit']) flags(`const x=<slider aria-label="Range" ${name}={update}/>;`, /UI_PROP/);
  flags('const x=<radio-option/>;', /UI_TAG/);
  flags('const x=<radio onClick={change}/>;', /UI_PROP/);
  flags('const icon=<vector paths={paths} onClick={click}/>;', /UI_PROP/);
  for (const [tag, prop] of [['text', 'lines={2}'], ['view', 'columns={3}'], ['image', 'fit="cover"']]) flags(`const x=<${tag} ${prop}/>;`, /UI_PROP/);
});

test('inline styles are CSS-named and checked per element', () => {
  clean('const x=<view className="grid" style={{gridTemplateColumns:3}}><text style={{lineClamp:2}}>Card</text></view>;');
  clean('const x=<view className="grid *:col-start-1 *:row-start-1"><text style={{position:"absolute",top:-5,left:10}}>Top</text></view>;');
  clean('const x=<view style={{display:"grid",gridTemplateColumns:3}}/>;');
  flags('const x=<text style={{gridTemplateColumns:3}}>A</text>;', /UI_STYLE: gridTemplateColumns is not supported on <text>/);
  flags('const x=<text style={{offset:{x:1}}}/>;', /UI_STYLE: unknown style property offset/);
  flags('const view = <view style={{animation:null}}/>;', /UI_ANIMATION/);
});

test('a literal role on the wrong element fails the build with where it goes', () => {
  clean('const a = <view role="tablist"><button role="tab" onClick={f}>A</button></view>;');
  clean('const a = <view role={open ? "dialog" : undefined} popover="manual" aria-label="Sheet"/>;');
  clean('const a = <text role={label ? "img" : undefined} aria-label={label}>★</text>;');
  clean('const a = <view role={role}/>;');
  for (const [source, message] of [
    ['<view role="tab"/>', 'role="tab" belongs on a <button>'],
    ['<button role="toolbar" onClick={f}>x</button>', 'role="toolbar" belongs on a <view>'],
    ['<button role="spinbutton" onClick={f}>x</button>', 'role="spinbutton" belongs on an <input>'],
    ['<text role="combobox">x</text>', 'role="combobox" belongs on an <input> or a <button>'],
    ['<view role="alertdialog"/>', 'role="alertdialog" goes on a <dialog>'],
    ['<view role="img"/>', 'role="img" goes on a <view> or <text> with an aria-label'],
    ['<view role="img" aria-label=" "/>', 'role="img" goes on a <view> or <text> with an aria-label'],
    ['<view role="dialog" aria-label="x"/>', 'role="dialog" goes on a popover <view>'],
    ['<dialog role="group" aria-label="x"/>', 'a <dialog> keeps its own role'],
    ['<view role={wide ? "list" : "tab"}/>', 'role="tab" belongs on a <button>'],
  ]) {
    const problems = audit(`const a = ${source};`, 'page.tsx');
    assert.equal(problems.length, 1, source);
    assert.ok(problems[0].startsWith(`page.tsx:1:${source.indexOf('role') + 11}: UI_PROP: ${message}`), problems[0]);
  }
});

test('a prop its role or element cannot carry fails the build with what it needs', () => {
  clean('const a = <text role="heading" headingLevel={2}>Title</text>;');
  clean('const a = <text role={level ? "heading" : undefined} headingLevel={level}>Title</text>;');
  clean('const a = <view role={role} headingLevel={2}/>;');
  clean('const a = <button role="menuitemcheckbox" aria-checked="mixed" keepOpen onClick={f}>All</button>;');
  clean('const a = <view id="p" popover="auto" open onToggle={f} onCloseRequest={g} role="dialog" aria-modal aria-label="Sheet"/>;');
  clean('const a = <dialog aria-label="Done" aria-modal open/>;');
  clean('const a = <view role="grid" aria-label="Days" onPage={f}/>;');
  clean('const a = <view popover={kind} onCloseRequest={g}/>;');
  clean('const a = <view {...rest} open/>;');
  clean('const a = <view role="progressbar" aria-valuenow={40} aria-valuetext="40%"/>;');
  clean('const a = <view onPointer={f} aria-valuenow={4} aria-valuemin={0} aria-valuemax={9}/>;');
  clean('const a = <slider aria-label="Volume" value={v} onChange={f} aria-valuetext="loud"/>;');
  clean('const a = <input aria-label="City" role="combobox" aria-valuetext="Paris" aria-activedescendant="o1"/>;');
  for (const [source, prop, message] of [
    ['<text headingLevel={2}>x</text>', 'headingLevel', 'headingLevel belongs on role="heading"'],
    ['<button role="tab" aria-level={2} onClick={f}>x</button>', 'aria-level', 'aria-level belongs on role="treeitem"'],
    ['<input aria-label="A" aria-controls="list"/>', 'aria-controls', 'aria-controls and aria-autocomplete belong on role="combobox"'],
    ['<button role="radio" aria-checked="mixed" onClick={f}>x</button>', 'aria-checked', 'aria-checked="mixed" goes on role="menuitemcheckbox"'],
    ['<button role="tab" aria-checked={false} onClick={f}>x</button>', 'aria-checked', 'aria-checked goes on role="radio"'],
    ['<view aria-modal/>', 'aria-modal', 'aria-modal belongs on a popover with role="dialog"'],
    ['<dialog aria-label="x" aria-modal={false}/>', 'aria-modal', 'a <dialog> is always modal'],
    ['<view onPage={f}/>', 'onPage', 'onPage belongs to a <view role="grid">'],
    ['<button aria-actions="a" onClick={f}>x</button>', 'aria-actions', 'aria-actions belongs to an element with drag'],
    ['<button keepOpen onClick={f}>x</button>', 'keepOpen', 'keepOpen belongs on a menu item'],
    ['<view open/>', 'open', 'open belongs to a popover; add popover'],
    ['<view anchor="b"/>', 'anchor', 'anchor belongs to a popover; add popover'],
    ['<view onCloseRequest={g}/>', 'onCloseRequest', 'onCloseRequest belongs to a popover; add popover'],
    ['<view popover="manual" onCloseRequest={g}/>', 'onCloseRequest', 'onCloseRequest belongs on a popover that is not manual'],
    ['<image src="a.png" alt="" aria-activedescendant="x"/>', 'aria-activedescendant', 'aria-activedescendant belongs on an <input> or <textarea>'],
    ['<view aria-valuenow={3}/>', 'aria-valuenow', 'aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar"'],
    ['<button role="combobox" aria-valuenow={3} onClick={f}>x</button>', 'aria-valuenow', 'role="combobox" takes aria-valuetext only'],
    ['<scroll onSnapChange={g}/>', 'onSnapChange', 'onSnapChange reports the snap index you pass in snapIndex'],
  ]) {
    const problems = audit(`const a = ${source};`, 'page.tsx');
    assert.equal(problems.length, 1, `${source}: ${problems.join('; ')}`);
    assert.ok(problems[0].startsWith(`page.tsx:1:${source.indexOf(` ${prop}`) + 12}: UI_PROP: ${message}`), problems[0]);
  }
});

test('bind and snapIndex on one element fail the build; either alone passes', () => {
  clean('const a = <scroll snapIndex={i} onSnapChange={g}/>;');
  clean('const a = <view drag="y" bind={c} aria-label="S"/>;');
  clean('const a = <scroll {...rest} snapIndex={i}/>;');
  assert.deepEqual(audit('const a = <scroll snapIndex={i}/>;', 'page.tsx'), ['page.tsx:1:19: UI_PROP: snapIndex controls the position, so it needs onSnapChange to hear where a swipe settled; use bind to let the element keep the index itself']);
  clean('const a = <scroll bind={controlled ? undefined : c} snapIndex={controlled ? i : undefined} onSnapChange={controlled ? g : undefined}/>;');
  assert.deepEqual(audit('const a = <scroll bind={c} snapIndex={1}/>;', 'page.tsx').map(problem => problem.split(': ').slice(1).join(': ')), [
    'UI_PROP: bind and snapIndex both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange',
    'UI_PROP: snapIndex and bind both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange',
  ]);
});

test('a <video> names one source, or a player that has one', () => {
  clean('const a = <video src="a.mp4"/>;');
  clean('const a = <video player={hero}/>;');
  clean('const a = <video {...rest}/>;');
  assert.deepEqual(audit('const a = <video/>;', 'page.tsx'), ['page.tsx:1:12: UI_PROP: a <video> needs a source: add src (one file) or sources (a choice of quality), or player={useVideo(name)} to show a player another <video> gives a source']);
  assert.deepEqual(audit('const a = <video src="a.mp4" sources={list}/>;', 'page.tsx').map(problem => problem.split(': ').slice(1).join(': ')), [
    'UI_PROP: src and sources both name the video; give src for one file, or sources for a choice of quality',
    'UI_PROP: sources and src both name the video; give src for one file, or sources for a choice of quality',
  ]);
});

test('a <span> outside <text> fails the build with the fix', () => {
  clean('const a = <text>A <span className="font-bold">b</span> <link to="/t"><span>c</span></link></text>;');
  clean('function Code() { return <span>x</span>; }');
  clean('const a = <view><Code/></view>;');
  assert.deepEqual(audit('const a = <view><span>loose</span></view>;', 'page.tsx'), ['page.tsx:1:17: UI_TEXT: <span> belongs inside <text>; wrap it in a <text>']);
  assert.deepEqual(audit('const a = <button onClick={f}><span>Go</span></button>;', 'page.tsx'), ['page.tsx:1:31: UI_TEXT: <span> belongs inside <text>; wrap it in a <text>']);
});

test('a run takes only its run props: an id or aria state on a <span> or an inline <link> fails the build', () => {
  clean('const a = <text dir="rtl">A <span className="font-bold" data-testid="b" key="b">b</span> <link to="/t" aria-describedby="h">c</link></text>;');
  clean('const a = <view><link to="/t" id="card" aria-describedby="h" aria-busy><text>Card</text></link></view>;');
  assert.deepEqual(audit('const a = <text><span id="x">b</span></text>;', 'page.tsx'), ['page.tsx:1:23: UI_PROP: <span> is a run of its <text>; put id on the <text>']);
  assert.deepEqual(audit('const a = <text><link to="/t" aria-hidden>b</link></text>;', 'page.tsx'), ['page.tsx:1:31: UI_PROP: <link> is a run of its <text>; put aria-hidden on the <text>']);
});

test('a unitless lineHeight past the multiplier cap fails the build with both fixes', () => {
  clean('const a = <text style={{lineHeight: 1.5}}>x</text>;');
  clean('const a = <text style={{lineHeight: "24px"}}>x</text>;');
  assert.deepEqual(audit('const a = <text style={{lineHeight: 24}}>x</text>;', 'page.tsx'),
    ['page.tsx:1:25: UI_STYLE_VALUE: lineHeight 24 is a multiple of the font size (24×), not points; write "24px", or a leading-* class']);
});

test('a link is a block link outside text, a run inside it, and needs to or onClick in both forms', async () => {
  clean('const a = <view><link to="/p" className="p-4 gap-2 active:bg-accent focus-visible:ring-2"><text>A</text><button onClick={f}>B</button></link></view>;');
  clean('const a = <text>See <link to="/t" className="underline">terms</link></text>;');
  clean('const a = <text>{items.map(item => <link key={item} onClick={f}>{item}</link>)}</text>;');
  flags('const a = <view><link className="p-4"><text>A</text></link></view>;', /UI_LINK: a <link> needs to, onClick, popoverTarget or interestFor, disabled or not; disabled only stops the press/);
  flags('const a = <text><link>x</link></text>;', /UI_LINK: a <link> needs to, onClick, popoverTarget or interestFor, disabled or not; disabled only stops the press/);
  flags('const a = <link to="/a"><view><link to="/b"><text>B</text></link></view></link>;', /UI_LINK: a block link cannot sit inside another block link/);
  clean('const a = <link to="/a"><text>Read <link to="/b">more</link></text></link>;');
  flags('const a = <text><link to="/t" style={{padding: 4}}>x</link></text>;', /UI_STYLE: padding is not supported on <link>/);
  await project({ 'page.tsx': `export function render() {
  return <view>
    <text><link to="/t" className="p-4">x</link></text>
    <link to="/c" className="p-4 rounded-lg active:bg-accent"><text>card</text></link>
  </view>;
}` }, async (root, build) => {
    const error = await build().catch(error => error);
    assert.match(error.message, /page.tsx:3:\d+: UI_CLASS_ELEMENT: p-4 does not apply to <link>/);
    assert.doesNotMatch(error.message, /page.tsx:4:/);
  });
});

test('one build reports every page with problems, not only the first', async () => {
  const page = cls => `export function render() { return <view className="${cls}"><text>A</text></view>; }`;
  await project({ 'a.tsx': page('not-a-utility'), 'ok.tsx': page('p-4'), 'b.tsx': page('also-not-one') }, async root => {
    const error = await buildPages(root, ['a.tsx', 'ok.tsx', 'b.tsx'], join(root, 'dist')).catch(error => error);
    assert.match(error.message, /^a.tsx: 1 authoring problem\na.tsx:1:\d+: UI_CLASS_UNKNOWN: not-a-utility/);
    assert.match(error.message, /\nb.tsx: 1 authoring problem\nb.tsx:1:\d+: UI_CLASS_UNKNOWN: also-not-one/);
  });
});

test('class diagnostics separate unknown, unsupported and wrong-element utilities and report all at once', async () => {
  await project({ 'page.tsx': `export function render(view) {
  return <view className="grid-cols-2 not-a-utility transition-[padding]">
    <text className={view.data.on ? "grid-cols-2" : "items-center"}>A</text>
    <view className="active:bg-danger hover:p-4 focus-visible:bg-danger"/>
  </view>;
}` }, async (root, build) => {
    const error = await build().catch(error => error);
    const lines = error.message.split('\n');
    assert.match(lines[0], /page.tsx: 7 authoring problems/);
    for (const pattern of [
      /^page.tsx:3:\d+: UI_CLASS_ELEMENT: grid-cols-2 does not apply to <text>; layout classes go on the element that holds it$/,
      /^page.tsx:2:\d+: UI_CLASS_UNKNOWN: not-a-utility is not a Tailwind utility$/,
      /^page.tsx:2:\d+: UI_CLASS_UNSUPPORTED: transition-\[padding\]: the native renderer does not support transition of padding animates layout/,
      /^page.tsx:3:\d+: UI_CLASS_ELEMENT: items-center does not apply to <text>; layout classes go on the element that holds it$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: active:bg-danger needs a control; <view> is never pressed$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: hover:p-4: hover styles change paint only, not paddingTop, paddingRight, paddingBottom, paddingLeft$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: focus-visible:bg-danger needs an element that takes focus; <view> never does$/,
    ]) assert.ok(lines.some(line => pattern.test(line)), `${pattern} in\n${error.message}`);
    await assert.rejects(readFile(join(root, 'dist/page.js')), { code: 'ENOENT' });
  });
});

test('a box that scales or rotates may not hold an editor, scroll, slider or web view; entering or leaving it only warns', async () => {
  await project({ 'page.tsx': `export function render(view) {
  return <view className="hover:scale-105">
    <view className="translate-x-2">{view.data.on && <input aria-label="Name"/>}</view>
    <view className="animate-spin"><slider aria-label="Volume" bind={view.data.v}/></view>
    <slider aria-label="Level" className="rotate-90 thumb:scale-110" bind={view.data.v}/>
    <view className="scale-100 rotate-0 translate-y-1"><input aria-label="Plain"/></view>
    <view className="scale-95"><text>No editor</text></view>
  </view>;
}` }, async (root, build) => {
    const message = (await build().catch(error => error)).message;
    for (const pattern of [
      /page.tsx:2:\d+: UI_TRANSFORM: hover:scale-105 scales a box holding an <input> \(line 3\); editors, scrolls and sliders cannot rotate or scale/,
      /page.tsx:4:\d+: UI_TRANSFORM: animate-spin rotates a box holding a <slider> \(line 4\)/,
      /page.tsx:5:\d+: UI_TRANSFORM: rotate-90 rotates a <slider>;/,
    ]) assert.match(message, pattern);
    assert.match(message, /3 authoring problems/);
  });
  await project({ 'page.tsx': `export function render(view) {
  return <view>
    <view className="starting:scale-95 translate-x-2"><input aria-label="Name"/></view>
    <view className="exit:rotate-12"><scroll><text>A</text></scroll></view>
  </view>;
}` }, async (root, build) => {
    const transform = (await build()).warnings.filter(line => line.includes('UI_TRANSFORM'));
    assert.equal(transform.length, 2, transform.join('\n'));
    assert.match(transform[0], /page.tsx:3:\d+: UI_TRANSFORM: starting:scale-95 scales a box holding an <input> \(line 3\) as it enters; only its translation moves/);
    assert.match(transform[1], /page.tsx:4:\d+: UI_TRANSFORM: exit:rotate-12 rotates a box holding a <scroll> \(line 4\) as it leaves/);
  });
  await project({ 'page.tsx': `export function render() {
  return <view>
    <view className="hover:scale-105"><webview aria-label="Doc" src="https://example.com"/></view>
    <view className="starting:scale-95 translate-y-2"><webview aria-label="Map" src="https://example.com"/></view>
  </view>;
}` }, async (root, build) => {
    const message = (await build().catch(error => error)).message;
    assert.match(message, /page.tsx:3:\d+: UI_TRANSFORM: hover:scale-105 scales a box holding a <webview> \(line 3\); a web view cannot rotate or scale/);
    assert.match(message, /1 authoring problem/);
  });
  await project({ 'page.tsx': `export function render() {
  return <view className="starting:scale-95 exit:opacity-0"><webview aria-label="Map" src="https://example.com"/></view>;
}` }, async (root, build) => {
    const transform = (await build()).warnings.filter(line => line.includes('UI_TRANSFORM'));
    assert.equal(transform.length, 1, transform.join('\n'));
    assert.match(transform[0], /starting:scale-95 scales a box holding a <webview> \(line 2\) as it enters; the web page shows as a still picture while it does/);
  });
});

test('a part names the controls that draw it', async () => {
  await project({ 'page.tsx': `export function render() {
  return <view className="thumb:size-4">
    <slider aria-label="Volume" className="checked:thumb:bg-accent track:w-8"/>
    <switch aria-label="Wi-Fi" className="range:bg-accent checked:track:bg-success track:w-11"/>
  </view>;
}` }, async (root, build) => {
    const message = (await build().catch(error => error)).message;
    for (const pattern of [
      /page.tsx:2:\d+: UI_CLASS_ELEMENT: thumb:size-4 styles a <slider> or <switch>'s thumb, not <view>/,
      /page.tsx:3:\d+: UI_CLASS_ELEMENT: checked:thumb:bg-accent: a <slider> is never checked/,
      /page.tsx:3:\d+: UI_CLASS_ELEMENT: track:w-8: a <slider> track spans the slider/,
      /page.tsx:4:\d+: UI_CLASS_ELEMENT: range:bg-accent styles a <slider>'s range, not <switch>/,
    ]) assert.match(message, pattern);
    assert.doesNotMatch(message, /checked:track:bg-success|track:w-11/);
  });
});

test('flex-only utilities on a grid name the grid equivalent', async () => {
  await project({ 'page.tsx': 'export function render(){return <view className="grid flex-wrap flex-col items-center content-between justify-items-center"/>}' }, async (root, build) => {
    const message = (await build().catch(error => error)).message;
    assert.match(message, /flex-wrap does not apply to <view className="grid">; grid items wrap into rows by themselves/);
    assert.match(message, /flex-col does not apply to <view className="grid">; use grid-flow-col/);
    assert.doesNotMatch(message, /items-center|content-between|justify-items-center/);
  });
});

test('motion classes are ordinary utilities', async () => {
  const page = 'export function render(){return <button className="active:scale-95 transition-transform duration-150 motion-safe:animate-pulse" onClick={() => {}}>Go</button>}';
  await project({ 'page.tsx': page }, async (root, build) => {
    await build();
    const module = await readFile(join(root, 'dist/page.js'), 'utf8');
    assert.match(module, /transition-transform/);
    assert.match(module, /"whenReduced":"keep"/);
  });
});

test('unknown utilities name the nearest form the native renderer supports', async () => {
  await project({ 'page.tsx': 'export function render(){return <view className="itmes-center md:roundd-lg tabluar-nums zzqqxx"/>}' }, async (root, build) => {
    const message = (await build().catch(error => error)).message;
    assert.match(message, /UI_CLASS_UNKNOWN: itmes-center is not a Tailwind utility; nearest supported: items-center/);
    assert.match(message, /UI_CLASS_UNKNOWN: md:roundd-lg is not a Tailwind utility; nearest supported: md:rounded-lg/);
    assert.match(message, /UI_CLASS_UNKNOWN: tabluar-nums is not a Tailwind utility; nearest supported: tabular-nums/);
    assert.match(message, /UI_CLASS_UNKNOWN: zzqqxx is not a Tailwind utility$/m);
  });
});

test('class usage follows lexical constants, hidden branches and imported maps to their defining module', async () => {
  await project({ 'styles.ts': 'export const bad = "nope-utility"; export const variants = {good:"text-sm", bad};' }, async (root, build) => {
    for (const expression of ['"nope-utility"', 'view.data.active ? "text-sm" : "nope-utility"', 'view.data.active && "nope-utility"', 'local', '(local as string)']) {
      await writeFile(join(root, 'page.tsx'), `const local = 'nope-utility'; export function render(view){return <view className={${expression}}/>}`);
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

test('class usage follows cx arguments, class maps and template pieces', async () => {
  await project({ 'styles.ts': 'export const recipe = { base: "text-sm", size: { sm: "p-1", md: "nope-utility" } };' }, async (root, build) => {
    const page = (expression, prelude = '') =>
      writeFile(join(root, 'page.tsx'), `import { cx, cx as join } from '@lingxia/view'; import { recipe } from './styles'; ${prelude}export function render(view){return <text className={${expression}}>Hello</text>}`);
    for (const expression of ['cx("text-sm", "nope-utility")', 'cx(view.data.on && "nope-utility")', 'cx(["p-1", ["nope-utility"]])',
      'cx({ "p-1": true, "nope-utility": view.data.on })', 'join("nope-utility")', 'local', '`p-1 ${view.data.extra} nope-utility`']) {
      await page(expression, 'const local = cx("p-1", "nope-utility"); ');
      await assert.rejects(build(), /page.tsx:1:\d+: UI_CLASS_UNKNOWN: nope-utility/, expression);
    }
    await page('cx(recipe.base, recipe.size[view.data.size])');
    await assert.rejects(build(), /styles.ts:1:\d+: UI_CLASS_UNKNOWN: nope-utility/);
    // A class name an interpolation joins is unknowable, so it is not checked.
    for (const expression of ['`p-${view.data.n}`', '`nope-${view.data.n} text-sm`', '`${view.data.a}nope-utility`'])
      await page(expression), await build();
    // A local helper named cx is not traced.
    await writeFile(join(root, 'page.tsx'), 'const cx = (...a) => a.join(" "); export function render(){return <text className={cx("nope-utility")}>Hello</text>}');
    await build();
    await writeFile(join(root, 'page.tsx'), `import { cx } from '@lingxia/view';
      export function render(view){return <text className={cx('text-sm', { 'p-4': view.data.on }, view.data.off && 'p-1')}>Hello</text>}`);
    await build();
    await load(root);
    const props = on => tree(describePage({ data: { on, off: false }, env: {} })).props;
    assert.equal(points(props(true).paddingTop), 16);
    assert.equal(props(false).paddingTop, undefined);
  });
});

async function authorColumn(body, lineNumber) {
  let column;
  await project({ 'page.tsx': `${body}\nexport function render() {\n  return <view><Total/></view>;\n}` }, async root => {
    await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { dev: true });
    await load(root);
    const error = (() => { try { describePage(); } catch (error) { return error; } })();
    column = Number(new RegExp(`\\n {2}at page\\.tsx:${lineNumber}:(\\d+)`).exec(error.message)?.[1]);
  });
  return column;
}

test('dev builds name the author column of a runtime error on a JSX line', async () => {
  const line = '  return <view><text>{first.price.toFixed(2)}</text></view>;';
  const column = await authorColumn(`function Total() {\n  const first = {};\n${line}\n}`, 3);
  assert.ok(column > line.indexOf('first') && column <= line.indexOf('(2)') + 1, `column ${column} of ${JSON.stringify(line)}`);
});

test('dev build columns hold after two JSX sites on one line and inside a multi-line element', async () => {
  const line = '  const a = <text>x</text>, b = <text>y</text>; return <view>{a}{b}{first.price.toFixed(2)}</view>;';
  const column = await authorColumn(`function Total() {\n  const first = {};\n${line}\n}`, 3);
  assert.ok(column > line.indexOf('first') && column <= line.indexOf('(2)') + 1, `column ${column} of ${JSON.stringify(line)}`);
  const inner = '      {first.price.toFixed(2)}';
  const multi = await authorColumn(`function Total() {\n  const first = {};\n  return <view\n    style={{ width: 1 }}>\n    <text>\n${inner}\n    </text>\n  </view>;\n}`, 6);
  assert.ok(multi > inner.indexOf('first') && multi <= inner.indexOf('(2)') + 1, `column ${multi} of ${JSON.stringify(inner)}`);
});

test('runtime errors in compiled pages name the JSX site and component chain', async () => {
  await project({ 'page.tsx': `function Card({ children }) {
  return <view>{children}</view>;
}
export function render() {
  return <Card><text onClick={() => {}}>x</text></Card>;
}` }, async (root, build) => {
    // A spread the checker cannot see past reaches the runtime, which reports the JSX chain.
    await writeFile(join(root, 'page.tsx'), `function Card({ children }) {
  return <view>{children}</view>;
}
const extra = JSON.parse('{"onClick":1}');
export function render() {
  return <Card><text {...extra}>x</text></Card>;
}`);
    await build();
    await load(root);
    assert.throws(() => describePage(), /UI_PROP: onClick is not a property of <text>\n  at <text> \(page.tsx:6:16\)\n  at <view> \(page.tsx:2:10\)\n  at Card \(page.tsx:6:10\)/);
  });
});

test('wrapper components forward children; a page config naming a renderer is refused', async () => {
  await project({ 'page.tsx': `
      function Card(props){return <view {...props}/>}
      export function render(){return <Card><text children="Replaced">Visible{true}{false}{null}{''}</text></Card>}
    ` }, async (root, build) => {
    await build(); await load(root);
    const card = tree(describePage());
    assert.equal(card.children[0].text, 'Visible');
    await writeFile(join(root, 'components.tsx'), 'export function Card(props){return <view {...props}/>}');
    await writeFile(join(root, 'page.tsx'), "import * as components from './components';export function render(){return <components.Card className='flex flex-col'><text>Member</text></components.Card>}");
    await build(); await load(root);
    assert.equal(tree(describePage()).props.flexDirection, 'column');
    await writeFile(join(root, 'page.json'), '{"renderer":"gpu"}');
    await assert.rejects(build(), /page.json: renderer: remove it; the renderer follows from the pages/);
    await assert.rejects(build('../outside.tsx'), /Invalid GPU page path/);
  });
});

test('compiled transparent and empty components follow the public children contract', async () => {
  await project({ 'page.tsx': `
      function Forward({children}: {children: JSX.Element}) { return children; }
      function Optional(): JSX.Element | null { return null; }
      export function render() { return <view><Forward><text>One</text></Forward><Optional/></view>; }
    ` }, async (root, build) => {
    await build(); await load(root);
    const column = tree(describePage());
    assert.deepEqual(column.children.map(child => child.text), ['One']);
  });
});

test('page builds reject invalid keyframes and inline animation before installing a module', async () => {
  await project({ 'page.tsx': 'export function render(){return <view style={{ animation: "spin 1s" }}><text>Pending</text></view>;}' }, async (root, build) => {
    await assert.rejects(build(), /UI_ANIMATION: inline animation is not supported; use animate-\* utilities/);
    await assert.rejects(readFile(join(root, 'dist/page.js')), { code: 'ENOENT' });
    await writeFile(join(root, 'tokens.css'), '@theme {--animate-enter:enter 1s;@keyframes enter{from{width:0px}to{width:10px}}}');
    await writeFile(join(root, 'page.tsx'), 'import "./tokens.css";export function render(){return <view className="animate-enter"/>;}');
    await assert.rejects(build(), /UI_ANIMATION|UI_CLASS_UNSUPPORTED/);
    await writeFile(join(root, 'tokens.css'), '@theme {--animate-enter:enter 1s;@keyframes enter{from{opacity:0}to{opacity:1}}}');
    await build();
  });
});

test('compiler emits executable ESM UI, carries the action catalog and preserves page config', async () => {
  await project({
    'page.json': '{"title":"Hello"}',
    'page.tsx': `export function render() { return <view>
      <view className="grid *:col-start-1 *:row-start-1"><text>Back</text><text style={{position:"absolute",left:12,top:-4}}>Front</text></view>
      <view className="grid" style={{gridTemplateColumns:2}}><text style={{lineClamp:2}}>Hello, world!</text><text>Second card</text></view>
      <input type="search" aria-label="Search" placeholder="Find a moment"/><slider aria-label="Volume" value={40}/>
      <radio aria-label="Repeat" value="daily" options={[{value:"daily",label:"Daily"},{value:"weekly",label:"Weekly"}]} />
    </view>; }`,
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
    assert.equal(records.find(record => record.props.display === 'grid' && record.props.gridTemplateColumns).props.gridTemplateColumns, 'repeat(2, minmax(0, 1fr))');
    assert.equal(records.find(record => record.tag === 'search').props.placeholder, 'Find a moment');
    assert.equal(records.find(record => record.tag === 'slider').props.sliderValue, 40);
    assert.deepEqual(records.filter(record => record.tag === 'radio-option').map(record => record.text), ['Daily', 'Weekly']);
    assert.equal(JSON.parse(await readFile(join(root, 'dist/page.json'), 'utf8')).title, 'Hello');
    await assert.rejects(buildPages(root, ['page.tsx'], join(root, 'dist'), { 'page.tsx': { __names: ['watch'], __modes: { watch: 'channel' } } }), /action metadata/);
  });
});

test('bundled TSX classes lower roles, conditional strings and explicit style overrides', async () => {
  await project({
    'theme.css': '@theme { --text-sm: 0.875rem; }',
    'page.tsx': `
      import './theme.css';
      function Label({ className, children }) { return <text className={className} style={{ fontSize: 17 }}>{children}</text>; }
      export function render(view) { return <Label className={view.data.active ? 'text-sm text-accent' : 'text-sm text-muted/60'}>Hello</Label>; }
    `,
  }, async (root, build) => {
    await build();
    const source = await load(root);
    const label = (active, theme) => tree(describePage({ data: { active }, env: { theme } })).props;
    const light = label(true, 'light');
    assert.deepEqual(points([light.color, light.fontSize, light.lineHeight]), ['accent', 17, 18], 'the pack\'s leading is a fixed height');
    assert.equal(label(true, 'dark').color, 'accent', 'roles follow appearance natively');
    assert.equal(label(false, 'dark').color, 'text-muted/60');
    assert.doesNotMatch(source, /function compileUtilities|function declarations\(/);
    await writeFile(join(root, 'page.tsx'), 'export function render() { return <text className="backdrop-invert">Hello</text>; }');
    await assert.rejects(build(), /page.tsx:1:\d+: UI_CLASS_UNSUPPORTED: backdrop-invert/);
  });
});

test('@font-face declares bundled faces the page names by family, with a generic fallback', async () => {
  await project({
    'brand.ttf': 'face bytes',
    'theme.css': '@font-face { font-family: "Brand Serif"; src: url("./brand.ttf") format("truetype"); font-weight: 400; }\n@theme { --font-brand: "Brand Serif", serif; }',
    'page.tsx': `
      import './theme.css';
      export function render() { return <view><text className="font-brand">A</text><text style={{ fontFamily: 'Brand Serif, monospace' }}>B</text><text className="font-serif">C</text></view>; }
    `,
  }, async (root, build) => {
    await build();
    const source = await load(root);
    assert.match(source, /configureFonts\(\[\{"family":"Brand Serif","src":"brand.ttf","weight":"400"\}\]\)/);
    assert.deepEqual(JSON.parse(globalThis.__lingxiaFonts()), [{ family: 'Brand Serif', src: 'brand.ttf', weight: '400' }]);
    assert.deepEqual(tree(describePage()).children.map(child => child.props.fontFamily), ['Brand Serif,serif', 'Brand Serif,mono', 'serif']);
    await writeFile(join(root, 'theme.css'), '@font-face { font-family: Brand; src: url("./gone.ttf"); }');
    await assert.rejects(build(), /theme\.css:1: UI_FONT: gone\.ttf is declared by @font-face but missing/);
    await writeFile(join(root, 'theme.css'), '@font-face { font-family: Brand; src: url("../outside.ttf"); }');
    await assert.rejects(build(), /UI_FONT: \.\.\/outside\.ttf is outside the lxapp/);
    await writeFile(join(root, 'theme.css'), '@font-face { font-family: Brand; src: url("./brand.woff2"); }');
    await assert.rejects(build(), /UI_FONT: @font-face src is one url\(…\) of a bundled \.ttf, \.otf or \.ttc file/);
  });
});

test('a theme that redefines --font-sans re-fonts the whole page from its root', async () => {
  await project({
    'brand.ttf': 'face bytes',
    'theme.css': '@font-face { font-family: Brand; src: url("./brand.ttf"); }\n@theme { --font-sans: Brand, sans-serif; }',
    'page.tsx': `
      import './theme.css';
      export function render() { return <view><text>A</text><button>B</button></view>; }
    `,
  }, async (root, build) => {
    await build();
    assert.match(await load(root), /configureFonts\(\[\{"family":"Brand","src":"brand.ttf"\}\], "Brand, sans-serif"\)/);
    const page = tree(describePage());
    assert.equal(page.props.fontFamily, 'Brand,sans');
    assert.deepEqual(page.children.map(child => child.props.fontFamily), [undefined, undefined], 'children inherit natively');
    await writeFile(join(root, 'page.tsx'), `import './theme.css';\nexport function render() { return <view className="font-mono"><text>A</text></view>; }`);
    await build();
    await load(root);
    assert.equal(tree(describePage()).props.fontFamily, 'mono', "the root's own class wins");
    await writeFile(join(root, 'theme.css'), '@theme { --font-display: serif; }');
    await writeFile(join(root, 'page.tsx'), `import './theme.css';\nexport function render() { return <view><text>A</text></view>; }`);
    await build();
    assert.doesNotMatch(await load(root), /configureFonts\(\[/);
    assert.equal(tree(describePage()).props.fontFamily, undefined, 'the default stack leaves the engine default');
    await writeFile(join(root, 'theme.css'), '@theme { --font-sans: Missing, sans-serif; }');
    await assert.rejects(build(), /UI_FONT: --font-sans: unsupported font-family: Missing, sans-serif/);
  });
});

// The exact colour diagnostics the docs reference quotes.
test('page colours are roles: palette classes, dark: colours and hex literals fail naming the role to use', async () => {
  await project({ 'page.tsx': '' }, async (root, build) => {
    const fails = async (page, message) => {
      await writeFile(join(root, 'page.tsx'), page);
      await assert.rejects(build(), error => { assert.match(error.message, /page\.tsx:1:\d+: UI_COLOR/); assert.ok(error.message.includes(message), error.message); return true; });
    };
    await fails('export function render() { return <text className="text-gray-500">Hi</text>; }',
      'UI_COLOR: text-gray-500: not a role; use a role class such as bg-surface, text-muted, border-line or bg-accent text-on-accent');
    await fails('export function render() { return <view className="bg-surface dark:bg-surface-raised" />; }',
      'UI_COLOR: dark:bg-surface-raised: roles follow appearance; drop dark:');
    await fails(`export function render() { return <view style={{ backgroundColor: '#ffffff' }} />; }`,
      `UI_COLOR: "#ffffff" is not a role; backgroundColor uses a role such as 'surface', 'text-muted', 'line', 'accent' or 'accent/12'`);
    await writeFile(join(root, 'page.tsx'), 'export function render() { return <view className="bg-transparent dark:p-2 border-line-strong shadow-md" />; }');
    await build();
  });
});

test('renderer test fixtures may keep literal colours only through the build option', async () => {
  await project({ 'page.tsx': `export function render() { return <view className="bg-[#ff0000] dark:bg-white" style={{ borderColor: '#0000ff' }} />; }` }, async root => {
    await assert.rejects(buildPages(root, ['page.tsx'], join(root, 'dist')), /UI_COLOR/);
    await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { literalColors: true });
    await load(root);
    assert.equal(tree(describePage()).props.background, '#ff0000ff');
  });
});

test('controls a screen reader announces need a name, and the message names the fix', () => {
  for (const [source, pattern] of [
    ['<input placeholder="Date"/>', /UI_A11Y: <input> needs an accessible name; add aria-label="…" \(a placeholder is a hint, not a name\)/],
    ['<slider value={1}/>', /UI_A11Y: <slider> needs an accessible name/],
    ['<button onClick={f}><image src="x.png" alt=""/></button>', /UI_A11Y: <button> has no text to name it/],
    ['<checkbox value={v}/>', /UI_A11Y: <checkbox> has no text to name it/],
    ['<image src="x.png"/>', /UI_A11Y: <image> needs alt/],
  ]) flags(`export function render(){ return ${source}; }`, pattern);
  for (const source of [
    '<input aria-label="Date" placeholder="Pick a date"/>',
    '<button onClick={f}>Save</button>',
    '<button onClick={f}>{label}</button>',
    '<button onClick={f} aria-label="Close"><image src="x.png" alt=""/></button>',
    '<button onClick={f}><view className="flex-row"><text>Pay</text></view></button>',
    '<input {...props}/>',
    '<image src="x.png" alt=""/>',
  ]) clean(`export function render(){ return ${source}; }`);
});

test('build problems read top-down: the page first in line order, imports after, one line per repeated message', async () => {
  await project({
    'page.tsx': `import { Card } from './card';
export function render() {
  return <view className="itmes-center">
    <text className="cursor-pointer">A</text>
    <Card/>
    <text className="cursor-pointer">B</text>
    <button bogus={1}>Go</button>
  </view>;
}`,
    'card.tsx': 'export function Card() { return <view className="skew-x-3"><image src="a.png"/></view>; }',
  }, async (root, build) => {
    const lines = (await build().catch(error => error)).message.split('\n');
    assert.match(lines[0], /page.tsx: 5 authoring problems/);
    assert.deepEqual(lines.slice(1).map(line => line.match(/^([\w.]+):([\d:, ]+): (UI_\w+)/).slice(1, 4)), [
      ['page.tsx', '3:26', 'UI_CLASS_UNKNOWN'],
      ['page.tsx', '4:21, 6:21', 'UI_CLASS_UNSUPPORTED'],
      ['page.tsx', '7:13', 'UI_PROP'],
      ['card.tsx', '1:49', 'UI_CLASS_UNSUPPORTED'],
      ['card.tsx', '1:60', 'UI_A11Y'],
    ]);
  });
});

test('dev builds name the author line of a runtime error', async () => {
  await project({ 'page.tsx': `function Total({ items }) {
  const first = items[0];
  return <text>{first.price.toFixed(2)}</text>;
}
export function render() {
  return <view>
    <button onClick={() => {}}>Clear</button>
    <Total items={[{}]}/>
  </view>;
}` }, async root => {
    await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { dev: true });
    const source = await load(root);
    const inputs = JSON.parse(await readFile(join(root, '.lingxia/gpu-inputs.json'), 'utf8'));
    // Besides the page, only this workspace View's own sources.
    assert.deepEqual(inputs['page.tsx'].filter(path => !path.includes('/lingxia-view/src/')), ['page.tsx']);
    assert.match(source, /\nconfigureSourceMap\(\{/);
    const error = (() => { try { describePage(); } catch (error) { return error; } })();
    assert.match(error.message, /\n  at page.tsx:3:\d+\n  at Total \(page.tsx:8:5\)/);
    await buildPages(root, ['page.tsx'], join(root, 'dist'));
    assert.doesNotMatch(await readFile(join(root, 'dist/page.js'), 'utf8'), /\nconfigureSourceMap\(\{/);
    // A page deleted since its last dev build leaves the record, so no later edit plans it.
    await writeFile(join(root, 'gone.tsx'), 'export function render() { return <view />; }');
    await buildPages(root, ['gone.tsx'], join(root, 'dist'), {}, { dev: true });
    await rm(join(root, 'gone.tsx'));
    await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { dev: true });
    assert.deepEqual(Object.keys(JSON.parse(await readFile(join(root, '.lingxia/gpu-inputs.json'), 'utf8'))), ['page.tsx']);
    // The record is a cache: a torn one (two writers raced) is rebuilt, never fails the build.
    await writeFile(join(root, '.lingxia/gpu-inputs.json'), '{"page.tsx": ["page.tsx"]}\n]}\n');
    await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { dev: true });
    assert.deepEqual(Object.keys(JSON.parse(await readFile(join(root, '.lingxia/gpu-inputs.json'), 'utf8'))), ['page.tsx']);
    // Builds racing on one project leave a whole record and whole modules.
    await Promise.all(Array.from({ length: 6 }, () => buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { dev: true })));
    assert.deepEqual(Object.keys(JSON.parse(await readFile(join(root, '.lingxia/gpu-inputs.json'), 'utf8'))), ['page.tsx']);
    assert.deepEqual((await readdir(join(root, '.lingxia'))).filter(name => name.endsWith('.tmp')), []);
  });
});

test('UI_PERF warnings name literal row sets, per-row layers and oversized bundled images without stopping the build', async () => {
  const png = (width, height) => { const bytes = Buffer.alloc(33); bytes.writeUInt32BE(0x89504e47, 0); bytes.writeUInt32BE(width, 16); bytes.writeUInt32BE(height, 20); return bytes; };
  await project({
    'page.tsx': `import { List } from '@lingxia/view';
const tags = Array.from({ length: 60 }, (_, i) => 'tag' + i);
const chips = ['a', 'b', 'c'];
function Row({ label }) {
  return <view className="flex-row backdrop-blur-md hover:opacity-50"><text>{label}</text></view>;
}
export function render() {
  return <view>
    {tags.map(tag => <text key={tag}>{tag}</text>)}
    {Array.from({ length: 80 }, (_, i) => <text key={String(i)}>{i}</text>)}
    {chips.map(chip => <text key={chip} className="opacity-50">{chip}</text>)}
    <List items={tags} getKey={tag => tag} renderItem={tag => <Row label={tag} />} />
    <image src="huge.png" alt="" className="w-10 h-10" />
    <image src="small.png" alt="" className="w-10 h-10" />
    <image src="heavy.png" alt="" className="w-10 h-10" />
  </view>;
}`,
    'huge.png': png(4000, 3000), 'small.png': png(512, 512), 'heavy.png': Buffer.alloc(9 * 1048576),
  }, async (root, build) => {
    const { warnings } = await build();
    await load(root);
    const codes = warnings.map(line => line.match(/^page.tsx:([\d:, ]+): UI_PERF: (.*)$/).slice(1, 3));
    assert.deepEqual(codes.map(([place]) => place), ['5:26', '9:6', '10:6', '11:51', '13:16', '15:16']);
    assert.match(codes[0][1], /backdrop-blur-md composites a layer for every row/);
    assert.match(codes[1][1], /^60 rows built at once; above 50 use <List>/);
    assert.match(codes[2][1], /^80 rows/);
    assert.match(codes[3][1], /opacity-50 composites/);
    assert.match(codes[4][1], /huge.png is 4000×3000/);
    assert.match(codes[5][1], /heavy.png is 9.0 MiB; images above 8.0 MiB fail to load/);
  });
});

const ROW_PARTS = `import { List, cx } from '@lingxia/view';
const r = { tone: { bar: 'bg-surface', overlay: 'opacity-50' }, fade: 'opacity-0', chip: 'backdrop-blur-sm' };
function Controls({ variant = 'overlay', on }) {
  const overlay = variant === 'overlay';
  const tone = overlay ? 'overlay' : 'bar';
  if (!overlay) {
    return <view className={r.tone[tone]}><text>bar</text></view>;
  }
  return <view className={cx(r.fade, { 'opacity-25': on })}><text>overlay</text></view>;
}
function Chip({ flat }) {
  return <view className={flat ? 'bg-surface' : r.chip} />;
}
function Player({ variant, wide }) {
  return <view>
    <Controls variant={variant} />
    <dialog open={wide} aria-label="Full"><Controls variant="overlay" /></dialog>
    <view popover id="tip"><view className="opacity-75" /></view>
  </view>;
}
const items = ['a', 'b'];
`;

async function rowLayers(rows) {
  let found;
  await project({ 'page.tsx': `${ROW_PARTS}export function render() {\n  return <view>\n${rows.map(row => `    <List items={items} getKey={i => i} renderItem={i => ${row}} />\n`).join('')}  </view>;\n}` }, async (root, build) => {
    const { warnings } = await build();
    found = warnings.map(line => /UI_PERF: (\S+) composites/.exec(line)?.[1]).filter(Boolean).sort();
  });
  return found;
}

test('UI_PERF row layers follow the branch a literal prop selects; dialog and popover content is not row content', async () => {
  // variant="bar" passes through Player to Controls: the overlay branch and the map's overlay key never render.
  assert.deepEqual(await rowLayers(['<Player variant="bar" />', '<Chip flat />']), []);
  // An absent prop without a spread is undefined; a default fills it. Fully transparent draws
  // nothing, so `opacity-0` is no layer.
  assert.deepEqual(await rowLayers(['<Controls />']), []);
  assert.deepEqual(await rowLayers(['<Controls on />']), ['opacity-25']);
  // A view whose popover is false stays in the row.
  assert.deepEqual(await rowLayers(['<view popover={false} id="inline"><view className="opacity-80" /></view>']), ['opacity-80']);
});

test('UI_PERF row layers stay on for a prop the row computes and for a spread', async () => {
  assert.deepEqual(await rowLayers(['<Controls variant={i} on={i === "b"} />']), ['opacity-25', 'opacity-50']);
  assert.deepEqual(await rowLayers(['<Chip {...{ flat: i === "a" }} />']), ['backdrop-blur-sm']);
  assert.deepEqual(await rowLayers(['<Chip flat {...{ flat: i === "a" }} />']), ['backdrop-blur-sm']);
});

test('UI_STYLE_STATIC warns on a style object of fixed values only; computed and style-only values stay quiet', async () => {
  await project({
    'page.tsx': `import { useView } from '@lingxia/view';
export function render() {
  const { env } = useView();
  const wide = env.viewport.width > 600;
  return <view>
    <text style={{ fontSize: 18, color: 'text-muted' }}>fixed</text>
    <text style={{ fontSize: wide ? 22 : 18, color: 'accent' }}>computed</text>
    <view style={{ positionArea: 'bottom' }} id="tip" popover><text>tip</text></view>
    <view style={{ width: \`\${env.viewport.width / 2}px\` }} />
  </view>;
}`,
  }, async (root, build) => {
    const { warnings } = await build();
    await load(root);
    assert.deepEqual(warnings.map(line => line.replace(/: UI_STYLE_STATIC: .*/, '')), ['page.tsx:6:11']);
    assert.match(warnings[0], /write it as Tailwind classes in className and keep style for computed values/);
  });
});

test('an element its props make a keyboard stop takes focus-visible:, and a stepped button is not inert', async () => {
  await project({
    'page.tsx': `import { useLocal } from '@lingxia/view';
export function render() {
  const at = useLocal('at', 2);
  return <view>
    <view onPointer={() => {}} aria-label="Month" aria-valuenow={at.value} aria-valuemax={11} className="h-10 focus-visible:bg-danger" />
    <view drag="x" dragSnapPoints={[0, 40]} aria-label="Card" className="w-10 h-10 focus-visible:bg-danger" />
    <button onPointer={() => {}} aria-label="Level" aria-valuenow={at.value}>Level</button>
  </view>;
}`,
  }, async (root, build) => {
    const { warnings } = await build();
    await load(root);
    assert.deepEqual(warnings.filter(line => line.includes('UI_INERT')), []);
  });
});

test('UI_INERT warns on a control with no handler, bind or disabled; invokers, menu items and spreads stay quiet', async () => {
  await project({
    'page.tsx': `import { useLocal } from '@lingxia/view';
export function render() {
  const on = useLocal('on', false);
  const more = { onClick() {} };
  return <view>
    <select aria-label="Size" options={[{ value: 's', label: 'S' }]} />
    <button>Idle</button>
    <switch aria-label="Wifi" bind={on} />
    <button disabled>Off</button>
    <button popoverTarget="tip">Open</button>
    <button {...more}>Spread</button>
    <view id="tip" popover><text>tip</text></view>
  </view>;
}`,
  }, async (root, build) => {
    const { warnings } = await build();
    await load(root);
    const inert = warnings.filter(line => line.includes('UI_INERT'));
    assert.deepEqual(inert.map(line => line.replace(/: UI_INERT: .*/, '')), ['page.tsx:6:5', 'page.tsx:7:5']);
    assert.match(inert[0], /this <select> does nothing: add onChange or bind, or mark it disabled/);
    assert.match(inert[1], /this <button> does nothing: add onClick, or mark it disabled/);
  });
});

test('a component keyed by its own function name fails the build: release builds rename functions', () => {
  flags('function A(){return <text>a</text>} const parts=[A]; export function render(){ return <view>{parts.map(Part => <Part key={Part.name} />)}</view>; }',
    /UI_KEY: Part\.name is not a stable key; release builds rename functions, so key by a label or index/);
  clean('function A(){return <text>a</text>} const parts=[A]; export function render(){ return <view>{parts.map((Part, i) => <Part key={String(i)} />)}</view>; }');
});

test('top-level components get a compile-time id from their module path and declaration name', () => {
  const out = identify('export function Card(){} const Row = () => null; function helper(){} let Late = () => null;', 'pages/a/index.tsx', 'pages/a/index.tsx');
  assert.match(out, /Card\.__lxid = "pages\/a\/index#Card";/);
  assert.match(out, /Row\.__lxid = "pages\/a\/index#Row";/);
  assert.doesNotMatch(out, /helper\.__lxid|Late\.__lxid/);
});

test('a context gets a compile-time id only as a module-level const', () => {
  const out = identify("import { createContext } from '@lingxia/view';\nexport const TabsContext = createContext(null);\nconst size = createContext('md');", 'pkg/tabs.tsx', 'pkg/tabs.tsx');
  assert.match(out, /TabsContext\.__lxid = "pkg\/tabs#TabsContext";/);
  assert.match(out, /size\.__lxid = "pkg\/tabs#size";/);
  clean("import { createContext } from '@lingxia/view';\nexport const A = createContext(0);");
  flags("import { createContext } from '@lingxia/view';\nfunction make() { const B = createContext(0); return B; }", /UI_CONTEXT: createContext belongs at module top level/);
  flags("import { createContext } from '@lingxia/view';\nlet C = createContext(0);", /UI_CONTEXT/);
  flags("import { createContext } from '@lingxia/view';\nexport const D = { ctx: createContext(0) };", /UI_CONTEXT/);
});

test('a pressable control around a single control fails the build; controls beside content are fine', () => {
  flags('export function render(){ return <button onClick={() => {}}><switch value={true} onChange={() => {}} aria-label="x" /></button>; }',
    /UI_BUTTON: <button> holds only a <switch>; put the action on that control/);
  clean('export function render(){ return <button onClick={() => {}}><view className="flex-row"><text>Wi-Fi</text><switch value={true} onChange={() => {}} aria-label="Wi-Fi" /></view></button>; }');
});

test('closures a render hands on keep their identity while their captures are unchanged', () => {
  const source = `import { List } from '@lingxia/view';
const label = (i: number) => String(i);
let mutable = 1;
export function render({ data }) {
  const local = data.n;
  let count = 0; count++;
  const run = () => local;
  const self = () => self();
  return <view>
    <List items={a} getKey={String} renderItem={i => <text>{label(i) + Math.max(i, 0)}</text>} />
    <List items={a} getKey={String} renderItem={i => <text>{String(local)}</text>} />
    <List items={a} getKey={String} renderItem={i => <text>{String(mutable)}</text>} />
    <List items={a} getKey={String} renderItem={function (i) { return <text>{String(this)}</text>; }} />
    <button onClick={data.ok ? () => count : run}>Go</button>
  </view>;
}`;
  const out = stabilizeClosures(source, 'pages/a/index.tsx', 'pages/a/index.tsx');
  const wrapped = [...out.matchAll(/__lxFn\("[^"]+", \((.*?)\), \[([^\]]*)\]\)/g)].map(m => [m[1].slice(0, 12), m[2]]);
  assert.deepEqual(wrapped, [['() => local', 'local'], ['i => <text>{', ''], ['i => <text>{', 'local'], ['i => <text>{', 'mutable']]);
  assert.match(out, /renderItem=\{function \(i\)/, 'this: identity cannot stand for it');
  assert.match(out, /const self = \(\) => self\(\)/, 'a capture not yet initialised');
  assert.match(out, /data\.ok \? \(\) => count : run/, 'a reassigned capture');
});

test('classes in a map checked with `satisfies Record<…>` are traced like any other map', async () => {
  const source = (map) => `type V = 'a' | 'b';\nconst m = { v: ${map} } as const;\nexport function render() { const k: V = 'a'; return <page className={m.v[k]} />; }\n`;
  for (const map of ["{ a: 'p-4', b: 'bg-acent' } satisfies Record<V, string>", "{ a: 'p-4', b: 'bg-acent' } satisfies Partial<Record<V, string>>"]) {
    await project({ 'page.tsx': source(map) }, async (root, build) => {
      assert.match((await build().catch(error => error)).message ?? '', /page\.tsx:2:\d+: UI_CLASS_UNKNOWN: bg-acent/, map);
    });
  }
});
