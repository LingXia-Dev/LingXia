import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

// What the region reads out: its message, without the hidden icon line.
const announced = node => node.children.filter(child => child.props.accessibilityHidden !== 1).map(textOf).join('');

const mark = node => node.children.length > 0 && JSON.stringify(node).includes('M20 6 9 17l-5-5');

test('Basic: named by the label, described by the help text; toggling draws and removes the mark', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Basic');
  const box = () => screen.getByTestId('digest');
  assert.equal(box().props.checked, 1);
  assert.equal(box().props.accessibilityLabel, 'Weekly digest');
  assert.deepEqual(described(screen, box()), ['A summary of your notes every Monday.']);
  assert.ok(mark(box()));
  screen.toggle(box(), false);
  assert.equal(box().props.checked, 0);
  assert.ok(!mark(box()));
});

test('Controlled: required and invalid until checked; the error is the description', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Controlled');
  const region = () => screen.queryAll(node => node.props.role === 'status')[0];
  const terms = () => screen.getByTestId('terms');
  assert.equal(announced(region()), 'Accept the terms to create the account.');
  assert.equal(terms().props.stateRequired, 1);
  assert.equal(terms().props.stateInvalid, 1);
  assert.deepEqual(described(screen, terms()), ['Accept the terms to create the account.']);
  screen.toggle(terms(), true);
  assert.equal(terms().props.checked, 1);
  assert.equal(terms().props.stateInvalid, undefined);
  assert.equal(terms().props.ariaDescribedBy, undefined);
  assert.equal(announced(region()), '');
});

test('Sizes: three independent boxes', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Sizes');
  assert.deepEqual(screen.queryAll(node => node.tag === 'checkbox').map(node => node.props.checked), [0, 1, 0]);
});

test('the status message starts under the label, past the box and the row gap', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Controlled');
  const region = screen.queryAll(node => node.props.role === 'status')[0];
  const row = screen.getByTestId('terms').children[0];
  const box = row.children[0].children[1];
  // The box's width in the box's type, plus the row's gap, as an `em` padding.
  const [, times] = box.props.width;
  const [, startTimes, plus] = region.props.paddingInlineStart ?? region.props.paddingLeft;
  assert.deepEqual([startTimes, plus, region.props.fontSize], [times, row.props.gap, box.props.fontSize]);
  const control = screen.getByTestId('terms').props;
  const margin = control.marginLeft ?? control.marginInlineStart;
  const padding = control.paddingLeft ?? control.paddingInlineStart;
  assert.ok(padding > 0, 'the focus fill reaches past the box');
  assert.equal(margin + padding, 0, 'no net inset before the box');
});

test('StateMixed: a dash on the filled box, read as mixed', async () => {
  const box = (await renderExample('CheckboxInput/CheckboxInput', 'StateMixed')).getByTestId('mixed');
  assert.deepEqual([box.props.mixed, JSON.parse(box.props.dataAttributes).state], [1, 'mixed']);
});

test('At large text: the box keeps its size and shape, never squeezed by its label', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Sizes');
  const lines = screen.queryAll(node => node.props.accessibilityHidden === 1 && node.children?.[1]?.props.borderTopWidth === 2);
  assert.equal(lines.length, 3);
  for (const line of lines) {
    const { shrink, width, height } = line.children[1].props;
    assert.equal(shrink, 0);
    // A square in `em` of the label's type: it grows with the text, never squeezed.
    assert.ok(Array.isArray(width) && String(width) === String(height));
  }
});

test('At large text: the box is centred on the label\'s first line, in a line box of the label\'s type', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'Sizes');
  const lines = screen.queryAll(node => node.props.accessibilityHidden === 1 && node.children?.[1]?.props.borderTopWidth === 2);
  for (const line of lines) {
    const [ghost, box] = line.children;
    assert.deepEqual([line.props.flexDirection, line.props.align, line.props.shrink], ['row', 'center', 0]);
    assert.equal(ghost.tag, 'text');
    assert.equal(ghost.text, '\u200B', 'a zero-width run: nothing drawn, no layer');
    assert.equal(ghost.props.opacity, undefined);
    const label = screen.queryAll(node => node.tag === 'text' && node !== ghost && node.props.fontSize === ghost.props.fontSize && node.props.lineHeight === ghost.props.lineHeight);
    assert.ok(label.length > 0, 'the ghost has the label\'s font size and line height');
    assert.equal(box.props.marginTop, undefined, 'no fixed nudge: the line centres it');
  }
});

test('StateDisabled: checked reads as off-but-checked, a neutral fill and a muted mark, not a faded accent', async () => {
  const screen = await renderExample('CheckboxInput/CheckboxInput', 'StateDisabled');
  const box = screen.queryAll(node => node.props.borderTopWidth === 2)[0];
  assert.deepEqual([box.props.background, box.props.color], ['text/20', 'text-muted']);
  assert.equal(screen.queryAll(node => node.tag === 'checkbox')[0].props.opacity, undefined, 'the row is not faded');
});

const savedPage = component => `
  import { useLocal } from '@lingxia/view';
  import { ${component} } from '@liulidesign/core/${component}';
  let finish: (ok: boolean) => void = () => {};
  let calls = 0;
  const action = () => { calls++; return new Promise<void>((resolve, reject) => { finish = ok => (ok ? resolve() : reject(Error('offline'))); }); };
  export function render() {
    const on = useLocal('on', false);
    return <page>
      <${component} label="Share usage data" description="Saved to your account." bind={on} changeAction={action} data-testid="saved" />
      <button data-testid="ok" onClick={() => finish(true)}>Ok</button>
      <button data-testid="fail" onClick={() => finish(false)}>Fail</button>
      <text data-testid="calls">{String(calls)}</text>
    </page>;
  }
`;

test('changeAction: flips at once, busy but not disabled, holds further presses, and keeps the value on success', async () => {
  const screen = await renderPage(savedPage('CheckboxInput'));
  screen.toggle(screen.getByTestId('saved'), true);
  const busy = screen.getByTestId('saved');
  assert.deepEqual([busy.props.checked, busy.props.stateBusy, busy.props.enabled], [1, 1, undefined]);
  assert.throws(() => screen.toggle(screen.getByTestId('saved'), false), /did not update value/, 'a press while saving is held off');
  assert.equal(screen.getByTestId('saved').props.checked, 1);
  screen.press(screen.getByTestId('ok'));
  await new Promise(resolve => setImmediate(resolve));
  screen.rerender({});
  const done = screen.getByTestId('saved');
  assert.deepEqual([done.props.checked, done.props.stateBusy], [1, undefined]);
  assert.equal(screen.getByText('1').tag, 'text', 'the action ran once');
});

test('changeAction: a failure flips back and is said in the status line, as a description, until the next change', async () => {
  const screen = await renderPage(savedPage('CheckboxInput'));
  const errors = [];
  const original = console.error;
  console.error = (...args) => errors.push(args.join(' '));
  try {
    screen.toggle(screen.getByTestId('saved'), true);
    screen.press(screen.getByTestId('fail'));
    await new Promise(resolve => setImmediate(resolve));
    screen.rerender({});
  } finally {
    console.error = original;
  }
  const control = screen.getByTestId('saved');
  assert.deepEqual([control.props.checked, control.props.stateBusy], [0, undefined]);
  assert.deepEqual(described(screen, control), ['Saved to your account.', "That didn't work. Try again."]);
  screen.toggle(screen.getByTestId('saved'), true);
  assert.deepEqual(described(screen, screen.getByTestId('saved')), ['Saved to your account.']);
});
