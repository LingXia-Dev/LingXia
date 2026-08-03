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
  const platform = (test.args as Record<string, string>).platform?.toLocaleLowerCase();
  const desktop = platform === 'windows' ? lx.automation().desktop : undefined;
  const host = desktop
    ? (await desktop.windows()).find((window) => (
      window.visible
      && window.process.toLocaleLowerCase().includes('lingxiademo')
      && window.title === 'LingXia'
    ))
    : undefined;
  const visibleWindowsBefore = host
    ? new Set((await desktop!.windows())
      .filter((window) => window.visible && window.pid === host.pid)
      .map((window) => window.id))
    : undefined;

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

  if (desktop && host && visibleWindowsBefore) {
    const deadline = Date.now() + 3_000;
    let leaked = [] as Awaited<ReturnType<typeof desktop.windows>>;
    do {
      leaked = (await desktop.windows()).filter((window) => (
        window.visible
        && window.pid === host.pid
        && window.title === ''
        && !visibleWindowsBefore.has(window.id)
      ));
      if (leaked.length === 0) return;
      await new Promise((resolve) => setTimeout(resolve, 50));
    } while (Date.now() < deadline);
    throw new Error(`navigation left visible native overlays: ${JSON.stringify(leaked)}`);
  }
});
