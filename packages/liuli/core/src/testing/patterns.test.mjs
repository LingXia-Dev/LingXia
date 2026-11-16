import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

const gallery = resolve(dirname(fileURLToPath(import.meta.url)), '../../../gallery');
const open = (page, input = {}) => render(resolve(gallery, `pages/${page}/index.tsx`), input, { root: gallery });
const settle = () => new Promise(resolve => setImmediate(resolve));
const has = (screen, id) => screen.queryAll(node => node.testId === id).length > 0;

test('sign-in: errors appear only after an attempt, and a valid form signs in', async () => {
  let calls = 0;
  const screen = await open('sign-in', { actions: { signIn: async () => { calls += 1; } } });
  assert.equal(screen.queryAll(node => node.text === 'Enter your email.').length, 0);
  screen.press(screen.getByTestId('submit'));
  await settle();
  screen.getByText('Enter your email.');
  screen.getByText('Enter your password.');
  assert.equal(calls, 0);
  screen.type(screen.getByTestId('email'), 'ana@example.com');
  screen.type(screen.getByTestId('password'), 'correct horse');
  assert.equal(screen.queryAll(node => node.text === 'Enter your email.').length, 0);
  screen.press(screen.getByTestId('submit'));
  await settle();
  screen.rerender({});
  assert.equal(calls, 1);
  assert.ok(has(screen, 'signed-in'));
});

test('settings: deleting the account asks first, and can be cancelled after', async () => {
  const screen = await open('settings');
  screen.press(screen.getByTestId('delete-account'));
  screen.press(screen.getByTestId('confirm-delete'));
  assert.ok(has(screen, 'deletion-scheduled'));
  screen.press(screen.getByTestId('cancel-deletion'));
  assert.ok(has(screen, 'delete-account'));
});

test('settings: each group is a list named by its heading, every row a list item', async () => {
  const screen = await open('settings');
  const lists = screen.queryAll(node => node.props.role === 'list');
  assert.deepEqual(lists.map(list => list.props.accessibilityLabel), ['Account', 'General', 'Signed-in devices', 'Danger zone']);
  const rows = list => screen.queryAll(node => node.props.role === 'listitem' && list.children.includes(node)).length;
  assert.deepEqual(lists.map(rows), [1, 5, 3, 1]);
});

test('uploads: each section is a list named by its heading, every card a list item', async () => {
  const screen = await open('uploads');
  const lists = screen.queryAll(node => node.props.role === 'list');
  assert.deepEqual(lists.map(list => list.props.accessibilityLabel), ['In progress', 'Done today']);
  const rows = list => screen.queryAll(node => node.props.role === 'listitem' && list.children.includes(node)).length;
  assert.deepEqual(lists.map(rows), [3, 1]);
});

test('uploads: a failed upload retries, and the photos finish', async () => {
  const screen = await open('uploads');
  screen.press(screen.getByTestId('retry'));
  assert.ok(has(screen, 'budget'));
  for (let press = 0; press < 2; press += 1) screen.press(screen.getByTestId('send-more'));
  assert.ok(has(screen, 'photos'));
  screen.press(screen.getByTestId('send-more'));
  assert.ok(!has(screen, 'photos'));
});

// A page whose content changes with a control is driven through every value: a size the View refuses
// (a layout number taken from the data) fails the render, and the press throws.
test('dashboard: every period draws its figures and both charts', async () => {
  const screen = await open('dashboard');
  for (const period of ['month', 'year', 'week', 'year']) {
    screen.press(screen.getByTestId(`period-${period}`));
    await settle();
    assert.ok(has(screen, 'revenue-chart'), `the revenue chart is drawn for the ${period}`);
    assert.ok(screen.queryAll(node => node.props.accessibilityLabel === 'Orders by channel').length >= 1, `the channels chart is drawn for the ${period}`);
    const sizes = screen.queryAll(node => node.tag !== 'vector' && node.props.grow !== undefined).map(node => node.props.grow);
    assert.ok(sizes.every(size => Number.isFinite(size) && size >= 0 && size <= 1000), `${period}: no layout number is a data value: ${sizes.filter(size => !(size >= 0 && size <= 1000))}`);
  }
});

test('list and detail: every request opens its record', async () => {
  for (const id of ['r-218', 'r-217', 'r-214', 'r-209', 'r-204']) {
    const screen = await open('list-detail');
    screen.press(screen.getByTestId(`request-${id}`));
    await settle();
    assert.ok(has(screen, 'details'), `${id} shows its details`);
  }
});
