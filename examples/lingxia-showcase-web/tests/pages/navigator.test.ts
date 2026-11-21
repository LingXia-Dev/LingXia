import { expect, test, type Fixture } from '@lingxia/test';
import { APP_ID } from '../support/app.js';
import { bindFixture } from '../support/poll.js';
import { waitForCurrentPage } from '../support/page.js';

async function openNavigator(t: Fixture, id: string) {
  const { app, defer } = bindFixture(t, id);
  defer(async () => { await app.nav.reset({ page: 'home' }); });
  await app.nav.reset({ page: 'home' });
  const home = await app.nav.current();
  await app.nav.go({ page: 'navigator' });
  const page = await app.page({ name: 'navigator' });
  return { app, page, home };
}

test('navigate through a component and return to the original page instances', {
  id: 'NAVIGATOR-STACK-001', app: APP_ID,
  covers: ['lx.nav.go', 'lx.nav.back'],
}, async (t) => {
  const { app, page, home } = await openNavigator(t, 'NAVIGATOR-STACK-001');
  const navigator = await app.nav.current();
  await page.view.css(`lx-navigator[to='{"page":"about","params":{"from":"navigator"}}']`).click();
  await waitForCurrentPage(app, 'about');
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['home', 'navigator', 'about']);
  await app.nav.back();
  expect((await app.nav.current()).instanceId).toBe(navigator.instanceId);
  await page.view.css(`lx-navigator[to='{"back":1}']`).click();
  expect((await waitForCurrentPage(app, 'home')).instanceId).toBe(home.instanceId);
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['home']);
});

test('redirect through a component replaces only the current stack entry', {
  id: 'NAVIGATOR-REDIRECT-001', app: APP_ID,
  covers: ['lx.nav.go'],
}, async (t) => {
  const { app, page, home } = await openNavigator(t, 'NAVIGATOR-REDIRECT-001');
  await page.view.css('lx-navigator[replace]').click();
  await waitForCurrentPage(app, 'video');
  await expect((await app.page({ name: 'video' })).view.testId('video-page')).toBeVisible();
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['home', 'video']);
  await app.nav.back();
  expect((await app.nav.current()).instanceId).toBe(home.instanceId);
});

test('relaunch through a component resets the stack', {
  id: 'NAVIGATOR-RELAUNCH-001', app: APP_ID,
  covers: ['lx.nav.reset'],
}, async (t) => {
  const { app, page } = await openNavigator(t, 'NAVIGATOR-RELAUNCH-001');
  await page.view.css('lx-navigator[reset]').click();
  await waitForCurrentPage(app, 'chat');
  expect((await app.nav.stack()).map((entry) => entry.name)).toEqual(['chat']);
  await expect((await app.page({ name: 'chat' })).view.testId('stream-page')).toBeVisible();
});
