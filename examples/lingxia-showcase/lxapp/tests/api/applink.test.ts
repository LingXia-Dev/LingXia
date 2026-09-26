import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';
import { waitForCurrentPage, waitForElementAttribute } from '../helpers/page.js';
import { bindFixture, expectReject } from '../helpers/poll.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

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
  await waitForCurrentPage(raw, 'home');

  const result = await lx.automation().lxapps.applink({
    url: 'https://applink.lingxia.app/lxapp/open?page=device&type=screen',
  });
  expect(result.accepted).toBe(true);
  expect(result.code).toBe(1);

  await waitForCurrentPage(raw, 'device', 30_000);
  await raw.page.waitFor({ page: 'device', css: '[data-testid="device-page"]' });
  await waitForElementAttribute(
    raw,
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
    { message: 'host is not in appLinks.hosts' },
  );
  expect((await app.nav.current()).name).toBe('device');
});

spec('route a warm AppLink from a product path', {
  id: 'APPLINK-002',
  covers: ['LxAppManager.applink'],
  app: SHOWCASE_APP_ID,
  timeout: 45_000,
}, async (t) => {
  const { app, defer } = bindFixture(t, 'APPLINK-002');
  defer(async () => {
    await app.nav.relaunch({ page: 'home' });
  });

  await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPage(raw, 'home');

  // No `/lxapp/` prefix: the host is the only gate, and Logic routes from the
  // pathname. A stray `path` param stays in the page query.
  const result = await lx.automation().lxapps.applink({
    url: 'https://applink.lingxia.app/showcase/device?type=screen&path=/ignored',
  });
  expect(result.accepted).toBe(true);

  await waitForCurrentPage(raw, 'device', 30_000);
  await waitForElementAttribute(
    raw,
    'device',
    '[data-testid="device-page"]',
    'data-mode',
    'screen',
  );
});
