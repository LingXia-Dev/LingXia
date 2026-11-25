import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset, createView } from '../src/internal.js';
import { publishHost } from '../src/host.js';
import { viewCall } from '../src/runtime/overlay.js';
import { decode } from './support/wire.mjs';

// The sheet's place follows the page's own size and pointer, not the device.
test('the action sheet is a bottom sheet under touch when either side is compact and a card otherwise', () => {
  const placed = env => {
    reset(); publishHost(() => h('view', {}));
    viewCall({ id: 1, method: 'ui.showActionSheet', params: { itemList: ['Copy'] } });
    const nodes = decode(prepare(() => h('page', {}), createView({}, env, 0))); commit();
    const sheet = nodes.find(node => node.testId === 'lx-action-sheet');
    return sheet.props.width === 320 ? 'card' : 'bottom';
  };
  const phone = { width: 390, height: 844 }, tablet = { width: 1024, height: 768 };
  assert.equal(placed({ sizeClass: 'compact', pointer: 'coarse', viewport: phone }), 'bottom');
  assert.equal(placed({ sizeClass: 'regular', pointer: 'coarse', viewport: { width: 844, height: 390 } }), 'bottom');
  assert.equal(placed({ sizeClass: 'regular', pointer: 'coarse', viewport: tablet }), 'card');
  assert.equal(placed({ sizeClass: 'regular', pointer: 'coarse', viewport: { width: 768, height: 1024 } }), 'card');
  assert.equal(placed({ sizeClass: 'compact', pointer: 'fine', viewport: phone }), 'card');
});
