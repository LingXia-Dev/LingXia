import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

const wide = { env: { sizeClass: 'regular', viewport: { width: 1024, height: 768 } } };
const fold = (direction) => ({
  env: {
    sizeClass: 'regular',
    direction,
    viewport: { width: 728, height: 868 },
    hinge: { x: 336, y: 0, width: 12, height: 868, orientation: 'vertical', posture: 'flat' },
  },
});
const phone = { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } };
const panes = (screen, id) => [screen.queryAll(n => n.testId === `${id}-list`)[0], screen.queryAll(n => n.testId === `${id}-detail`)[0]];

test('Orders, wide: the list and the placeholder side by side, each a named group; a row opens its record', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'Orders', wide);
  const [list, detail] = panes(screen, 'orders');
  assert.deepEqual([list.props.width, list.props.role, list.props.accessibilityLabel], [360, 'group', 'Orders']);
  assert.ok(textOf(detail).includes('Choose an order'));
  assert.equal(screen.queryAll(n => n.testId === 'orders-actions').length, 0, 'no actions while nothing is chosen');
  screen.press(screen.getByTestId('order-ord-102'));
  assert.ok(textOf(screen.getByTestId('order-detail')).includes('Warehouse pickup'));
  assert.equal(panes(screen, 'orders')[1].props.accessibilityLabel, 'Order #102');
  assert.ok(screen.getByTestId('orders-actions'), 'the record\'s actions, outside its scroll');
});

test('Each pane scrolls on its own; the detail\'s scroll starts over for each new choice', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'Orders', wide);
  screen.press(screen.getByTestId('order-ord-101'));
  const scrolls = () => screen.queryAll(n => n.tag === 'scroll');
  assert.equal(scrolls().length, 2);
  const first = scrolls()[1];
  screen.press(screen.getByTestId('order-ord-103'));
  assert.notEqual(scrolls()[1], first, 'a new scroll for a new record');
});

test('At a fold the list takes the start leaf, in either direction, and the detail starts past the crease', async () => {
  for (const [direction, width] of [['ltr', 336], ['rtl', 728 - 336 - 12]]) {
    const screen = await renderExample('ListDetail/ListDetail', 'Orders', fold(direction));
    const [list, detail] = panes(screen, 'orders');
    assert.equal(list.props.width, width, direction);
    assert.equal(detail.props.marginInlineStart, 12, direction);
  }
});

test('Orders, phone: a row opens its record over the list, which stays mounted; the back button returns', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'Orders', phone);
  assert.equal(panes(screen, 'orders')[1], undefined);
  screen.press(screen.getByTestId('order-ord-101'));
  assert.equal(panes(screen, 'orders')[0].props.display, 'none');
  const heading = screen.queryAll(n => n.tag === 'text' && n.props.role === 'heading')[0];
  assert.equal(textOf(heading), 'Order #101');
  const back = screen.getByTestId('orders-back');
  assert.equal(back.props.accessibilityLabel, 'Orders', 'named for where it leads');
  screen.press(back);
  assert.equal(panes(screen, 'orders')[1], undefined);
  assert.notEqual(panes(screen, 'orders')[0].props.display, 'none');
});

test('Workspace: `value` with `onChange` drives it, and the detail is capped for prose', async () => {
  const screen = await renderExample('ListDetail/ListDetail', 'Workspace', wide);
  assert.equal(textOf(screen.getByTestId('workspace-title')), 'Inbox');
  const inner = screen.getByTestId('workspace-title').parent;
  assert.equal(inner.props.maxWidth, 784);
  assert.equal(screen.getByTestId('workspace-list').props.width, 200);
});
