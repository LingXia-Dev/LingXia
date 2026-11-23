import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const regular = { env: { sizeClass: 'regular', viewport: { width: 1024, height: 768 } } };
const foldVertical = {
  env: {
    sizeClass: 'regular',
    viewport: { width: 728, height: 868 },
    hinge: { x: 336, y: 0, width: 12, height: 868, orientation: 'vertical', posture: 'flat' },
  },
};
const compact = { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } };

test('ListDetail regular: renders list and placeholder side by side, then shows selected detail', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'OrdersMasterDetail', regular);
  const list = screen.getByTestId('orders-list-detail-list');
  const detail = screen.getByTestId('orders-list-detail-detail');
  assert.equal(list.props.width, 360);
  assert.ok(textOf(detail).includes('Select an order'));

  screen.press(screen.getByTestId('order-ord-102'));
  assert.equal(textOf(screen.getByTestId('order-detail-body')).includes('Order #102'), true);
});

test('ListDetail foldable hinge: snaps list width and detail gap to vertical hinge', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'OrdersMasterDetail', foldVertical);
  assert.equal(screen.getByTestId('orders-list-detail-list').props.width, 336);
  assert.equal(screen.getByTestId('orders-list-detail-detail').props.marginInlineStart, 12);
});

test('ListDetail compact: drills into detail keeping list mounted hidden, and back returns to list', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'OrdersMasterDetail', compact);
  assert.equal(screen.queryAll(n => n.testId === 'orders-list-detail-detail').length, 0);
  screen.press(screen.getByTestId('order-ord-101'));

  assert.equal(screen.getByTestId('orders-list-detail-list').props.display, 'none');
  assert.ok(textOf(screen.getByTestId('order-detail-body')).includes('Order #101'));

  screen.press(screen.getByTestId('orders-list-detail-back'));
  assert.equal(screen.queryAll(n => n.testId === 'orders-list-detail-detail').length, 0);
  assert.notEqual(screen.getByTestId('orders-list-detail-list').props.display, 'none');
});
