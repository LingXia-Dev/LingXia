import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { bindFixture } from '../helpers/poll.js';
import { tabIndex } from '../helpers/tabs.js';
import { pinEnglish } from '../helpers/language.js';

// Labels below are the English ones.
spec.beforeEach(pinEnglish);

// Host chrome driven as a user does: found through the host's own view or
// accessibility tree by its label, pressed with a real tap at its bounds.

spec('a tap on a tab label switches to that tab and shows its declared chrome', {
  id: 'CHROME-TAB-001',
  covers: ['app:tabBar', 'app:tabBar.presentation=immersive', 'page:navigationStyle=default', 'page:navigationStyle=custom'],
  app: SHOWCASE_APP_ID,
  requires: { input: ['touch', 'mouse'] },
}, async (t) => {
  const { app } = bindFixture(t, 'CHROME-TAB-001');
  await app.nav.relaunch({ page: 'home' });
  // home declares navigationStyle custom: no host navigation bar to press.
  await expect.poll(async () => (await app.chrome.elements({ scope: 'navigationBar' })).length).toBe(0);

  await app.tabBar.tap('API');
  await waitForCurrentPage(app, 'api');
  const api = await tabIndex(app, ['API', '接口']);
  await expect.poll(async () => (await app.info()).tabBar?.selectedIndex).toBe(api);
  // api declares navigationStyle default with its title.
  await expect.poll(async () => (await app.info()).navigationBar?.title).toBe('API');

  await app.tabBar.tap('Home');
  await waitForCurrentPage(app, 'home');
});

spec('the navigation bar back button returns to the page below', {
  id: 'CHROME-BACK-001',
  covers: ['app:navigationBar.back'],
  app: SHOWCASE_APP_ID,
  requires: { input: ['touch', 'mouse'] },
}, async (t) => {
  const { app } = bindFixture(t, 'CHROME-BACK-001');
  await app.nav.relaunch({ page: 'api' });
  await app.nav.to({ page: 'system' });
  await waitForCurrentPage(app, 'system');
  await app.chrome.tap({ label: 'Back', scope: 'navigationBar' });
  await waitForCurrentPage(app, 'api');
  expect((await app.nav.stack()).length).toBe(1);
});
