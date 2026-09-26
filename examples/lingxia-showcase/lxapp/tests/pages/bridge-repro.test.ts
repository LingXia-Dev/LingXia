import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec, type Fixture } from '@lingxia/test';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { attachShot } from '../helpers/poll.js';

const waitForText = (
  t: Fixture,
  css: string,
  predicate: (text: string) => boolean,
) => waitForElementText(t, 'bridge-repro', css, predicate, 30_000);

spec('keeps bootstrap, calls, and streams healthy across the page bridge', { app: SHOWCASE_APP_ID }, async (t) => {
  const app = t.app;
  try {
    await app.nav.relaunch({ page: 'bridge-repro' });
    await app.view.css('[data-testid="bridge-repro-page"][data-automation-contract="bridge-v1"]', { page: 'bridge-repro' }).first().waitFor({ timeout: 30_000 });
    await app.view.css('#bootstrap-verdict', { page: 'bridge-repro' }).first().waitFor({ timeout: 30_000 });

    expect(await waitForText(t, '#bootstrap-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');

    await app.view.css('#btn-echo', { page: 'bridge-repro' }).click();
    expect(await waitForText(t, '#stat-echo', (text) => text.includes('echo #1 ok')))
      .toContain('echo #1 ok');

    await app.view.css('#btn-restart', { page: 'bridge-repro' }).click();
    await waitForText(t, '#stat-received', (text) => Number.parseInt(text.replace(/\D+/g, ''), 10) >= 2);
    expect(await waitForText(t, '#stream-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');
    expect(await waitForText(t, '#stat-gaps', (text) => text.includes('none'))).toContain('none');
    expect(await waitForText(t, '#stat-error', (text) => text.includes('none'))).toContain('none');
    await app.view.css('#btn-stop', { page: 'bridge-repro' }).click();
    await new Promise<void>((resolve) => setTimeout(() => resolve(), 100));
  } catch (error) {
    try {
      const screenshot = await app.view.screenshot({ page: 'bridge-repro' });
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
      await waitForCurrentPage(app, 'home');
    } catch {
      // Keep a cleanup failure from hiding the original bridge assertion.
    }
  }
});
