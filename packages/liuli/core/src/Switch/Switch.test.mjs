import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

test('Basic: named by its label, described by its help text, and a toggle reaches the bound cell', async () => {
  const screen = await renderExample('Switch/Switch', 'Basic');
  const control = screen.getByTestId('sync');
  assert.equal(control.tag, 'switch');
  assert.equal(control.props.accessibilityLabel, 'Sync over mobile data');
  assert.deepEqual(described(screen, control), ['Photos wait for Wi-Fi either way.']);
  assert.ok(JSON.stringify(control.children).includes('Photos wait for Wi-Fi either way.'), 'the help text is inside the press target');
  assert.equal(control.props.checked, 1);
  screen.toggle(control, false);
  assert.equal(screen.getByTestId('sync').props.checked, 0);
});

test('InList: each row is its own switch', async () => {
  const screen = await renderExample('Switch/Switch', 'InList');
  screen.toggle(screen.getByTestId('digest'), true);
  assert.equal(screen.getByTestId('digest').props.checked, 1);
  assert.equal(screen.getByTestId('mentions').props.checked, 1);
});

test('the track turns to the deepened accent when on, and the thumb rings on keyboard focus', async () => {
  const screen = await renderExample('Switch/Switch', 'Basic');
  const control = screen.getByTestId('sync');
  // The on track is a checked state style, so the View paints it from the switch's own state.
  assert.match(control.props.checkedStyle.trackStyle, /"accent-solid"/);
  assert.match(control.props.trackStyle, /"line-strong\/45"/);
  screen.toggle(control, false);
  assert.equal(screen.getByTestId('sync').props.checked, 0);
  assert.match(control.props.focusVisibleStyle.thumbStyle, /focus/);
});

test('a row rings keyboard focus inside its edges, not round the thumb', async () => {
  const screen = await renderExample('Switch/Switch', 'InList');
  const focus = screen.getByTestId('mentions').props.focusVisibleStyle;
  assert.deepEqual(JSON.parse(focus.ring), { inner: { width: 2, color: 'focus' } });
  assert.equal(focus.thumbStyle, undefined);
});

test('Applying: busy, takes no press, and shows a spinner beside the words', async () => {
  const screen = await renderExample('Switch/Switch', 'Applying');
  const control = screen.getByTestId('applying');
  assert.deepEqual([control.props.stateBusy, control.props.enabled, control.props.checked], [1, 0, 1]);
  assert.equal(screen.queryAll(node => node.tag === 'vector').length, 1);
});

test('LabelHidden: named by its label, which is not drawn', async () => {
  const screen = await renderExample('Switch/Switch', 'LabelHidden');
  const control = screen.getByTestId('pin');
  assert.equal(control.props.accessibilityLabel, 'Pin chat');
  assert.equal(JSON.stringify(control.children ?? []).includes('Pin chat'), false);
  screen.toggle(control, true);
  assert.equal(screen.getByTestId('pin').props.checked, 1);
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
  const screen = await renderPage(savedPage('Switch'));
  screen.toggle(screen.getByTestId('saved'), true);
  const busy = screen.getByTestId('saved');
  assert.deepEqual([busy.props.checked, busy.props.stateBusy, busy.props.enabled], [1, 1, undefined]);
  assert.equal(screen.queryAll(node => node.tag === 'vector').length, 0, 'no spinner: the row does not reflow');
  assert.throws(() => screen.toggle(screen.getByTestId('saved'), false), /did not update value/, 'a press while saving is held off');
  assert.equal(screen.getByTestId('saved').props.checked, 1);
  screen.press(screen.getByTestId('ok'));
  await new Promise(resolve => setImmediate(resolve));
  screen.rerender({});
  const done = screen.getByTestId('saved');
  assert.deepEqual([done.props.checked, done.props.stateBusy], [1, undefined]);
  assert.equal(screen.getByText('1').tag, 'text', 'the action ran once');
});

test('changeAction: a failure flips back and is said under the label, as a description, until the next change', async () => {
  const screen = await renderPage(savedPage('Switch'));
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
