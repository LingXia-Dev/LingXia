// Diagnostic probe (not part of any suite): drives relaunch/push/pop cycles
// at driver pace using only pages that exist on the branch base, then checks
// whether `home` still boots — reproduces the Harmony post-navigation
// bridge-poisoning without the lifecycle fixtures.
import { expect, spec } from '@lingxia/test';
import { showcaseApp } from '../helpers/app.js';

spec('relaunch/push/pop churn leaves home bootable', async () => {
  const app = showcaseApp();
  for (let i = 0; i < 6; i += 1) {
    await app.nav.relaunch({ page: 'home' });
    await app.nav.to({ page: 'ui' });
    await app.nav.back();
    await app.nav.relaunch({ page: 'device' });
  }
  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({
    page: 'home',
    css: '[data-testid="home-page"]',
    state: 'visible',
    timeoutMs: 8_000,
  });
  const current = await app.nav.current();
  expect(current.ready).toBe(true);
});
