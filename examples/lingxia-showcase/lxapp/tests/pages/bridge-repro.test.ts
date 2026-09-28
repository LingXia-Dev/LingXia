import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { expect, spec, type TestView } from '@lingxia/test';
import { waitForCurrentPage, waitForElementText } from '../helpers/page.js';
import { attachShot } from '../helpers/poll.js';

spec('keeps bootstrap, calls, and streams healthy across the page bridge', { app: SHOWCASE_APP_ID }, async (t) => {
  const app = t.app;
  // Until the page is bound, a failure shot takes whatever page is current.
  let repro: TestView = app.view;
  const waitForText = (css: string, predicate: (text: string) => boolean) =>
    waitForElementText(t, repro, css, predicate, 30_000);
  try {
    await app.nav.relaunch({ page: 'bridge-repro' });
    repro = (await app.page({ name: 'bridge-repro' }, { timeout: 30_000 })).view;
    await repro.css('[data-testid="bridge-repro-page"][data-automation-contract="bridge-v1"]').first().waitFor({ timeout: 30_000 });
    await repro.css('#bootstrap-verdict').first().waitFor({ timeout: 30_000 });

    expect(await waitForText('#bootstrap-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');

    await repro.css('#btn-echo').click();
    expect(await waitForText('#stat-echo', (text) => text.includes('echo #1 ok')))
      .toContain('echo #1 ok');

    await repro.css('#btn-restart').click();
    await waitForText('#stat-received', (text) => Number.parseInt(text.replace(/\D+/g, ''), 10) >= 2);
    expect(await waitForText('#stream-verdict', (text) => text.includes('PASS')))
      .toContain('PASS');
    expect(await waitForText('#stat-gaps', (text) => text.includes('none'))).toContain('none');
    expect(await waitForText('#stat-error', (text) => text.includes('none'))).toContain('none');
    await repro.css('#btn-stop').click();
    await new Promise<void>((resolve) => setTimeout(() => resolve(), 100));
  } catch (error) {
    try {
      const screenshot = await repro.screenshot();
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
