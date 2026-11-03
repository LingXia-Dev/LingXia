import { expect, spec, type Fixture } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture } from '../helpers/poll.js';
import { waitForCurrentPage } from '../helpers/page.js';

async function openNavigator(t: Fixture, id: string) {
  const { app, defer } = bindFixture(t, id);
  defer(async () => { await app.nav.relaunch({ page: 'home' }); });
  await app.nav.relaunch({ page: 'home' });
  const home = await app.nav.current();
  await app.nav.to({ page: 'navigator' });
  const page = await app.page({ name: 'navigator' });
  return { app, page, home };
}

spec('navigate through a component and return to the original page instances', {
  id: 'NAVIGATOR-STACK-001', app: SHOWCASE_APP_ID,
  covers: ['lx.navigateTo', 'lx.navigateBack'],
}, async (t) => {
  const { app, page, home } = await openNavigator(t, 'NAVIGATOR-STACK-001');
  const navigator = await app.nav.current();
  await page.view.css('lx-navigator[open-type="navigate"][page="device"]').click();
  await waitForCurrentPage(app, 'device');
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['home', 'navigator', 'device']);
  await expect((await app.page({ name: 'device' })).view.testId('device-page')).toHaveAttribute('data-mode', 'device');
  await app.nav.back();
  expect((await app.nav.current()).instanceId).toBe(navigator.instanceId);
  await page.view.css('lx-navigator[open-type="navigateBack"]').click();
  expect((await waitForCurrentPage(app, 'home')).instanceId).toBe(home.instanceId);
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['home']);
});

spec('redirect through a component replaces only the current stack entry', {
  id: 'NAVIGATOR-REDIRECT-001', app: SHOWCASE_APP_ID,
  covers: ['lx.redirectTo'],
}, async (t) => {
  const { app, page, home } = await openNavigator(t, 'NAVIGATOR-REDIRECT-001');
  await page.view.css('lx-navigator[open-type="redirect"]').click();
  await waitForCurrentPage(app, 'ui');
  await expect((await app.page({ name: 'ui' })).view.testId('toast-show')).toBeVisible();
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['home', 'ui']);
  await app.nav.back();
  expect((await app.nav.current()).instanceId).toBe(home.instanceId);
});

spec('relaunch through a component resets the stack and delivers its query', {
  id: 'NAVIGATOR-RELAUNCH-001', app: SHOWCASE_APP_ID,
  covers: ['lx.reLaunch'],
}, async (t) => {
  const { app, page } = await openNavigator(t, 'NAVIGATOR-RELAUNCH-001');
  await page.view.css('lx-navigator[open-type="reLaunch"]').click();
  await waitForCurrentPage(app, 'device');
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['device']);
  await expect((await app.page({ name: 'device' })).view.testId('device-page')).toHaveAttribute('data-mode', 'screen');
});

spec('switch tab through a component clears the pushed stack', {
  id: 'NAVIGATOR-TAB-001', app: SHOWCASE_APP_ID,
  covers: ['lx.switchTab'],
}, async (t) => {
  const { app, page } = await openNavigator(t, 'NAVIGATOR-TAB-001');
  await page.view.css('lx-navigator[open-type="switchTab"][page="todo"]').click();
  await waitForCurrentPage(app, 'todo');
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['todo']);
  await expect((await app.page({ name: 'todo' })).view.testId('todo-page')).toBeVisible();
});
