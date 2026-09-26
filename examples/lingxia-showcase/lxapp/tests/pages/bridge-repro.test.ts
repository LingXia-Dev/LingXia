import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';
import { expect, spec } from '@lingxia/test';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { attachShot } from '../helpers/poll.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

const waitForText = (
  app: Parameters<typeof waitForElementText>[0],
  css: string,
  predicate: (text: string) => boolean,
) => waitForElementText(app, 'bridge-repro', css, predicate, 30_000);

spec('keeps bootstrap, calls, and streams healthy across the page bridge', async (t) => {
  const app = t.apps.lxapp(SHOWCASE_APP_ID);
  try {
    await app.nav.relaunch({ page: 'bridge-repro' });
    await raw.page.waitFor({
      page: 'bridge-repro',
      css: '[data-testid="bridge-repro-page"][data-automation-contract="bridge-v1"]',
    });
    await raw.page.waitFor({ page: 'bridge-repro', css: '#bootstrap-verdict' });

    expect(await waitForText(raw, '#bootstrap-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');

    await app.view.css('#btn-echo', { page: 'bridge-repro' }).click();
    expect(await waitForText(raw, '#stat-echo', (text) => text.includes('echo #1 ok')))
      .toContain('echo #1 ok');

    await app.view.css('#btn-restart', { page: 'bridge-repro' }).click();
    await waitForText(raw, '#stat-received', (text) => Number.parseInt(text.replace(/\D+/g, ''), 10) >= 2);
    expect(await waitForText(raw, '#stream-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');
    expect(await waitForText(raw, '#stat-gaps', (text) => text.includes('none'))).toContain('none');
    expect(await waitForText(raw, '#stat-error', (text) => text.includes('none'))).toContain('none');
    await app.view.css('#btn-stop', { page: 'bridge-repro' }).click();
    await new Promise<void>((resolve) => setTimeout(() => resolve(), 100));
  } catch (error) {
    try {
      const screenshot = await raw.page.screenshot({ page: 'bridge-repro' });
      await attachShot(t, 'bridge-repro-failure.png', {
        mimeType: 'image/png',
        base64: screenshot.base64,
      });
    } catch {
      // Preserve the bridge failure when screenshot capture also fails.
    }
    throw error;
  } finally {
    try {
      await app.nav.relaunch({ page: 'home' });
      await waitForCurrentPage(raw, 'home');
    } catch {
      // Keep a cleanup failure from hiding the original bridge assertion.
    }
  }
});
