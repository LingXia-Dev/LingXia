import { expect, test } from '@rongjs/test';
import type { LxAppDriver, PageInfo } from 'lingxia-types';
import { waitForElementAttribute } from '../helpers/page.js';

async function waitForCurrent(app: LxAppDriver, name: string): Promise<PageInfo> {
  const deadline = Date.now() + 10_000;
  let current = await app.nav.current();
  while (Date.now() < deadline) {
    if (current.name === name && current.ready) return current;
    await new Promise((resolve) => setTimeout(resolve, 50));
    current = await app.nav.current();
  }
  throw new Error(`Timed out waiting for current page '${name}': ${JSON.stringify(current)}`);
}

test('preserves navigation stack, query, redirect, back, and tab semantics', async () => {
  const app = lx.automation().lxapp();

  await app.nav.relaunch({ page: 'home' });
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);

  await app.nav.to({ page: 'device', query: { type: 'screen' } });
  await waitForCurrent(app, 'device');
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-page"]' });
  await waitForElementAttribute(
    app,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'screen',
  );
  const mode = await app.page.eval({
    page: 'device',
    script: 'document.querySelector(\'[data-testid="device-page"]\')?.getAttribute(\'data-mode\')',
  });
  expect(mode).toBe('screen');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'device']);

  const backed = await app.nav.back();
  expect(backed.name).toBe('home');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home']);

  await app.nav.to({ page: 'components' });
  await app.nav.redirect({ page: 'picker' });
  await waitForCurrent(app, 'picker');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'picker']);

  await app.nav.switchTab({ page: 'todo' });
  await waitForCurrent(app, 'todo');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['todo']);

  // Page instances are cached by path. A query-free navigation must not
  // inherit the query from the earlier device visit.
  await app.nav.relaunch({ page: 'device' });
  await waitForCurrent(app, 'device');
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-page"]' });
  await waitForElementAttribute(
    app,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'device',
  );
  const defaultMode = await app.page.eval({
    page: 'device',
    script: 'document.querySelector(\'[data-testid="device-page"]\')?.getAttribute(\'data-mode\')',
  });
  expect(defaultMode).toBe('device');
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['device']);
});
