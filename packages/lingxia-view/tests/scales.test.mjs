import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset } from '../src/internal.js';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { scales } from '../src/generated/contract.js';
import { decode } from './support/wire.mjs';

const slot = name => scales.findIndex(entry => entry.name === name);
const step = (name, times = 1, plus = 0) => [slot(name), times, plus];
const compiled = async names => Object.fromEntries((await compileUtilities(new Set(names))).map(rule => [rule.name, rule]));
const render = (view) => { const out = decode(prepare(view, { env: {} })); commit(); return out; };
const props = (className) => render(() => h('page', {}, h('view', { className, 'data-testid': 'v' }, h('text', {}, 'a')))).find(node => node.testId === 'v').props;

test('theme steps compile to [slot, times, plus] references; literals stay numbers', async () => {
  const rules = await compiled(['p-4', '-mt-2', 'p-0.5', 'gap-[calc(var(--spacing)*2+1px)]', 'text-sm', 'rounded-lg', 'rounded', 'size-[1.25em]', 'p-[13px]', 'w-1/2', 'rounded-full']);
  assert.deepEqual(rules['p-4'].style.paddingTop, step('space', 4));
  assert.deepEqual(rules['-mt-2'].style.marginTop, step('space', -2));
  assert.deepEqual(rules['p-0.5'].style.paddingLeft, step('space', 0.5));
  assert.deepEqual(rules['gap-[calc(var(--spacing)*2+1px)]'].style.gap, step('space', 2, 1));
  assert.deepEqual([rules['text-sm'].style.fontSize, rules['text-sm'].style.lineHeight], [step('text.sm'), step('leading.sm')]);
  assert.deepEqual([rules['rounded-lg'].style.borderRadius, rules.rounded.style.borderRadius], [step('radius.lg'), step('radius.DEFAULT')]);
  assert.deepEqual(rules['size-[1.25em]'].style.width, step('em', 1.25), 'em follows the text, on the host');
  assert.equal(rules['p-[13px]'].style.paddingTop, 13, 'a literal does not follow the pack');
  assert.equal(rules['w-1/2'].style.width, '50%');
  assert.equal(rules['rounded-full'].style.borderRadius, 16384);
});

test('UI_SCALE: arithmetic a step cannot follow at run time fails the build, naming it', async () => {
  const rules = await compiled(['m-[calc(var(--spacing)*2+var(--radius-lg))]', 'p-[max(var(--spacing),4px)]', 'w-[calc(var(--spacing)*var(--spacing))]']);
  assert.match(rules['m-[calc(var(--spacing)*2+var(--radius-lg))]'].error.message, /UI_SCALE: .*two scale steps \(space, radius\.lg\)/);
  assert.match(rules['p-[max(var(--spacing),4px)]'].error.message, /UI_SCALE: .*min\(\) or max\(\)/);
  assert.match(rules['w-[calc(var(--spacing)*var(--spacing))]'].error.message, /UI_SCALE: .*a product of two steps/);
});

test('a step reaches the wire as a reference and goes only where its family applies', async () => {
  reset();
  configureClasses(await compileUtilities(new Set(['p-4', 'text-sm', 'leading-tight', 'tracking-wide', 'size-[1em]', 'p-[var(--radius-lg)]'])));
  const view = props('p-4 text-sm leading-tight tracking-wide size-[1em]');
  assert.deepEqual(JSON.parse(JSON.stringify(view.paddingTop)), step('space', 4));
  assert.deepEqual(view.fontSize, step('text.sm'));
  assert.deepEqual(view.lineHeight, step('text.sm', 1.25), 'a multiplier scales the step');
  assert.deepEqual(view.letterSpacing, step('text.sm', 0.025), 'so does an em letter spacing');
  assert.deepEqual(view.width, step('em'));
  assert.throws(() => props('p-[var(--radius-lg)]'), /UI_SCALE: paddingTop does not take the theme step radius\.lg; it takes space/);
  configureClasses([]);
});

test('motion steps resolve when the transition starts', async () => {
  reset();
  configureClasses(await compileUtilities(new Set(['transition-opacity', 'duration-fast', 'ease-enter'])));
  const { transition } = props('transition-opacity duration-fast ease-enter');
  const { durationMs, easing } = JSON.parse(transition);
  assert.deepEqual([durationMs, easing], [step('duration.fast'), step('ease.enter')]);
  configureClasses([]);
});

test('a negative multiple of a step is refused where only lengths of 0 or more fit', async () => {
  reset();
  configureClasses([{ name: 'neg', when: {}, style: { paddingTop: step('space', -2) } }]);
  assert.throws(() => props('neg'), /UI_STYLE_VALUE: paddingTop takes a step that resolves to 0 or more/);
  configureClasses([]);
});

test('outside the fields a step family lists, a step takes the built-in pack value at build time', async () => {
  const rules = await compiled(['translate-x-4', 'border-[length:var(--spacing)]']);
  assert.equal(rules['translate-x-4'].transform.translateX, 16);
  assert.equal(rules['border-[length:var(--spacing)]'].style.borderTopWidth, 4);
});

test('@lingxia/view/testing points() resolves steps under the built-in pack, at any depth', async () => {
  const { points } = await import('@lingxia/view/testing');
  assert.deepEqual(points({ paddingTop: step('space', 4), gap: [step('space', 2), 2], fontSize: step('text.sm'), easing: step('ease.exit'), width: step('em') }),
    { paddingTop: 16, gap: [8, 2], fontSize: 13, easing: [0.4, 0, 1, 1], width: step('em') });
  // Three numbers on a field that takes no step are its own values, not a step.
  assert.deepEqual(points({ plotValues: [1, 2, 3], pressedStyle: { paddingTop: step('space', 2) } }), { plotValues: [1, 2, 3], pressedStyle: { paddingTop: 8 } });
});

test('@lingxia/view/testing render(..., { points: true }) reads every node in points, after a press too', async () => {
  const { render } = await import('@lingxia/view/testing');
  const { mkdtemp, writeFile, rm } = await import('node:fs/promises');
  const { tmpdir } = await import('node:os');
  const { join } = await import('node:path');
  const root = await mkdtemp(join(tmpdir(), 'lingxia-view-points-'));
  try {
    await writeFile(join(root, 'lxapp.json'), '{}');
    await writeFile(join(root, 'page.tsx'), `import { useLocal } from '@lingxia/view';
export function render() {
  const wide = useLocal('wide', false);
  return <view data-testid="box" className={wide.value ? 'p-8 rounded-lg' : 'p-4 rounded-lg'}><button data-testid="grow" onClick={() => wide.set(true)}>Grow</button></view>;
}`);
    const plain = await render(join(root, 'page.tsx'));
    assert.deepEqual([...plain.getByTestId('box').props.paddingTop], step('space', 4));
    const screen = await render(join(root, 'page.tsx'), {}, { points: true });
    assert.deepEqual([screen.getByTestId('box').props.paddingTop, screen.getByTestId('box').props.radius], [16, 10]);
    screen.press(screen.getByTestId('grow'));
    assert.equal(screen.getByTestId('box').props.paddingTop, 32);
  } finally { await rm(root, { recursive: true, force: true }); }
});
