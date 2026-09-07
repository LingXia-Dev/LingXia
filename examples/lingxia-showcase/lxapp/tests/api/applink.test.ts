import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage, waitForElementAttribute } from '../helpers/page.js';
import { bindFixture, expectReject } from '../helpers/poll.js';

spec('route a warm AppLink onto the query page once', {
  id: 'APPLINK-001',
  covers: ['LxAppManager.applink'],
  app: SHOWCASE_APP_ID,
  timeout: 45_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'APPLINK-001');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(app, 'home');

  const result = await lx.automation().lxapps.applink({
    url: 'https://applink.lingxia.app/lxapp/open?page=device&type=screen',
  });
  expect(result.accepted).toBe(true);
  expect(result.code).toBe(1);

  await waitForCurrentPage(app, 'device', 30_000);
  await app.page.waitFor({ page: 'device', css: '[data-testid="device-page"]' });
  await waitForElementAttribute(
    app,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'screen',
  );
  expect((await app.nav.stack()).map((page) => page.name)).toEqual(['home', 'device']);

  await expectReject(
    () => lx.automation().lxapps.applink({
      url: 'https://evil.example/lxapp/open?page=todo',
    }),
    { message: 'not a configured AppLink' },
  );
  expect((await app.nav.current()).name).toBe('device');
});
