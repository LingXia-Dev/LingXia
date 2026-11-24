import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { collectClassAnalysis, traceClasses } from '../compiler/class-usage.mjs';
import { parseSource, sourceChecker } from '../compiler/source.mjs';

// Traces every className in one page module: the class words it can see, and why it cannot follow the rest.
function analyse(source) {
  const file = parseSource('page.tsx', `import { cx } from '@lingxia/view';\n${source}`);
  const { usages, untraced } = collectClassAnalysis([file], sourceChecker(file));
  return {
    classes: usages.flatMap(usage => usage.value.split(/\s+/)).filter(Boolean),
    untraced: untraced.map(({ expression, reason }) => `${expression}: ${reason}`),
  };
}

const traced = [
  ['a literal', `<view className="p-1 m-2"/>`, ['p-1', 'm-2']],
  ['a const', `const a = 'p-1'; <view className={a}/>`, ['p-1']],
  ['a conditional', `const on = Math.random() > 0.5; <view className={on ? 'p-1' : 'p-2'}/>`, ['p-1', 'p-2']],
  ['&&, || and ??', `const on = Math.random() > 0.5; <view className={(on && 'p-1') || 'p-2'}/>; <view className={undefined ?? 'p-3'}/>`, ['p-1', 'p-2', 'p-3']],
  ['cx with arrays and maps', `const on = Math.random() > 0.5; <view className={cx('p-1', ['m-1'], { italic: on, 'p-2': on })}/>`, ['p-1', 'm-1', 'italic', 'p-2']],
  ['a template with whole words', 'const a = "p-1"; <view className={`m-1 ${a} p-2`}/>', ['m-1', 'p-2', 'p-1']],
  ['a const map member', `const R = { a: 'p-1', b: { c: 'p-2' } } as const; <view className={cx(R.a, R.b.c)}/>`, ['p-1', 'p-2']],
  ['a const map selected by a literal key', `const R = { a: 'p-1', b: 'p-2' }; <view className={R['b']}/>`, ['p-2']],
  ['a const map selected by any key', `const R = { a: 'p-1', b: 'p-2' }; const k = Math.random() > 0.5 ? 'a' : 'b'; <view className={R[k]}/>`, ['p-1', 'p-2']],
  ['a shorthand member', `const a = 'p-1'; const R = { a }; <view className={R.a}/>`, ['p-1']],
  ['a spread of a const object', `const extra = { className: 'p-1' }; <view {...extra}/>`, ['p-1']],
  ['a member of a written table under a wider type', `const R: Record<string, { tone: string }> = { a: { tone: 'p-1' } }; <view className={R.a.tone}/>`, ['p-1']],
  ['a member of each object a conditional picks', `const R = { a: { on: 'p-1' }, b: { on: 'p-2' } }; const t = Math.random() > 0.5 ? R.a : R.b; <view className={t.on}/>`, ['p-1', 'p-2']],
];
for (const [name, source, classes] of traced)
  test(`class tracing follows ${name}`, () => {
    const result = analyse(source);
    assert.deepEqual(result.untraced, []);
    assert.deepEqual(result.classes.sort(), [...classes].sort());
  });

test('undefined, null, false and an empty string are no class, not untraced', () => {
  const result = analyse(`const on = Math.random() > 0.5; <view className={cx(undefined, null, false, '', on ? undefined : 'p-1')}/>; <view className={on ? null : ''}/>`);
  assert.deepEqual(result.untraced, []);
  assert.deepEqual(result.classes, ['p-1']);
});

const untraced = [
  ['a parameter', `function Part(className: string) { return <view className={className}/>; }`, /^className: `className` is a parameter; its value comes from each caller$/],
  ['a prop destructured in the parameter list', `function Part({ className }: { className: string }) { return <view className={className}/>; }`, /is destructured from a parameter/],
  ['a prop destructured in the body', `function Part(props: { className: string }) { const { className } = props; return <view className={className}/>; }`, /`className` is destructured, which the build does not follow/],
  ['a rest binding', `function Part(props: { id: string; className: string }) { const { id, ...rest } = props; return <view className={rest.className}/>; }`, /`rest.className` is known only by its type/],
  ['a let binding', `let a = 'p-1'; <view className={a}/>`, /`a` is a `let` binding, which can be reassigned/],
  ['a call', `function tone(on: boolean) { return on ? 'p-1' : 'p-2'; } <view className={tone(true)}/>`, /`tone\(true\)` is the result of a call/],
  ['concatenation', `const n = 4; <view className={'gap-' + n}/>`, /`'gap-' \+ n` is computed with `\+` at run time/],
  ['a template hole inside a word', 'const n = 4; <view className={`gap-${n}`}/>', /`n` is joined into a class name at run time/],
  ['two adjacent template holes', `const a = 'gap'; const b = '-4'; <view className={\`\${a}\${b} p-1\`}/>`, /`a` is joined into a class name at run time/],
  ['a member known by its type only', `declare const R: { a: { tone: string } }; <view className={R.a.tone}/>`, /`R.a.tone` is known only by its type/],
  ['a member of an object built at run time', `const R = Object.assign({}, { a: 'p-1' }); <view className={R.a}/>`, /`R.a` /],
  ['an array element', `const A = ['p-1', 'p-2']; <view className={A[0]}/>`, /`A\[0\]` is an array element/],
  ['an object where class text belongs', `const R = { pad: { a: 'p-1' }, b: 'p-2' }; const k = Math.random() > 0.5 ? 'pad' : 'b'; <view className={cx(R[k])}/>`, /is an object, not class text/],
  ['a spread whose className is only typed', `function Part(props: { className?: string }) { return <view {...props}/>; }`, /the spread `props` declares a className known only by its type/],
  ['an unresolved name', `<view className={missing}/>`, /`missing` cannot be resolved/],
  ['a spread inside a cx map', `const more = { italic: true }; <view className={cx({ ...more })}/>`, /in a cx map is a spread/],
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

test('class tracing reports a failure of its own instead of throwing', () => {
  const file = parseSource('page.tsx', `const a = 'p-1';\n<view className={a}/>`);
  const failing = new Proxy(sourceChecker(file), { get: () => () => { throw Error('boom'); } });
  const expression = file.statements[1].expression.attributes.properties[0].initializer.expression;
  const result = traceClasses(failing, expression);
  assert.deepEqual(result.pieces, []);
  assert.match(result.untraced[0].reason, /^the class tracer failed on `a`: boom$/);
  assert.ok(ts.isIdentifier(result.untraced[0].node));
});
