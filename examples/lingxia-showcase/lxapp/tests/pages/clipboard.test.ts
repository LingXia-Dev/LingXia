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
  await app.page.waitFor({
    page: 'api',
    css: '[data-testid="api-device-section"]',
    state: 'visible',
  });
  await app.page.click({ page: 'api', css: '[data-testid="api-device-section"]' });
  // Clipboard sits at the end of the long Device list, below the fold.
  await app.page.waitFor({ page: 'api', css: '[data-testid="api-clipboard"]', state: 'attached' });
  await app.page.eval({
    page: 'api',
    script: `document.querySelector('[data-testid="api-clipboard"]')?.scrollIntoView({ block: 'center' })`,
  });
  await app.page.waitFor({
    page: 'api',
    css: '[data-testid="api-clipboard"]',
    state: 'visible',
  });
  await app.page.click({ page: 'api', css: '[data-testid="api-clipboard"]' });
  await app.page.waitFor({
    page: 'clipboard',
    css: '[data-testid="clipboard-status"]',
    state: 'visible',
  });

  await app.page.click({ page: 'clipboard', css: '[data-testid="clipboard-write-text"]' });
  await waitForElementText(
    app,
    'clipboard',
    '[data-testid="clipboard-status"]',
    (text) => text.includes('Wrote text'),
  );

  // HarmonyOS denies the read without READ_PASTEBOARD (see LOGIC-CLIPBOARD-001);
  // the page must say so instead of claiming the clipboard is empty.
  const readsDenied = await runtimePlatform(app) === 'harmony';
  await app.page.click({ page: 'clipboard', css: '[data-testid="clipboard-read-text"]' });
  const readStatus = await waitForElementText(
    app,
    'clipboard',
    '[data-testid="clipboard-status"]',
    (text) => readsDenied
      ? text !== 'Wrote text' && !text.includes('Read text')
      : text.includes('Read text'),
  );
  if (readsDenied) expect(readStatus).not.toContain('No text on clipboard');

  await app.page.click({ page: 'clipboard', css: '[data-testid="clipboard-clear"]' });
  const cleared = await waitForElementText(
    app,
    'clipboard',
    '[data-testid="clipboard-status"]',
    (text) => text.includes('Cleared'),
  );
  expect(cleared).toContain('Cleared');
});
