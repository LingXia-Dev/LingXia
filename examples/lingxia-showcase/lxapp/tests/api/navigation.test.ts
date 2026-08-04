import { expect } from '@rongjs/test';
import type { LxAppDriver, PageInfo } from 'lingxia-types/automation';
import { currentPageOrNull, waitForElementAttribute } from '../helpers/page.js';
import { contract, eventually } from '../support/contract.js';

async function waitForCurrent(app: LxAppDriver, name: string): Promise<PageInfo> {
  return eventually(
    () => app.nav.current(),
    (current) => current.name === name && current.ready,
    { describe: `current page '${name}' to become ready`, timeoutMs: 30_000 },
  );
}

contract({
  id: 'NAV-001',
  title: 'preserve stack, query, redirect, back, and tab semantics',
  covers: [
    'NavDriver.relaunch',
    'NavDriver.to',
    'NavDriver.current',
    'NavDriver.stack',
    'NavDriver.back',
    'NavDriver.redirect',
    'NavDriver.switchTab',
  ],
  layer: 'view',
  levels: ['semantic', 'boundary', 'lifecycle'],
  scope: 'portable',
  expectedOutcome: 'supported',
}, async ({ app }) => {
  const initial = await currentPageOrNull(app);
  if (initial?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrent(app, 'home');
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
