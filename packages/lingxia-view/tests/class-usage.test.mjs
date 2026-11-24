import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { collectClassAnalysis, traceClasses } from '../compiler/class-usage.mjs';
import { moduleChecker, parseSource, sourceChecker } from '../compiler/source.mjs';

// Traces every className in one page module: the class words it can see, and why it cannot follow the rest.
function analyse(source) {
  const file = parseSource('page.tsx', `import { cx } from '@lingxia/view';\n${source}`);
  return summary(collectClassAnalysis([file], sourceChecker(file)));
}

// The same over several modules that import each other.
function analyseFiles(files) {
  const dir = mkdtempSync(join(tmpdir(), 'lingxia-view-class-usage-'));
  try {
    const parsed = Object.entries(files).map(([name, text]) => {
      writeFileSync(join(dir, name), text);
      return parseSource(join(dir, name), text);
    });
    return summary(collectClassAnalysis(parsed, moduleChecker(parsed)));
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

function summary({ usages, untraced }) {
  return {
    classes: [...new Set(usages.flatMap(usage => usage.value.split(/\s+/)).filter(Boolean))].sort(),
    on: usages.map(usage => `${usage.tag} ${usage.value}`),
    complete: usages.every(usage => usage.complete),
    untraced: untraced.map(({ expression, reason }) => `${expression}: ${reason}`),
  };
}

const traced = [
  ['a literal', `<view className="p-1 m-2"/>`, ['m-2', 'p-1']],
  ['a const', `const a = 'p-1'; <view className={a}/>`, ['p-1']],
  ['a conditional', `const on = Math.random() > 0.5; <view className={on ? 'p-1' : 'p-2'}/>`, ['p-1', 'p-2']],
  ['&&, || and ??', `const on = Math.random() > 0.5; <view className={(on && 'p-1') || 'p-2'}/>; <view className={undefined ?? 'p-3'}/>`, ['p-1', 'p-2', 'p-3']],
  ['cx with arrays and maps', `const on = Math.random() > 0.5; <view className={cx('p-1', ['m-1'], { italic: on, 'p-2': on })}/>`, ['italic', 'm-1', 'p-1', 'p-2']],
  ['a cx map passed by name', `const on = Math.random() > 0.5; const map = { italic: on, underline: !on }; <view className={cx('p-1', map)}/>`, ['italic', 'p-1', 'underline']],
  ['a template with whole words', 'const a = "p-1"; <view className={`m-1 ${a} p-2`}/>', ['m-1', 'p-1', 'p-2']],
  ['a template over known values', 'const n = 4; const on = Math.random() > 0.5; <view className={`gap-${n} p-${on ? 1 : 2}`}/>', ['gap-4', 'p-1', 'p-2']],
  ['a const map member', `const R = { a: 'p-1', b: { c: 'p-2' } } as const; <view className={cx(R.a, R.b.c)}/>`, ['p-1', 'p-2']],
  ['a const map selected by a literal key', `const R = { a: 'p-1', b: 'p-2' }; <view className={R['b']}/>`, ['p-2']],
  ['a const map selected by a key of a literal type', `const R = { a: 'p-1', b: 'p-2', nested: { c: 'p-3' } }; const k: 'a' | 'b' = Math.random() > 0.5 ? 'a' : 'b'; <view className={R[k]}/>`, ['p-1', 'p-2']],
  ['a shorthand member', `const a = 'p-1'; const R = { a }; <view className={R.a}/>`, ['p-1']],
  ['a spread of a const object', `const extra = { className: 'p-1' }; <view {...extra}/>`, ['p-1']],
  ['a member of a written table under a wider type', `const R: Record<string, { tone: string }> = { a: { tone: 'p-1' } }; <view className={R.a.tone}/>`, ['p-1']],
  ['a member of each object a conditional picks', `const R = { a: { on: 'p-1' }, b: { on: 'p-2' } }; const t = Math.random() > 0.5 ? R.a : R.b; <view className={t.on}/>`, ['p-1', 'p-2']],
  ['an array destructured from a const', `const [a, b] = ['p-1', 'p-2']; <view className={b}/>`, ['p-2']],
  ['a prop destructured in the parameter list', `function Part({ className }: { className?: string }) { return <view className={className}/>; } export const A = () => <Part className="p-1"/>;`, ['p-1']],
  ['a prop read from the props parameter', `function Part(props: { className?: string }) { return <view className={props.className}/>; } export const A = () => <Part className="p-1"/>;`, ['p-1']],
  ['a prop destructured in the body', `function Part(props: { className?: string }) { const { className } = props; return <view className={className}/>; } export const A = () => <Part className="p-1"/>;`, ['p-1']],
  ['a prop with a default', `function Part({ tone = 'p-2' }: { tone?: string }) { return <view className={tone}/>; } export const A = () => <Part tone="p-1"/>;`, ['p-1', 'p-2']],
  ['a rest binding read and spread', `function Part(props: { id: string; className?: string }) { const { id, ...rest } = props; return <view className={rest.className} {...rest}/>; } export const A = () => <Part id="a" className="p-1"/>;`, ['p-1']],
  ['a prop passed through two parts', `function Leaf({ className }: { className?: string }) { return <view className={className}/>; } function Mid({ tone }: { tone?: string }) { return <Leaf className={tone}/>; } export const A = () => <Mid tone="p-1"/>;`, ['p-1']],
  ['a part that renders itself', `function Tree({ className, depth }: { className?: string; depth: number }): any { return <view className={className}>{depth ? <Tree className={className} depth={depth - 1}/> : null}</view>; } export const A = () => <Tree className="p-1" depth={2}/>;`, ['p-1']],
  ['props spread at the call site', `function Part({ className }: { className?: string }) { return <view className={className}/>; } const props = { className: 'p-1' }; export const A = () => <Part {...props}/>;`, ['p-1']],
  ['a recipe passed as a prop', `const R = { box: { base: 'p-1' } }; function Part({ recipe: r }: { recipe: typeof R }) { return <view className={r.box.base}/>; } export const A = () => <Part recipe={R}/>;`, ['p-1']],
  ['a helper and its arguments', `function tone(kind: 'ok' | 'bad') { return kind === 'ok' ? 'p-1' : 'p-2'; } <view className={tone('ok')}/>`, ['p-1', 'p-2']],
  ['a helper returning its parameter', `const pick = (c: string) => c; <view className={pick('p-1')}/>; <view className={pick('p-2')}/>`, ['p-1', 'p-2']],
  ['the element of a map callback over a const array', `const ROWS = [{ cls: 'p-1' }, { cls: 'p-2' }] as const; <view>{ROWS.map(row => <view className={row.cls}/>)}</view>`, ['p-1', 'p-2']],
  ['a part attached as a member of another', `export function Root() { return <view/>; } function Part({ className }: { className?: string }) { return <view className={className}/>; } Root.Part = Part; export const A = () => <Root.Part className="p-1"/>;`, ['p-1']],
  ['a part held where it cannot be called', `const held = new Set<unknown>(); function hold(c: unknown) { held.add(c); } function Part({ className }: { className?: string }) { return <view className={className}/>; } hold(Part); export const A = () => <Part className="p-1"/>;`, ['p-1']],
];
for (const [name, source, classes] of traced)
  test(`class tracing follows ${name}`, () => {
    const result = analyse(source);
    assert.deepEqual(result.untraced, []);
    assert.deepEqual(result.classes, classes);
    assert.ok(result.complete);
  });

test('a class passed through props is checked on the element it lands on', () => {
  const result = analyse(`function Part({ className }: { className?: string }) { return <text className={className}/>; } export const A = () => <Part className="p-1"/>;`);
  assert.deepEqual(result.on.sort(), ['Part p-1', 'text p-1']);
});

test('class tracing follows a default-exported part imported under another name', () => {
  const result = analyseFiles({
    'part.tsx': `export default function Part({ className }: { className?: string }) { return <view className={className}/>; }`,
    'page.tsx': `import Thing from './part'; export const A = () => <Thing className="p-1"/>;`,
  });
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.on.sort(), ['Thing p-1', 'view p-1']);
});

test('class tracing follows a part through a re-export and a renamed import', () => {
  const result = analyseFiles({
    'part.tsx': `export function Part({ className }: { className?: string }) { return <view className={className}/>; }`,
    'index.ts': `export { Part } from './part';`,
    'page.tsx': `import { Part as P } from './index'; export const A = () => <P className="p-1"/>;`,
  });
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.on.sort(), ['P p-1', 'view p-1']);
});

test('undefined, null, false and an empty string are no class, not untraced', () => {
  const result = analyse(`const on = Math.random() > 0.5; <view className={cx(undefined, null, false, '', on ? undefined : 'p-1')}/>; <view className={on ? null : ''}/>`);
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.classes, ['p-1']);
});

const untraced = [
  ["the page's View", `export function render(view: { data: { cls: string } }) { return <view className={view.data.cls}/>; }`, /`view` comes from the page's View, known only at run time/],
  ['a part also used as a value', `function Part({ className }: { className?: string }) { return <view className={className}/>; } const Other = Part; export const A = () => <Other className="p-1"/>;`, /is a parameter of a function that is also used as a value \(.*page\.tsx:\d+:\d+\), so its callers are not known/],
  ['a callback the runtime calls', `function Part(props: { onTone: (t: string) => void }) { return null; } export const A = () => <Part onTone={tone => <view className={tone}/>}/>;`, /`tone` is a parameter of a function that is also used as a value/],
  ['a let binding', `let a = 'p-1'; <view className={a}/>`, /`a` is a `let` binding, which can be reassigned/],
  ['a call it cannot follow', `const parts = ['p-1', 'p-2']; <view className={parts.join(' ')}/>`, /`parts\.join\(' '\)` is the result of a call the build does not follow/],
  ['concatenation', `const n = 4; <view className={'gap-' + n}/>`, /`'gap-' \+ n` is computed with `\+` at run time/],
  ['a template hole of unknown value', 'function Part({ n }: { n: number }) { return <view className={`gap-${n}`}/>; } export const A = (p: { n: number }) => <Part n={Math.round(p.n)}/>;', /`n` is joined into a class name at run time, and its values are not known to the build/],
  ['a template past the combination limit', 'const B = { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8, i: 9 }; declare const k: keyof typeof B; <view className={`p-${B[k]}${B[k]}${B[k]}`}/>', /expands to 729 class names \(limit 64\)/],
  ['an array element', `const A = ['p-1', 'p-2']; const i = Math.round(Math.random()); <view className={A[i]}/>`, /`A\[i\]` is an array element/],
  ['a member of a declared value', `declare const R: { a: { tone: string } }; <view className={R.a.tone}/>`, /`R` is declared without a value/],
  ['a member of an object built at run time', `const R = Object.assign({}, { a: 'p-1' }); <view className={R.a}/>`, /`Object\.assign\(\{\}, \{ a: 'p-1' \}\)` is the result of a call the build does not follow/],
  ['an object where class text belongs', `const R = { pad: { a: 'p-1' }, b: 'p-2' }; const k = String(Math.random()) as keyof typeof R | 'pad'; <view className={cx(R[k as string as 'b'], (R as Record<string, unknown>)[String(k)] as string)}/>`, /is an object, not class text/],
  ['a spread whose className is only declared', `declare const extra: { className: string }; <view {...extra}/>`, /the spread `extra` may carry a className: `extra` is declared without a value/],
  ['an unresolved name', `<view className={missing}/>`, /`missing` cannot be resolved/],
  ['a spread of an unknown object in a cx map', `declare const more: { italic: boolean }; <view className={cx({ ...more })}/>`, /`more` is declared without a value/],
];
for (const [name, source, reason] of untraced)
  test(`class tracing reports ${name} as untraced`, () => {
    const result = analyse(source);
    assert.ok(result.untraced.some(line => reason.test(line)), result.untraced.join('\n') || 'nothing untraced');
  });

test('a spread typed with only an index signature carries no className', () => {
  const result = analyse(`function dataOf(props: object): Record<string, unknown> { return {}; } function Part(props: object) { return <view {...dataOf(props)} className="p-1"/>; } <view {...dataOf({})}/>`);
  assert.deepEqual(result.untraced, []);
});

test('a spread of a Record at a call site writes no className prop', () => {
  const result = analyse(`function Part({ className }: { className?: string }) { return <view className={cx('p-1', className)}/>; } declare const edits: Record<string, string>; export const A = () => <Part {...edits}/>;`);
  assert.deepEqual(result.untraced, []);
});

test('class tracing reports a failure of its own instead of throwing', () => {
  const file = parseSource('page.tsx', `const a = 'p-1';\n<view className={a}/>`);
  const failing = new Proxy(sourceChecker(file), { get: () => () => { throw Error('boom'); } });
  const expression = file.statements[1].expression.attributes.properties[0].initializer.expression;
  const result = traceClasses(failing, expression);
  assert.deepEqual(result.pieces, []);
  assert.match(result.untraced[0].reason, /^the class tracer failed on `a`: boom$/);
  assert.ok(ts.isIdentifier(result.untraced[0].node));
});

test('a computed key checks every member of a map of class text, and of a map indexed again', () => {
  const result = analyse(`function Part({ tone, size }: { tone: 'a' | 'b'; size: 's' | 'm' }) { const R = { a: 'p-1', b: 'p-2' }; const N = { a: { s: 'm-1', m: 'm-2' }, b: { s: 'm-3', m: 'm-4' } }; return <view className={cx(R[tone], N[tone][size])}/>; } export const A = () => <Part tone="a" size="s"/>;`);
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.classes, ['m-1', 'm-2', 'm-3', 'm-4', 'p-1', 'p-2']);
});

test('a computed key over a map holding an object reads only the members its values name', () => {
  const result = analyse(`function Part({ k }: { k: 'a' | 'b' | 'nested' }) { const R = { a: 'p-1', b: 'p-2', nested: { c: 'p-3' } }; return <view className={R[k]}/>; } export const A = () => <Part k="b"/>;`);
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.classes, ['p-2']);
});

// Two builds over the same parsed files share answers; a changed caller is a new parse, and its answers are new.
test('class answers kept from an earlier build follow a caller that changed', () => {
  const dir = mkdtempSync(join(tmpdir(), 'lingxia-view-class-usage-'));
  try {
    const parse = (name, text) => { writeFileSync(join(dir, name), text); return parseSource(join(dir, name), text); };
    const part = parse('part.tsx', `export function Part({ className }: { className?: string }) { return <view className={className}/>; }`);
    const run = files => summary(collectClassAnalysis(files, moduleChecker(files)));
    const before = parse('page.tsx', `import { Part } from './part'; export const A = () => <Part className="p-1"/>;`);
    assert.deepEqual(run([part, before]).on.sort(), ['Part p-1', 'view p-1']);
    assert.deepEqual(run([part, before]).on.sort(), ['Part p-1', 'view p-1']);
    const after = parse('page.tsx', `import { Part } from './part'; export const A = () => <Part className="p-2"/>;`);
    assert.deepEqual(run([part, after]).on.sort(), ['Part p-2', 'view p-2']);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('class answers kept from an earlier build follow an imported value that changed', () => {
  const dir = mkdtempSync(join(tmpdir(), 'lingxia-view-class-usage-'));
  try {
    const parse = (name, text) => { writeFileSync(join(dir, name), text); return parseSource(join(dir, name), text); };
    const run = files => summary(collectClassAnalysis(files, moduleChecker(files))).classes;
    const page = parse('page.tsx', `import { R } from './index'; export const A = () => <view className={R.a}/>;`);
    const index = parse('index.ts', `export { R } from './recipe';`);
    assert.deepEqual(run([page, index, parse('recipe.ts', `export const R = { a: 'p-1' };`)]), ['p-1']);
    assert.deepEqual(run([page, index, parse('recipe.ts', `export const R = { a: 'p-2' };`)]), ['p-2']);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('an empty className expression is no class and no failure', () => {
  const result = analyse(`<view className={}/>`);
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.classes, []);
});
