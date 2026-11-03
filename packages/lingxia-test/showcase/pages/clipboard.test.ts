import { expect, spec } from '@lingxia/test';
import { waitForElementText } from '../helpers/page.js';
import { bindFixture } from '../helpers/poll.js';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { runtimePlatform } from '../helpers/platform.js';

spec('open the clipboard demo from the API menu and round-trip text', {
  id: 'UI-CLIPBOARD-001',
  covers: [
    'lx.clipboard.writeText',
    'lx.clipboard.readText',
    'lx.clipboard.clear',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
}, async (t) => {
  const { app } = bindFixture(t, 'UI-CLIPBOARD-001');

  await app.nav.relaunch({ page: 'api' });
  const api = (await app.page({ name: 'api' }, { timeout: 30_000 })).view;
  await api.testId('api-device-section').waitFor({ state: 'visible', timeout: 30_000 });
  await api.testId("api-device-section").click();
  // Clipboard sits at the end of the long Device list, below the fold.
  await api.testId('api-clipboard').waitFor({ state: 'attached', timeout: 30_000 });
  // The click scrolls it into view.
  await api.testId('api-clipboard').waitFor({ state: 'visible', timeout: 30_000 });
  await api.testId("api-clipboard").click();
  const clipboard = (await app.page({ name: 'clipboard' }, { timeout: 30_000 })).view;
  await clipboard.testId('clipboard-status').waitFor({ state: 'visible', timeout: 30_000 });

  await clipboard.testId("clipboard-write-text").click();
  await waitForElementText(
    t,
    clipboard,
    '[data-testid="clipboard-status"]',
    (text) => text.includes('Wrote text'),
  );

  // HarmonyOS denies the read without READ_PASTEBOARD (see LOGIC-CLIPBOARD-001);
  // the page must say so instead of claiming the clipboard is empty.
  const readsDenied = await runtimePlatform(app) === 'harmony';
  await clipboard.testId("clipboard-read-text").click();
  const readStatus = await waitForElementText(
    t,
    clipboard,
    '[data-testid="clipboard-status"]',
    (text) => readsDenied
      ? text !== 'Wrote text' && !text.includes('Read text')
      : text.includes('Read text'),
  );
  if (readsDenied) expect(readStatus).not.toContain('No text on clipboard');

  await clipboard.testId("clipboard-clear").click();
  const cleared = await waitForElementText(
    t,
    clipboard,
    '[data-testid="clipboard-status"]',
    (text) => text.includes('Cleared'),
  );
  expect(cleared).toContain('Cleared');
});
