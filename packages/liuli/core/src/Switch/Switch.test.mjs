import { test } from 'node:test';
import assert from 'node:assert/strict';
import { described, renderExample } from '../testing/harness.mjs';

test('Basic: named by its label, described by its help text, and a toggle reaches the bound cell', async () => {
  const screen = await renderExample('Switch/Switch', 'Basic');
  const control = screen.getByTestId('sync');
  assert.equal(control.tag, 'switch');
  assert.equal(control.props.accessibilityLabel, 'Sync over mobile data');
  assert.deepEqual(described(screen, control), ['Photos wait for Wi-Fi either way.']);
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

test('the track turns to the accent when on, and the thumb rings on keyboard focus', async () => {
  const screen = await renderExample('Switch/Switch', 'Basic');
  const control = screen.getByTestId('sync');
  assert.match(control.props.trackStyle, /"accent"/);
  screen.toggle(control, false);
  assert.match(screen.getByTestId('sync').props.trackStyle, /"line-strong\/45"/);
  assert.match(control.props.focusVisibleStyle.thumbStyle, /focus/);
});

test('a row shows keyboard focus as a fill, not a thumb ring', async () => {
  const screen = await renderExample('Switch/Switch', 'InList');
  const focus = screen.getByTestId('mentions').props.focusVisibleStyle;
  assert.ok(focus.background !== undefined || focus.backgroundColor !== undefined, JSON.stringify(focus));
  assert.equal(focus.thumbStyle, undefined);
});
