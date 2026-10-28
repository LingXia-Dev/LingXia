import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { audit, buildPages, identify } from '../compiler/index.mjs';
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
  flags('export const render = () => (<><text>Hello</text></>);', /UI_FRAGMENT/);
  clean('export function render(){return <page data-state="open" className="data-[state=open]:p-4"><text>x</text></page>}');
  flags('export function render(){return <page dataState="open"><text>x</text></page>}', /UI_PROP/);
  clean('function Rows(){return <><text>a</text><text>b</text></>} export function render(){return <page><Rows/><>{[1].map(n => <text key={String(n)}>x</text>)}</></page>}');
});

test('spread and explicit props obey one element contract', () => {
  for (const source of ['const view = <text {...{typo:true}}>Hello</text>;', 'const props = {typo:true}; const view = <text {...props}/>;',
    'const view = <search {...{secure:true}}/>;']) flags(source, /UI_PROP/);
  clean('const props = {children:"Hello",className:"text-sm"}; const view = <text {...props}/>;');
  clean('const view = <list count={keys.length} itemKeys={keys} row={renderRow}/>;');
  flags('const view = <scroll itemKeys={keys}/>;', /UI_PROP: itemKeys is not a property of <scroll>/);
  clean('const x = <input bind={draft} accessibilityLabel="Note" placeholder="Write" keyboard="text" autoFocus onSubmit={save} disabled/>;');
  clean('const x = <textarea accessibilityLabel="Note" value="" onInput={change} disabled={false}/>;');
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
  clean('const x=<column style={{display:"grid",gridTemplateColumns:3}}/>;');
  flags('const x=<text style={{gridTemplateColumns:3}}>A</text>;', /UI_STYLE: gridTemplateColumns is not supported on <text>/);
  flags('const x=<text style={{offset:{x:1}}}/>;', /UI_STYLE: unknown style property offset/);
  flags('const view = <column style={{animation:null}}/>;', /UI_ANIMATION/);
});

test('class diagnostics separate unknown, unsupported and wrong-element utilities and report all at once', async () => {
  await project({ 'page.tsx': `export function render(view) {
  return <column className="grid-cols-2 not-a-utility transition-[width]">
    <text className={view.data.on ? "grid-cols-2" : "items-center"}>A</text>
    <column className="active:bg-danger hover:p-4"/>
  </column>;
}` }, async (root, build) => {
    const error = await build().catch(error => error);
    const lines = error.message.split('\n');
    assert.match(lines[0], /page.tsx: 6 authoring problems/);
    for (const pattern of [
      /^page.tsx:3:\d+: UI_CLASS_ELEMENT: grid-cols-2 sets gridTemplateColumns, which <text> does not support$/,
      /^page.tsx:2:\d+: UI_CLASS_UNKNOWN: not-a-utility is not a Tailwind utility$/,
      /^page.tsx:2:\d+: UI_CLASS_UNSUPPORTED: transition-\[width\]: the native renderer does not support transition of width animates layout/,
      /^page.tsx:3:\d+: UI_CLASS_ELEMENT: items-center sets alignItems, which <text> does not support$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: active:bg-danger needs a control; <column> is never pressed$/,
      /^page.tsx:4:\d+: UI_CLASS_ELEMENT: hover:p-4: hover styles change paint only, not paddingTop, paddingRight, paddingBottom, paddingLeft$/,
    ]) assert.ok(lines.some(line => pattern.test(line)), `${pattern} in\n${error.message}`);
    await assert.rejects(readFile(join(root, 'dist/page.js')), { code: 'ENOENT' });
  });
});

test('flex-only utilities on a grid name the grid equivalent', async () => {
  await project({ 'page.tsx': 'export function render(){return <grid className="flex-wrap flex-col items-center content-between justify-items-center"/>}' }, async (root, build) => {
    const message = (await build().catch(error => error)).message;
    assert.match(message, /flex-wrap sets flexWrap, which <grid> does not support; grid items wrap into rows by themselves/);
    assert.match(message, /flex-col sets flexDirection, which <grid> does not support; use grid-flow-col/);
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
  await project({ 'page.tsx': 'export function render(){return <column className="itmes-center md:roundd-lg tabluar-nums zzqqxx"/>}' }, async (root, build) => {
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

test('page builds reject invalid keyframes and inline animation before installing a module', async () => {
  await project({ 'page.tsx': 'export function render(){return <column style={{ animation: "spin 1s" }}><text>Pending</text></column>;}' }, async (root, build) => {
    await assert.rejects(build(), /UI_ANIMATION: inline animation is not supported; use animate-\* utilities/);
    await assert.rejects(readFile(join(root, 'dist/page.js')), { code: 'ENOENT' });
    await writeFile(join(root, 'tokens.css'), '@theme {--animate-enter:enter 1s;@keyframes enter{from{width:0px}to{width:10px}}}');
    await writeFile(join(root, 'page.tsx'), 'import "./tokens.css";export function render(){return <column className="animate-enter"/>;}');
    await assert.rejects(build(), /UI_ANIMATION|UI_CLASS_UNSUPPORTED/);
    await writeFile(join(root, 'tokens.css'), '@theme {--animate-enter:enter 1s;@keyframes enter{from{opacity:0}to{opacity:1}}}');
    await build();
  });
});

test('compiler emits executable ESM UI, carries the action catalog and preserves page config', async () => {
  await project({
    'page.json': '{"renderer":"gpu","title":"Hello"}',
    'page.tsx': `export function render() { return <column>
      <stack><text>Back</text><text style={{position:"absolute",left:12,top:-4}}>Front</text></stack>
      <grid style={{gridTemplateColumns:2}}><text style={{lineClamp:2}}>Hello, world!</text><text>Second card</text></grid>
      <search accessibilityLabel="Search" placeholder="Find a moment"/><slider accessibilityLabel="Volume" value={40}/>
      <radio accessibilityLabel="Repeat" value="daily" options={[{value:"daily",label:"Daily"},{value:"weekly",label:"Weekly"}]} />
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
    assert.equal(records.find(record => record.tag === 'grid').props.gridTemplateColumns, 'repeat(2, minmax(0, 1fr))');
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
    assert.deepEqual([light.color, light.fontSize, light.lineHeight], ['accent', 17, 17 * (1.25 / .875)]);
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
      export function render() { return <column><text className="font-brand">A</text><text style={{ fontFamily: 'Brand Serif, monospace' }}>B</text><text className="font-serif">C</text></column>; }
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
    ['<input placeholder="Date"/>', /UI_A11Y: <input> needs an accessible name; add accessibilityLabel="…" \(a placeholder is a hint, not a name\)/],
    ['<slider value={1}/>', /UI_A11Y: <slider> needs an accessible name/],
    ['<button onClick={f}><image src="x.png" alt=""/></button>', /UI_A11Y: <button> has no text to name it/],
    ['<checkbox value={v}/>', /UI_A11Y: <checkbox> has no text to name it/],
    ['<image src="x.png"/>', /UI_A11Y: <image> needs alt/],
  ]) flags(`export function render(){ return ${source}; }`, pattern);
  for (const source of [
    '<input accessibilityLabel="Date" placeholder="Pick a date"/>',
    '<button onClick={f}>Save</button>',
    '<button onClick={f}>{label}</button>',
    '<button onClick={f} accessibilityLabel="Close"><image src="x.png" alt=""/></button>',
    '<button onClick={f}><row><text>Pay</text></row></button>',
    '<input {...props}/>',
    '<image src="x.png" alt=""/>',
  ]) clean(`export function render(){ return ${source}; }`);
});

test('build problems read top-down: the page first in line order, imports after, one line per repeated message', async () => {
  await project({
    'page.tsx': `import { Card } from './card';
export function render() {
  return <column className="itmes-center">
    <text className="cursor-pointer">A</text>
    <Card/>
    <text className="cursor-pointer">B</text>
    <button bogus={1}>Go</button>
  </column>;
}`,
    'card.tsx': 'export function Card() { return <column className="skew-x-3"><image src="a.png"/></column>; }',
  }, async (root, build) => {
    const lines = (await build().catch(error => error)).message.split('\n');
    assert.match(lines[0], /page.tsx: 5 authoring problems/);
    assert.deepEqual(lines.slice(1).map(line => line.match(/^([\w.]+):([\d:, ]+): (UI_\w+)/).slice(1, 4)), [
      ['page.tsx', '3:28', 'UI_CLASS_UNKNOWN'],
      ['page.tsx', '4:21, 6:21', 'UI_CLASS_UNSUPPORTED'],
      ['page.tsx', '7:13', 'UI_PROP'],
      ['card.tsx', '1:51', 'UI_CLASS_UNSUPPORTED'],
      ['card.tsx', '1:62', 'UI_A11Y'],
    ]);
  });
});

test('dev builds name the author line of a runtime error', async () => {
  await project({ 'page.tsx': `function Total({ items }) {
  const first = items[0];
  return <text>{first.price.toFixed(2)}</text>;
}
export function render() {
  return <column>
    <button onClick={() => {}}>Clear</button>
    <Total items={[{}]}/>
  </column>;
}` }, async root => {
    await buildPages(root, ['page.tsx'], join(root, 'dist'), {}, { dev: true });
    const source = await load(root);
    const inputs = JSON.parse(await readFile(join(root, '.lingxia/gpu-inputs.json'), 'utf8'));
    assert.deepEqual(inputs['page.tsx'], ['page.tsx']);
    assert.match(source, /\nconfigureSourceMap\(\{/);
    const error = (() => { try { describePage(); } catch (error) { return error; } })();
    assert.match(error.message, /\n  at page.tsx:3:\d+\n  at Total \(page.tsx:8:5\)/);
    await buildPages(root, ['page.tsx'], join(root, 'dist'));
    assert.doesNotMatch(await readFile(join(root, 'dist/page.js'), 'utf8'), /\nconfigureSourceMap\(\{/);
  });
});

test('UI_PERF warnings name literal row sets, per-row layers and oversized bundled images without stopping the build', async () => {
  const png = (width, height) => { const bytes = Buffer.alloc(33); bytes.writeUInt32BE(0x89504e47, 0); bytes.writeUInt32BE(width, 16); bytes.writeUInt32BE(height, 20); return bytes; };
  await project({
    'page.tsx': `import { List } from '@lingxia/ui';
const tags = Array.from({ length: 60 }, (_, i) => 'tag' + i);
const chips = ['a', 'b', 'c'];
function Row({ label }) {
  return <row className="backdrop-blur-md hover:opacity-50"><text>{label}</text></row>;
}
export function render() {
  return <column>
    {tags.map(tag => <text key={tag}>{tag}</text>)}
    {Array.from({ length: 80 }, (_, i) => <text key={String(i)}>{i}</text>)}
    {chips.map(chip => <text key={chip} className="opacity-50">{chip}</text>)}
    <List items={tags} getKey={tag => tag} renderItem={tag => <Row label={tag} />} />
    <image src="huge.png" alt="" className="w-10 h-10" />
    <image src="small.png" alt="" className="w-10 h-10" />
    <image src="heavy.png" alt="" className="w-10 h-10" />
  </column>;
}`,
    'huge.png': png(4000, 3000), 'small.png': png(512, 512), 'heavy.png': Buffer.alloc(9 * 1048576),
  }, async (root, build) => {
    const { warnings } = await build();
    await load(root);
    const codes = warnings.map(line => line.match(/^page.tsx:([\d:, ]+): UI_PERF: (.*)$/).slice(1, 3));
    assert.deepEqual(codes.map(([place]) => place), ['5:25', '9:6', '10:6', '11:51', '13:16', '15:16']);
    assert.match(codes[0][1], /backdrop-blur-md composites a layer for every row/);
    assert.match(codes[1][1], /^60 rows built at once; above 50 use <List>/);
    assert.match(codes[2][1], /^80 rows/);
    assert.match(codes[3][1], /opacity-50 composites/);
    assert.match(codes[4][1], /huge.png is 4000×3000/);
    assert.match(codes[5][1], /heavy.png is 9.0 MiB; images above 8.0 MiB fail to load/);
  });
});

test('a component keyed by its own function name fails the build: release builds rename functions', () => {
  flags('function A(){return <text>a</text>} const parts=[A]; export function render(){ return <column>{parts.map(Part => <Part key={Part.name} />)}</column>; }',
    /UI_KEY: Part\.name is not a stable key; release builds rename functions, so key by a label or index/);
  clean('function A(){return <text>a</text>} const parts=[A]; export function render(){ return <column>{parts.map((Part, i) => <Part key={String(i)} />)}</column>; }');
});

test('top-level components get a compile-time id from their module path and declaration name', () => {
  const out = identify('export function Card(){} const Row = () => null; function helper(){} let Late = () => null;', 'pages/a/index.tsx', 'pages/a/index.tsx');
  assert.match(out, /Card\.__lxid = "pages\/a\/index#Card";/);
  assert.match(out, /Row\.__lxid = "pages\/a\/index#Row";/);
  assert.doesNotMatch(out, /helper\.__lxid|Late\.__lxid/);
});

test('a pressable control around a single control fails the build; controls beside content are fine', () => {
  flags('export function render(){ return <button onClick={() => {}}><switch value={true} onChange={() => {}} accessibilityLabel="x" /></button>; }',
    /UI_BUTTON: <button> holds only a <switch>; put the action on that control/);
  clean('export function render(){ return <button onClick={() => {}}><row><text>Wi-Fi</text><switch value={true} onChange={() => {}} accessibilityLabel="Wi-Fi" /></row></button>; }');
});
