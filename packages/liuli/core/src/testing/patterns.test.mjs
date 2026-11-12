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

test('uploads: a failed upload retries, and the photos finish', async () => {
  const screen = await open('uploads');
  screen.press(screen.getByTestId('retry'));
  assert.ok(has(screen, 'budget'));
  for (let press = 0; press < 2; press += 1) screen.press(screen.getByTestId('send-more'));
  assert.ok(has(screen, 'photos'));
  screen.press(screen.getByTestId('send-more'));
  assert.ok(!has(screen, 'photos'));
});
