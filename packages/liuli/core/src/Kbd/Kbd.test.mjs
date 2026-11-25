import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';
import { textOf } from '@lingxia/view/testing';

test('Shortcuts: one cap per key, read once by the spoken form', async () => {
  const kbd = (await renderExample('Kbd/Kbd', 'Shortcuts')).getByTestId('archive');
  assert.deepEqual(kbd.children.map(textOf), ['⇧', '⌘', 'A']);
  assert.equal(kbd.props.accessibilityLabel, 'Shift Command A');
  assert.ok(kbd.children.every(cap => cap.props.accessibilityHidden === 1), 'the caps are not read again');
});

test('Hint: without a spoken form the key is read as written, in a cap at least as wide as tall', async () => {
  const kbd = (await renderExample('Kbd/Kbd', 'Hint')).getByTestId('enter');
  const [cap] = kbd.children;
  assert.deepEqual([kbd.props.accessibilityLabel, cap.props.accessibilityHidden, cap.props.height, cap.props.minWidth], [undefined, undefined, 24, 24]);
});

test('a combination keeps its caps left to right inside a right-to-left page', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(`
    import { Kbd } from '@liulidesign/core/Kbd';
    export function render() { return <page><view dir="rtl"><Kbd keys={['⌘', 'Z']} data-testid="combo" /></view></page>; }
  `);
  const combo = screen.getByTestId('combo');
  assert.equal(combo.props.direction, 'ltr');
  assert.equal(textOf(combo).replace(/\s+/g, ''), '⌘Z');
});
